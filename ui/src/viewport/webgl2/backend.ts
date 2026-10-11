import type {
  DrawCall,
  Geometry,
  PickSample,
  RenderBackend,
  RenderFrame,
  Rgb,
} from '../backend'
import {
  DASH_GAP,
  DASH_LENGTH,
  EMPHASIS_LINE_WIDTH,
  EMPHASIS_POINT_SIZE,
  FLOAT_BYTES,
  LINE_ATTRIBUTES,
  LINE_CORNERS,
  POINT_ATTRIBUTES,
  POINT_CORNERS,
  POSITION_BYTES,
  SEGMENT_BYTES,
  SURFACE_ATTRIBUTES,
  elementTextureSize,
  geometryLengthProblem,
  lineStartDistances,
  packElementColors,
  packElementStates,
} from './layout'
import { collectPickSamples, pickBlock } from './readback'
import { shaderSources, type Pass, type ProgramKind } from './shaders'

const CONTEXT_ATTRIBUTES: WebGLContextAttributes = {
  alpha: false,
  antialias: true,
  depth: true,
  stencil: false,
  premultipliedAlpha: true,
  preserveDrawingBuffer: false,
  powerPreference: 'high-performance',
}

const LAYERS: readonly DrawCall['layer'][] = ['opaque', 'translucent', 'overlay']
const STATE_TEXTURE_UNIT = 0
const COLOR_TEXTURE_UNIT = 1

type Program = {
  readonly program: WebGLProgram
  readonly uniforms: Map<string, WebGLUniformLocation | null>
}

type Programs = Readonly<Record<`${ProgramKind}-${Pass}`, Program | null>>

type Resources = {
  readonly programs: Programs
  readonly lineCorners: WebGLBuffer
  readonly pointCorners: WebGLBuffer
  readonly emptyVertexArray: WebGLVertexArrayObject
  /** A 1 × 1 color texture bound when a draw has no element colors, so the sampler always has a texture. */
  readonly noElementColors: WebGLTexture
}

type GpuGeometry = {
  readonly kind: Geometry['kind']
  readonly vertexArray: WebGLVertexArrayObject
  readonly buffers: readonly WebGLBuffer[]
  readonly states: WebGLTexture
  readonly elementCount: number
  readonly vertexCount: number
  /** Index count for triangles and fills, instance count for lines and points. */
  readonly drawCount: number
  scalars: WebGLBuffer | null
}

type IdTarget = {
  readonly framebuffer: WebGLFramebuffer
  readonly textures: readonly WebGLTexture[]
  readonly depth: WebGLRenderbuffer
  readonly width: number
  readonly height: number
}

type Size = { readonly width: number; readonly height: number; readonly pixelRatio: number }

function compileShader(
  gl: WebGL2RenderingContext,
  type: GLenum,
  source: string,
  name: string,
): WebGLShader {
  const shader = gl.createShader(type)
  if (shader === null) {
    throw new Error(`Could not create the ${name} shader.`)
  }
  gl.shaderSource(shader, source)
  gl.compileShader(shader)
  if (gl.getShaderParameter(shader, gl.COMPILE_STATUS) !== true && !gl.isContextLost()) {
    const log = gl.getShaderInfoLog(shader) ?? ''
    gl.deleteShader(shader)
    throw new Error(`The ${name} shader does not compile: ${log}`)
  }
  return shader
}

function linkProgram(gl: WebGL2RenderingContext, kind: ProgramKind, pass: Pass): Program {
  const name = `${kind} ${pass}`
  const sources = shaderSources(kind, pass)
  const vertex = compileShader(gl, gl.VERTEX_SHADER, sources.vertex, `${name} vertex`)
  const fragment = compileShader(gl, gl.FRAGMENT_SHADER, sources.fragment, `${name} fragment`)
  const program = gl.createProgram()
  gl.attachShader(program, vertex)
  gl.attachShader(program, fragment)
  gl.linkProgram(program)
  gl.deleteShader(vertex)
  gl.deleteShader(fragment)
  if (gl.getProgramParameter(program, gl.LINK_STATUS) !== true && !gl.isContextLost()) {
    const log = gl.getProgramInfoLog(program) ?? ''
    gl.deleteProgram(program)
    throw new Error(`The ${name} program does not link: ${log}`)
  }
  return { program, uniforms: new Map() }
}

