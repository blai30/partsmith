import { describe, expect, it } from 'vitest'
import { ELEMENT_STATE } from '../backend'
import {
  ELEMENT_TEXTURE_WIDTH,
  LINE_ATTRIBUTES,
  POINT_ATTRIBUTES,
  SURFACE_ATTRIBUTES,
} from './layout'
import { PROGRAMS, shaderSources } from './shaders'

function uniformDeclarations(source: string): string[] {
  return [...source.matchAll(/^uniform\s+(?:\w+\s+)*?(\w+)\s+(u_\w+);$/gm)].map(
    (match) => match[2] ?? '',
  )
}

function uniformTypes(source: string): Map<string, string> {
  return new Map(
    [...source.matchAll(/^uniform\s+(.*?)\s+(u_\w+);$/gm)].map((match) => [
      match[2] ?? '',
      match[1] ?? '',
    ]),
  )
}

function attributeLocations(source: string): Record<string, number> {
  return Object.fromEntries(
    [...source.matchAll(/layout\(location = (\d+)\) in \w+ (a_\w+);/g)].map((match) => [
      match[2] ?? '',
      Number(match[1]),
    ]),
  )
}

describe('shaderSources', () => {
  it.each(PROGRAMS)(
    '$kind $pass sources start with the GLSL ES 3.00 version line',
    ({ kind, pass }) => {
      const sources = shaderSources(kind, pass)
      expect(sources.vertex.startsWith('#version 300 es\n')).toBe(true)
      expect(sources.fragment.startsWith('#version 300 es\n')).toBe(true)
    },
  )

  it.each(PROGRAMS)(
    '$kind $pass declares every uniform it uses exactly once per shader',
    ({ kind, pass }) => {
      const sources = shaderSources(kind, pass)
      for (const source of [sources.vertex, sources.fragment]) {
        const declared = uniformDeclarations(source)
        expect(new Set(declared).size).toBe(declared.length)
        const used = new Set(source.match(/\bu_\w+/g) ?? [])
        expect([...used].filter((name) => !declared.includes(name))).toEqual([])
      }
    },
  )

  it.each(PROGRAMS)(
    '$kind $pass gives a uniform shared by both stages the same type',
    ({ kind, pass }) => {
      const sources = shaderSources(kind, pass)
      const vertexTypes = uniformTypes(sources.vertex)
      const mismatched = [...uniformTypes(sources.fragment)].filter(
        ([name, type]) => vertexTypes.has(name) && vertexTypes.get(name) !== type,
      )
      expect(mismatched).toEqual([])
    },
  )

  it('declares the bits of ELEMENT_STATE as its STATE constants', () => {
    for (const { kind, pass } of PROGRAMS.filter((program) => program.kind !== 'background')) {
      const { fragment } = shaderSources(kind, pass)
      for (const [name, bit] of Object.entries(ELEMENT_STATE)) {
        expect(fragment).toContain(`const uint STATE_${name.toUpperCase()} = ${String(bit)}u;`)
      }
    }
  })

  it('defines ID_PASS only in the ID pass', () => {
    expect(shaderSources('lines', 'id').vertex).toContain('#define ID_PASS\n')
    expect(shaderSources('lines', 'color').vertex).not.toContain('#define ID_PASS')
  })

  it('writes the pick id and the view depth to two targets in the ID pass', () => {
    for (const kind of ['triangles', 'fill', 'lines', 'points'] as const) {
      const fragment = shaderSources(kind, 'id').fragment
      expect(fragment).toContain('layout(location = 0) out vec4 o_pickId;')
      expect(fragment).toContain('layout(location = 1) out vec4 o_depth;')
      expect(fragment).toContain('writePick(v_elementId, v_viewDepth);')
      expect(fragment).toContain('clipSection(v_worldPosition);')
    }
  })

  it('writes one color target in the color pass', () => {
    for (const kind of ['background', 'triangles', 'fill', 'lines', 'points'] as const) {
      const fragment = shaderSources(kind, 'color').fragment
      expect(fragment.match(/out vec4 o_\w+;/g)).toEqual(['out vec4 o_color;'])
    }
  })

  it('has no ID pass for the background', () => {
    expect(() => shaderSources('background', 'id')).toThrow('The background has no ID pass.')
  })

  it('binds attributes at the locations of the buffer layout', () => {
    expect(attributeLocations(shaderSources('triangles', 'color').vertex)).toEqual({
      a_position: SURFACE_ATTRIBUTES.position,
      a_normal: SURFACE_ATTRIBUTES.normal,
      a_elementId: SURFACE_ATTRIBUTES.elementId,
      a_scalar: SURFACE_ATTRIBUTES.scalar,
    })
    expect(attributeLocations(shaderSources('lines', 'id').vertex)).toEqual({
      a_start: LINE_ATTRIBUTES.start,
      a_end: LINE_ATTRIBUTES.end,
      a_elementId: LINE_ATTRIBUTES.elementId,
      a_startDistance: LINE_ATTRIBUTES.startDistance,
      a_corner: LINE_ATTRIBUTES.corner,
    })
    expect(attributeLocations(shaderSources('points', 'color').vertex)).toEqual({
      a_center: POINT_ATTRIBUTES.center,
      a_elementId: POINT_ATTRIBUTES.elementId,
      a_corner: POINT_ATTRIBUTES.corner,
    })
  })

  it('looks element states up in rows of the element texture width', () => {
    expect(shaderSources('triangles', 'color').fragment).toContain(
      `int(elementId % ${String(ELEMENT_TEXTURE_WIDTH)}u)`,
    )
  })

  it('shades faces with the matcap formula, rim and state tints of 3.12', () => {
    const fragment = shaderSources('triangles', 'color').fragment
    expect(fragment).toContain('normalize(vec3(0.3, 0.6, 0.74))')
    expect(fragment).toContain(
      'lighting = 0.32 + 0.50 * max(dot(normal, LIGHT), 0.0) + 0.18 * facing;',
    )
    expect(fragment).toContain('color = base * lighting + 0.06 * pow(1.0 - facing, 3.0);')
    expect(fragment).toContain('encodeSrgb(base * 0.55)')
    expect(fragment).toContain('color = mix(color, ramp * lighting, 0.85);')
    expect(fragment).toContain('color = mixEncoded(color, u_problem, 0.5);')
    expect(fragment).toContain('color = mixEncoded(color, u_highlight, 0.35);')
    expect(fragment).toContain('color = mixEncoded(color, u_highlight, 0.2);')
  })

  it('tints face states in sRGB, so a selected face keeps the accent hue on screen (3.12)', () => {
    const fragment = shaderSources('triangles', 'color').fragment
    expect(fragment).toContain(
      'return decodeSrgb(mix(encodeSrgb(linear), encodeSrgb(target), amount));',
    )
    expect(fragment).not.toContain('mix(color, u_highlight')
  })

  it('interpolates the screen distances of line quads linearly on screen, not perspective-correctly', () => {
    const { vertex, fragment } = shaderSources('lines', 'color')
    expect(vertex).toContain('out vec3 v_screenDistances;')
    expect(vertex).toContain('v_screenDistances = vec3(across, along, 1.0) * clip.w;')
    expect(vertex).toContain(
      'gl_Position = vec4((screen / u_viewportSize * 2.0 - 1.0) * clip.w, clip.z, clip.w);',
    )
    expect(fragment).toContain('vec2 screenDistances = v_screenDistances.xy / v_screenDistances.z;')
    expect(fragment).toContain('abs(across)')
    expect(fragment).toContain('mod(along, u_dashPeriod)')
    expect(`${vertex}${fragment}`).not.toMatch(/\bv_(across|along)\b/)
  })

  it('mixes hovered lines toward the highlight by u_hoverHighlight and selected lines fully', () => {
    const fragment = shaderSources('lines', 'color').fragment
    expect(fragment).toContain('uniform float u_hoverHighlight;')
    expect(fragment).toContain('color = mix(color, u_highlight, u_hoverHighlight);')
    expect(fragment).toContain('color = u_highlight;')
    expect(fragment).not.toContain('0.7')
  })

  it('mixes the analysis ramp in sRGB and linearizes the result', () => {
    expect(shaderSources('triangles', 'color').fragment).toContain(
      'vec3 ramp = decodeSrgb(mix(u_analysisLow, u_analysisHigh, amount));',
    )
  })

  it('writes the background gradient from bottom to top', () => {
    expect(shaderSources('background', 'color').fragment).toContain(
      'mix(u_backgroundBottom, u_backgroundTop, clamp(v_height, 0.0, 1.0))',
    )
  })
})