function staticBuffer(gl: WebGL2RenderingContext, data: Float32Array): WebGLBuffer {
  const buffer = gl.createBuffer()
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
  gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW)
  return buffer
}

function nearestTexture(gl: WebGL2RenderingContext): WebGLTexture {
  const texture = gl.createTexture()
  gl.bindTexture(gl.TEXTURE_2D, texture)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE)
  return texture
}

function uploadStates(
  gl: WebGL2RenderingContext,
  texture: WebGLTexture,
  states: Uint8Array,
  elementCount: number,
): void {
  const { width, height } = elementTextureSize(elementCount)
  gl.activeTexture(gl.TEXTURE0 + STATE_TEXTURE_UNIT)
  gl.bindTexture(gl.TEXTURE_2D, texture)
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1)
  gl.texImage2D(
    gl.TEXTURE_2D,
    0,
    gl.R8UI,
    width,
    height,
    0,
    gl.RED_INTEGER,
    gl.UNSIGNED_BYTE,
    packElementStates(states, elementCount),
  )
}

function uploadColors(
  gl: WebGL2RenderingContext,
  texture: WebGLTexture,
  colors: Float32Array,
  elementCount: number,
): void {
  const { width, height } = elementTextureSize(elementCount)
  gl.activeTexture(gl.TEXTURE0 + COLOR_TEXTURE_UNIT)
  gl.bindTexture(gl.TEXTURE_2D, texture)
  gl.texImage2D(
    gl.TEXTURE_2D,
    0,
    gl.RGBA32F,
    width,
    height,
    0,
    gl.RGBA,
    gl.FLOAT,
    packElementColors(colors, elementCount),
  )
}

function float3(gl: WebGL2RenderingContext, location: number, buffer: WebGLBuffer): void {
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
  gl.enableVertexAttribArray(location)
  gl.vertexAttribPointer(location, 3, gl.FLOAT, false, 0, 0)
}

function elementIds(
  gl: WebGL2RenderingContext,
  location: number,
  buffer: WebGLBuffer,
  divisor: number,
): void {
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
  gl.enableVertexAttribArray(location)
  gl.vertexAttribIPointer(location, 1, gl.UNSIGNED_INT, 0, 0)
  gl.vertexAttribDivisor(location, divisor)
}

function corners(gl: WebGL2RenderingContext, location: number, buffer: WebGLBuffer): void {
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
  gl.enableVertexAttribArray(location)
  gl.vertexAttribPointer(location, 2, gl.FLOAT, false, 0, 0)
}

/** Returns null when the canvas cannot create a WebGL2 context. */
export function createWebGl2Backend(canvas: HTMLCanvasElement): RenderBackend | null {
  const context = canvas.getContext('webgl2', CONTEXT_ATTRIBUTES)
  if (!context) {
    return null
  }
  const gl: WebGL2RenderingContext = context
  const geometries = new Map<string, GpuGeometry>()
  const colorTextures = new Map<string, { source: Float32Array; texture: WebGLTexture }>()
  const lostListeners = new Set<() => void>()
  const restoredListeners = new Set<() => void>()
  let resources: Resources | null = null
  let idTarget: IdTarget | null = null
  let idFrame: RenderFrame | null = null
  let size: Size | null = null
  let lost = gl.isContextLost()
  let disposed = false

  function createResources(): Resources {
    const programs: Programs = {
      'background-color': linkProgram(gl, 'background', 'color'),
      'background-id': null,
      'triangles-color': linkProgram(gl, 'triangles', 'color'),
      'triangles-id': linkProgram(gl, 'triangles', 'id'),
      'fill-color': linkProgram(gl, 'fill', 'color'),
      'fill-id': linkProgram(gl, 'fill', 'id'),
      'lines-color': linkProgram(gl, 'lines', 'color'),
      'lines-id': linkProgram(gl, 'lines', 'id'),
      'points-color': linkProgram(gl, 'points', 'color'),
      'points-id': linkProgram(gl, 'points', 'id'),
    }
    const noElementColors = nearestTexture(gl)
    uploadColors(gl, noElementColors, new Float32Array(3), 1)
    gl.disable(gl.DITHER)
    gl.cullFace(gl.BACK)
    gl.depthFunc(gl.LEQUAL)
    gl.polygonOffset(1, 1)
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA)
    return {
      programs,
      lineCorners: staticBuffer(gl, LINE_CORNERS),
      pointCorners: staticBuffer(gl, POINT_CORNERS),
      emptyVertexArray: gl.createVertexArray(),
      noElementColors,
    }
  }

  function assertUsable(method: string): void {
    if (disposed) {
      throw new Error(`The WebGL2 backend's ${method} was called after dispose.`)
    }
  }

  function contextAvailable(): boolean {
    return !lost && !gl.isContextLost()
  }

  function ready(): Resources | null {
    if (!contextAvailable()) {
      return null
    }
    resources ??= createResources()
    return resources
  }

  function deleteColorTexture(id: string): void {
    const colors = colorTextures.get(id)
    if (colors !== undefined && contextAvailable()) {
      gl.deleteTexture(colors.texture)
    }
    colorTextures.delete(id)
  }

  function deleteGeometry(geometry: GpuGeometry): void {
    gl.deleteVertexArray(geometry.vertexArray)
    for (const buffer of geometry.buffers) {
      gl.deleteBuffer(buffer)
    }
    if (geometry.scalars !== null) {
      gl.deleteBuffer(geometry.scalars)
    }
    gl.deleteTexture(geometry.states)
  }

  function deleteIdTarget(): void {
    if (idTarget === null) {
      return
    }
    gl.deleteFramebuffer(idTarget.framebuffer)
    for (const texture of idTarget.textures) {
      gl.deleteTexture(texture)
    }
    gl.deleteRenderbuffer(idTarget.depth)
    idTarget = null
  }

  function deleteResources(): void {
    for (const geometry of geometries.values()) {
      deleteGeometry(geometry)
    }
    for (const { texture } of colorTextures.values()) {
      gl.deleteTexture(texture)
    }
    deleteIdTarget()
    if (resources !== null) {
      for (const program of Object.values(resources.programs)) {
        if (program !== null) {
          gl.deleteProgram(program.program)
        }
      }
      gl.deleteBuffer(resources.lineCorners)
      gl.deleteBuffer(resources.pointCorners)
      gl.deleteVertexArray(resources.emptyVertexArray)
      gl.deleteTexture(resources.noElementColors)
    }
    forgetResources()
  }

  // After a context loss every handle is already invalid: forget them without deleting.
  function forgetResources(): void {
    geometries.clear()
    colorTextures.clear()
    idTarget = null
    idFrame = null
    resources = null
  }

  function buildGeometry(current: Resources, geometry: Geometry): GpuGeometry {
    const vertexArray = gl.createVertexArray()
    gl.bindVertexArray(vertexArray)
    const buffers: WebGLBuffer[] = []
    const arrayBuffer = (data: Float32Array | Uint32Array): WebGLBuffer => {
      const buffer = gl.createBuffer()
      gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
      gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW)
      buffers.push(buffer)
      return buffer
    }
    let drawCount = 0
    const vertexCount = geometry.positions.length / 3
    switch (geometry.kind) {
      case 'triangles':
      case 'fill': {
        float3(gl, SURFACE_ATTRIBUTES.position, arrayBuffer(geometry.positions))
        if (geometry.kind === 'triangles') {
          float3(gl, SURFACE_ATTRIBUTES.normal, arrayBuffer(geometry.normals))
        }
        elementIds(gl, SURFACE_ATTRIBUTES.elementId, arrayBuffer(geometry.elementIds), 0)
        const indices = gl.createBuffer()
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, indices)
        gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, geometry.indices, gl.STATIC_DRAW)
        buffers.push(indices)
        drawCount = geometry.indices.length
        break
      }
      case 'lines': {
        arrayBuffer(geometry.positions)
        gl.enableVertexAttribArray(LINE_ATTRIBUTES.start)
        gl.vertexAttribPointer(LINE_ATTRIBUTES.start, 3, gl.FLOAT, false, SEGMENT_BYTES, 0)
        gl.vertexAttribDivisor(LINE_ATTRIBUTES.start, 1)
        gl.enableVertexAttribArray(LINE_ATTRIBUTES.end)
        gl.vertexAttribPointer(
          LINE_ATTRIBUTES.end,
          3,
          gl.FLOAT,
          false,
          SEGMENT_BYTES,
          POSITION_BYTES,
        )
        gl.vertexAttribDivisor(LINE_ATTRIBUTES.end, 1)
        elementIds(gl, LINE_ATTRIBUTES.elementId, arrayBuffer(geometry.elementIds), 1)
        arrayBuffer(lineStartDistances(geometry.positions))
        gl.enableVertexAttribArray(LINE_ATTRIBUTES.startDistance)
        gl.vertexAttribPointer(LINE_ATTRIBUTES.startDistance, 1, gl.FLOAT, false, FLOAT_BYTES, 0)
        gl.vertexAttribDivisor(LINE_ATTRIBUTES.startDistance, 1)
        corners(gl, LINE_ATTRIBUTES.corner, current.lineCorners)
        drawCount = geometry.elementIds.length
        break
      }
      case 'points': {
        float3(gl, POINT_ATTRIBUTES.center, arrayBuffer(geometry.positions))
        gl.vertexAttribDivisor(POINT_ATTRIBUTES.center, 1)
        elementIds(gl, POINT_ATTRIBUTES.elementId, arrayBuffer(geometry.elementIds), 1)
        corners(gl, POINT_ATTRIBUTES.corner, current.pointCorners)
        drawCount = vertexCount
        break
      }
    }
    gl.bindVertexArray(null)
    const states = nearestTexture(gl)
    uploadStates(gl, states, new Uint8Array(0), geometry.elementCount)
    return {
      kind: geometry.kind,
      vertexArray,
      buffers,
      states,
      elementCount: geometry.elementCount,
      vertexCount,
      drawCount,
      scalars: null,
    }
  }

  function uploadedGeometry(method: string, id: string): GpuGeometry | null {
    if (!contextAvailable()) {
      return null
    }
    const geometry = geometries.get(id)
    if (geometry === undefined) {
      throw new Error(`${method}("${id}"): no geometry is uploaded under this id.`)
    }
    return geometry
  }

  function bufferSize(): { readonly width: number; readonly height: number } {
    return { width: gl.drawingBufferWidth, height: gl.drawingBufferHeight }
  }

  function pixelRatioOf(frame: RenderFrame): number {
    return size?.pixelRatio ?? frame.pixelRatio
  }

  function use(program: Program): (name: string) => WebGLUniformLocation | null {
    gl.useProgram(program.program)
    return (name) => {
      if (!program.uniforms.has(name)) {
        program.uniforms.set(name, gl.getUniformLocation(program.program, name))
      }
      return program.uniforms.get(name) ?? null
    }
  }

  function setRgb(location: WebGLUniformLocation | null, color: Rgb): void {
    gl.uniform3f(location, color[0], color[1], color[2])
  }

  function elementColorTexture(current: Resources, draw: DrawCall, elementCount: number): WebGLTexture {
    if (draw.elementColors === null) {
      return current.noElementColors
    }
    const cached = colorTextures.get(draw.id)
    if (cached !== undefined && cached.source === draw.elementColors) {
      return cached.texture
    }
    const texture = cached?.texture ?? nearestTexture(gl)
    uploadColors(gl, texture, draw.elementColors, elementCount)
    colorTextures.set(draw.id, { source: draw.elementColors, texture })
    return texture
  }

  function setDrawState(draw: DrawCall, geometry: GpuGeometry, frame: RenderFrame, pass: Pass): void {
    const sectionCut = frame.section !== null && draw.clipped
    if (draw.depthTest) {
      gl.enable(gl.DEPTH_TEST)
    } else {
      gl.disable(gl.DEPTH_TEST)
    }
    const surface = geometry.kind === 'triangles' || geometry.kind === 'fill'
    if (pass === 'id') {
      gl.disable(gl.BLEND)
      gl.depthMask(true)
    } else {
      const opaqueSurface = geometry.kind === 'triangles' && draw.layer === 'opaque' && draw.opacity >= 1
      if (opaqueSurface) {
        gl.disable(gl.BLEND)
      } else {
        gl.enable(gl.BLEND)
      }
      gl.depthMask(surface && draw.layer === 'opaque')
    }
    if (geometry.kind === 'triangles' && !sectionCut) {
      gl.enable(gl.CULL_FACE)
    } else {
      gl.disable(gl.CULL_FACE)
    }
    if (geometry.kind === 'triangles') {
      gl.enable(gl.POLYGON_OFFSET_FILL)
    } else {
      gl.disable(gl.POLYGON_OFFSET_FILL)
    }
  }

  function draw(
    current: Resources,
    drawCall: DrawCall,
    frame: RenderFrame,
    pass: Pass,
    viewportSize: { readonly width: number; readonly height: number },
  ): void {
    const geometry = geometries.get(drawCall.id)
    if (geometry === undefined || geometry.drawCount === 0) {
      return
    }
    const program = current.programs[`${geometry.kind}-${pass}`]
    if (program === null) {
      return
    }
    const uniform = use(program)
    setDrawState(drawCall, geometry, frame, pass)
    gl.uniformMatrix4fv(uniform('u_view'), false, frame.view)
    gl.uniformMatrix4fv(uniform('u_projection'), false, frame.projection)
    gl.uniform2f(uniform('u_viewportSize'), viewportSize.width, viewportSize.height)
    const section = drawCall.clipped ? frame.section : null
    gl.uniform1i(uniform('u_clipped'), section === null ? 0 : 1)
    if (section !== null) {
      gl.uniform4f(
        uniform('u_clipPlane'),
        section.normal[0],
        section.normal[1],
        section.normal[2],
        section.offset,
      )
    }
    gl.activeTexture(gl.TEXTURE0 + STATE_TEXTURE_UNIT)
    gl.bindTexture(gl.TEXTURE_2D, geometry.states)
    gl.uniform1i(uniform('u_elementStates'), STATE_TEXTURE_UNIT)
    const colors = elementColorTexture(current, drawCall, geometry.elementCount)
    gl.activeTexture(gl.TEXTURE0 + COLOR_TEXTURE_UNIT)
    gl.bindTexture(gl.TEXTURE_2D, colors)
    gl.uniform1i(uniform('u_elementColors'), COLOR_TEXTURE_UNIT)
    gl.uniform1i(uniform('u_hasElementColors'), drawCall.elementColors === null ? 0 : 1)
    setRgb(uniform('u_color'), drawCall.color)
    const devicePixels = frame.uiScale * pixelRatioOf(frame)
    if (pass === 'id') {
      gl.uniform1ui(uniform('u_pickBase'), drawCall.pickBase ?? 0)
      gl.uniform1f(uniform('u_width'), drawCall.pickSize * devicePixels)
      gl.uniform1f(uniform('u_size'), drawCall.pickSize * devicePixels)
    } else {
      setRgb(uniform('u_highlight'), frame.theme.highlight)
      setRgb(uniform('u_problem'), frame.theme.problem)
      gl.uniform1f(uniform('u_opacity'), drawCall.opacity)
      gl.uniform1f(uniform('u_width'), drawCall.size * devicePixels)
      gl.uniform1f(uniform('u_emphasisWidth'), EMPHASIS_LINE_WIDTH * devicePixels)
      gl.uniform1f(uniform('u_size'), drawCall.size * devicePixels)
      gl.uniform1f(uniform('u_emphasisSize'), EMPHASIS_POINT_SIZE * devicePixels)
      gl.uniform1i(uniform('u_dashed'), drawCall.dashed ? 1 : 0)
      gl.uniform1f(uniform('u_dashLength'), DASH_LENGTH * devicePixels)
      gl.uniform1f(uniform('u_dashPeriod'), (DASH_LENGTH + DASH_GAP) * devicePixels)
      gl.uniform1i(uniform('u_matcap'), drawCall.shading === 'matcap' ? 1 : 0)
      gl.uniform1i(uniform('u_sectionCaps'), section === null ? 0 : 1)
      const analysis = drawCall.analysis && geometry.scalars !== null ? frame.analysis : null
      gl.uniform1i(uniform('u_analysis'), analysis === null ? 0 : 1)
      if (analysis !== null) {
        setRgb(uniform('u_analysisLow'), analysis.low)
        setRgb(uniform('u_analysisHigh'), analysis.high)
        gl.uniform1f(uniform('u_analysisThreshold'), analysis.threshold)
        gl.uniform1f(uniform('u_analysisMaximum'), analysis.maximum)
      }
    }
    gl.bindVertexArray(geometry.vertexArray)
    if (geometry.kind === 'triangles' && geometry.scalars === null) {
      gl.vertexAttrib1f(SURFACE_ATTRIBUTES.scalar, 0)
    }
    if (geometry.kind === 'triangles' || geometry.kind === 'fill') {
      gl.drawElements(gl.TRIANGLES, geometry.drawCount, gl.UNSIGNED_INT, 0)
    } else {
      gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, geometry.drawCount)
    }
    gl.bindVertexArray(null)
  }

  function drawLayers(current: Resources, frame: RenderFrame, pass: Pass): void {
    const viewportSize = bufferSize()
    for (const layer of LAYERS) {
      for (const drawCall of frame.draws) {
        if (drawCall.layer === layer && (pass === 'color' || drawCall.pickBase !== null)) {
          draw(current, drawCall, frame, pass, viewportSize)
        }
      }
    }
  }

  function drawBackground(current: Resources, frame: RenderFrame): void {
    const program = current.programs['background-color']
    if (program === null) {
      return
    }
    const uniform = use(program)
    setRgb(uniform('u_backgroundTop'), frame.theme.backgroundTop)
    setRgb(uniform('u_backgroundBottom'), frame.theme.backgroundBottom)
    gl.disable(gl.DEPTH_TEST)
    gl.disable(gl.BLEND)
    gl.disable(gl.CULL_FACE)
    gl.depthMask(false)
    gl.bindVertexArray(current.emptyVertexArray)
    gl.drawArrays(gl.TRIANGLES, 0, 3)
    gl.bindVertexArray(null)
  }

  function ensureIdTarget(): IdTarget {
    const { width, height } = bufferSize()
    if (idTarget !== null && idTarget.width === width && idTarget.height === height) {
      return idTarget
    }
    deleteIdTarget()
    const framebuffer = gl.createFramebuffer()
    gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer)
    const textures = [0, 1].map((attachment) => {
      const texture = nearestTexture(gl)
      gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGBA8, width, height)
      gl.framebufferTexture2D(
        gl.FRAMEBUFFER,
        gl.COLOR_ATTACHMENT0 + attachment,
        gl.TEXTURE_2D,
        texture,
        0,
      )
      return texture
    })
    const depth = gl.createRenderbuffer()
    gl.bindRenderbuffer(gl.RENDERBUFFER, depth)
    gl.renderbufferStorage(gl.RENDERBUFFER, gl.DEPTH_COMPONENT24, width, height)
    gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.RENDERBUFFER, depth)
    gl.bindFramebuffer(gl.FRAMEBUFFER, null)
    idTarget = { framebuffer, textures, depth, width, height }
    idFrame = null
    return idTarget
  }

  function renderIdPass(current: Resources, target: IdTarget, frame: RenderFrame): void {
    gl.bindFramebuffer(gl.FRAMEBUFFER, target.framebuffer)
    gl.drawBuffers([gl.COLOR_ATTACHMENT0, gl.COLOR_ATTACHMENT1])
    gl.viewport(0, 0, target.width, target.height)
    gl.depthMask(true)
    gl.clearBufferfv(gl.COLOR, 0, [0, 0, 0, 0])
    gl.clearBufferfv(gl.COLOR, 1, [0, 0, 0, 0])
    gl.clearBufferfv(gl.DEPTH, 0, [1])
    drawLayers(current, frame, 'id')
    gl.bindFramebuffer(gl.FRAMEBUFFER, null)
    idFrame = frame
  }

  function readAttachment(
    attachment: number,
    block: { left: number; bottom: number; width: number; height: number },
  ): Uint8Array {
    const pixels = new Uint8Array(block.width * block.height * 4)
    gl.readBuffer(gl.COLOR_ATTACHMENT0 + attachment)
    gl.readPixels(block.left, block.bottom, block.width, block.height, gl.RGBA, gl.UNSIGNED_BYTE, pixels)
    return pixels
  }

  const handleContextLost = (event: Event): void => {
    // Lets the browser restore the context (3.12).
    event.preventDefault()
    lost = true
    forgetResources()
    for (const listener of [...lostListeners]) {
      listener()
    }
  }

  const handleContextRestored = (): void => {
    lost = false
    forgetResources()
    for (const listener of [...restoredListeners]) {
      listener()
    }
  }

  canvas.addEventListener('webglcontextlost', handleContextLost)
  canvas.addEventListener('webglcontextrestored', handleContextRestored)
  ready()

  return {
    kind: 'webgl2',
    resize(width, height, pixelRatio) {
      assertUsable('resize')
      size = { width, height, pixelRatio }
      canvas.width = Math.max(1, Math.round(width * pixelRatio))
      canvas.height = Math.max(1, Math.round(height * pixelRatio))
      idFrame = null
    },
    upload(id, geometry) {
      assertUsable('upload')
      const problem = geometryLengthProblem(geometry)
      if (problem !== null) {
        throw new RangeError(`upload("${id}"): ${problem}.`)
      }
      const current = ready()
      if (current === null) {
        return
      }
      const previous = geometries.get(id)
      if (previous !== undefined) {
        deleteGeometry(previous)
      }
      deleteColorTexture(id)
      geometries.set(id, buildGeometry(current, geometry))
      idFrame = null
    },
    remove(id) {
      assertUsable('remove')
      const geometry = geometries.get(id)
      if (geometry !== undefined && contextAvailable()) {
        deleteGeometry(geometry)
      }
      geometries.delete(id)
      deleteColorTexture(id)
      idFrame = null
    },
    setElementStates(id, states) {
      assertUsable('setElementStates')
      const geometry = uploadedGeometry('setElementStates', id)
      if (geometry === null) {
        return
      }
      if (states.length !== geometry.elementCount) {
        throw new RangeError(
          `setElementStates("${id}"): ${String(states.length)} states for ${String(geometry.elementCount)} elements.`,
        )
      }
      uploadStates(gl, geometry.states, states, geometry.elementCount)
    },
    setScalarField(id, values) {
      assertUsable('setScalarField')
      const geometry = uploadedGeometry('setScalarField', id)
      if (geometry === null) {
        return
      }
      if (values !== null && values.length !== geometry.vertexCount) {
        throw new RangeError(
          `setScalarField("${id}"): ${String(values.length)} values for ${String(geometry.vertexCount)} vertices.`,
        )
      }
      // Only shaded triangles color a scalar field (3.12 pass 3).
      if (geometry.kind !== 'triangles') {
        return
      }
      gl.bindVertexArray(geometry.vertexArray)
      if (values === null) {
        gl.disableVertexAttribArray(SURFACE_ATTRIBUTES.scalar)
        if (geometry.scalars !== null) {
          gl.deleteBuffer(geometry.scalars)
        }
        geometry.scalars = null
      } else {
        geometry.scalars ??= gl.createBuffer()
        gl.bindBuffer(gl.ARRAY_BUFFER, geometry.scalars)
        gl.bufferData(gl.ARRAY_BUFFER, values, gl.STATIC_DRAW)
        gl.enableVertexAttribArray(SURFACE_ATTRIBUTES.scalar)
        gl.vertexAttribPointer(SURFACE_ATTRIBUTES.scalar, 1, gl.FLOAT, false, 0, 0)
      }
      gl.bindVertexArray(null)
    },
    render(frame) {
      assertUsable('render')
      const current = ready()
      if (current === null) {
        return
      }
      const { width, height } = bufferSize()
      gl.bindFramebuffer(gl.FRAMEBUFFER, null)
      gl.viewport(0, 0, width, height)
      drawBackground(current, frame)
      gl.depthMask(true)
      gl.clear(gl.DEPTH_BUFFER_BIT)
      drawLayers(current, frame, 'color')
    },
    pick(frame, x, y, radius): readonly PickSample[] {
      assertUsable('pick')
      const current = ready()
      if (current === null) {
        return []
      }
      const target = ensureIdTarget()
      if (idFrame !== frame) {
        renderIdPass(current, target, frame)
      }
      const pixelRatio = pixelRatioOf(frame)
      const block = pickBlock(x, y, radius, pixelRatio, target.width, target.height)
      if (block === null) {
        return []
      }
      gl.bindFramebuffer(gl.FRAMEBUFFER, target.framebuffer)
      const ids = readAttachment(0, block)
      const depths = readAttachment(1, block)
      gl.bindFramebuffer(gl.FRAMEBUFFER, null)
      return collectPickSamples({
        ids,
        depths,
        block,
        bufferHeight: target.height,
        pixelRatio,
        x,
        y,
        radius,
      })
    },
    onContextLost(listener) {
      lostListeners.add(listener)
      return () => {
        lostListeners.delete(listener)
      }
    },
    onContextRestored(listener) {
      restoredListeners.add(listener)
      return () => {
        restoredListeners.delete(listener)
      }
    },
    dispose() {
      assertUsable('dispose')
      disposed = true
      canvas.removeEventListener('webglcontextlost', handleContextLost)
      canvas.removeEventListener('webglcontextrestored', handleContextRestored)
      lostListeners.clear()
      restoredListeners.clear()
      if (contextAvailable()) {
        deleteResources()
      } else {
        forgetResources()
      }
    },
  }
}
