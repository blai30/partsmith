import {
  ANTIALIAS_WIDTH,
  ELEMENT_TEXTURE_WIDTH,
  LINE_ATTRIBUTES,
  POINT_ATTRIBUTES,
  SURFACE_ATTRIBUTES,
} from './layout'

/** The programs of the backend: the background gradient and one per geometry kind. */
export type ProgramKind = 'background' | 'triangles' | 'fill' | 'lines' | 'points'
/** The color pass draws to the canvas; the ID pass writes pick ids and view depths (3.6). */
export type Pass = 'color' | 'id'

export type ShaderSources = { readonly vertex: string; readonly fragment: string }

// Bits of the element state byte (RenderBackend.setElementStates).
const STATE_BITS = `
const uint STATE_HOVER = 1u;
const uint STATE_SELECTED = 2u;
const uint STATE_PROBLEM = 4u;
`

const ELEMENT_LOOKUP = `
uniform highp usampler2D u_elementStates;
uniform highp sampler2D u_elementColors;
uniform bool u_hasElementColors;
uniform vec3 u_color;

ivec2 elementTexel(uint elementId) {
  return ivec2(int(elementId % ${String(ELEMENT_TEXTURE_WIDTH)}u), int(elementId / ${String(ELEMENT_TEXTURE_WIDTH)}u));
}

uint elementState(uint elementId) {
  return texelFetch(u_elementStates, elementTexel(elementId), 0).r;
}

vec3 elementColor(uint elementId) {
  return u_hasElementColors ? texelFetch(u_elementColors, elementTexel(elementId), 0).rgb : u_color;
}
`

const TRANSFORMS = `
uniform mat4 u_view;
uniform mat4 u_projection;
`

// Section clipping (3.15): removes the half-space where dot(normal, position) > offset.
const CLIPPING = `
uniform bool u_clipped;
uniform vec4 u_clipPlane;

void clipSection(vec3 worldPosition) {
  if (u_clipped && dot(u_clipPlane.xyz, worldPosition) > u_clipPlane.w) {
    discard;
  }
}
`

const COLOR_SPACE = `
vec3 encodeSrgb(vec3 linear) {
  vec3 clamped = clamp(linear, 0.0, 1.0);
  return mix(clamped * 12.92, 1.055 * pow(clamped, vec3(1.0 / 2.4)) - 0.055, step(0.0031308, clamped));
}

vec3 decodeSrgb(vec3 encoded) {
  return mix(encoded / 12.92, pow((encoded + 0.055) / 1.055, vec3(2.4)), step(0.04045, encoded));
}
`

const COLOR_OUTPUT = `
uniform vec3 u_highlight;
uniform vec3 u_problem;
uniform float u_opacity;
layout(location = 0) out vec4 o_color;
`

// Pick ids and the float bits of the view depth, each as RGBA8 bytes, least significant byte in red (encodePickId).
const ID_OUTPUT = `
uniform highp uint u_pickBase;
layout(location = 0) out vec4 o_pickId;
layout(location = 1) out vec4 o_depth;

vec4 bytesOf(uint value) {
  return vec4(uvec4(value, value >> 8u, value >> 16u, value >> 24u) & 255u) / 255.0;
}

void writePick(uint elementId, float viewDepth) {
  o_pickId = bytesOf(u_pickBase + elementId);
  o_depth = bytesOf(floatBitsToUint(viewDepth));
}
`

const SURFACE_VERTEX = `
layout(location = ${String(SURFACE_ATTRIBUTES.position)}) in vec3 a_position;
layout(location = ${String(SURFACE_ATTRIBUTES.normal)}) in vec3 a_normal;
layout(location = ${String(SURFACE_ATTRIBUTES.elementId)}) in uint a_elementId;
layout(location = ${String(SURFACE_ATTRIBUTES.scalar)}) in float a_scalar;

out vec3 v_worldPosition;
out vec3 v_viewNormal;
out float v_viewDepth;
out float v_scalar;
flat out uint v_elementId;

void main() {
  vec4 viewPosition = u_view * vec4(a_position, 1.0);
  v_worldPosition = a_position;
  v_viewNormal = mat3(u_view) * a_normal;
  v_viewDepth = -viewPosition.z;
  v_scalar = a_scalar;
  v_elementId = a_elementId;
  gl_Position = u_projection * viewPosition;
}
`

const SURFACE_INPUTS = `
in vec3 v_worldPosition;
in vec3 v_viewNormal;
in float v_viewDepth;
in float v_scalar;
flat in uint v_elementId;
`

// Matcap-style shading of 3.12 pass 2, the overhang ramp of pass 3 and the element state tints.
const TRIANGLES_COLOR = `
const vec3 LIGHT = normalize(vec3(0.3, 0.6, 0.74));

uniform bool u_matcap;
uniform bool u_sectionCaps;
uniform bool u_analysis;
uniform vec3 u_analysisLow;
uniform vec3 u_analysisHigh;
uniform float u_analysisThreshold;
uniform float u_analysisMaximum;

void main() {
  clipSection(v_worldPosition);
  vec3 base = elementColor(v_elementId);
  if (u_sectionCaps && !gl_FrontFacing) {
    o_color = vec4(encodeSrgb(base * 0.55), u_opacity);
    return;
  }
  vec3 normal = normalize(gl_FrontFacing ? v_viewNormal : -v_viewNormal);
  float facing = max(normal.z, 0.0);
  float lighting = 1.0;
  vec3 color = base;
  if (u_matcap) {
    lighting = 0.32 + 0.50 * max(dot(normal, LIGHT), 0.0) + 0.18 * facing;
    color = base * lighting + 0.06 * pow(1.0 - facing, 3.0);
  }
  if (u_analysis && v_scalar > u_analysisThreshold) {
    float span = max(u_analysisMaximum - u_analysisThreshold, 1e-6);
    float amount = clamp((v_scalar - u_analysisThreshold) / span, 0.0, 1.0);
    vec3 ramp = decodeSrgb(mix(u_analysisLow, u_analysisHigh, amount));
    color = mix(color, ramp * lighting, 0.85);
  }
  uint state = elementState(v_elementId);
  if ((state & STATE_PROBLEM) != 0u) {
    color = mix(color, u_problem, 0.5);
  }
  if ((state & STATE_SELECTED) != 0u) {
    color = mix(color, u_highlight, 0.35);
  } else if ((state & STATE_HOVER) != 0u) {
    color = mix(color, u_highlight, 0.2);
  }
  o_color = vec4(encodeSrgb(color), u_opacity);
}
`

// Region fills: flat, and in highlight at 35 percent when selected or 20 percent when hovered (3.12 pass 7).
const FILL_COLOR = `
void main() {
  clipSection(v_worldPosition);
  uint state = elementState(v_elementId);
  vec3 color = elementColor(v_elementId);
  float alpha = u_opacity;
  if ((state & STATE_PROBLEM) != 0u) {
    color = u_problem;
    alpha = max(alpha, 0.35);
  }
  if ((state & STATE_SELECTED) != 0u) {
    color = u_highlight;
    alpha = 0.35;
  } else if ((state & STATE_HOVER) != 0u) {
    color = u_highlight;
    alpha = 0.2;
  }
  if (alpha <= 0.0) {
    discard;
  }
  o_color = vec4(encodeSrgb(color), alpha);
}
`

const SURFACE_ID = `
void main() {
  clipSection(v_worldPosition);
  writePick(v_elementId, v_viewDepth);
}
`

// Screen-space segment quads (3.12 pass 4), clipped to the near plane, widened for hover and selection in the color pass.
// GLSL ES 3.00 has no noperspective, so the vertex shader writes the screen-space distances across and along the quad multiplied by clip w, together with w, and the fragment shader divides by the interpolated w. The distances then interpolate linearly on screen, which keeps the width and the dashes constant along a segment that recedes in depth.
const LINES_VERTEX = `
layout(location = ${String(LINE_ATTRIBUTES.start)}) in vec3 a_start;
layout(location = ${String(LINE_ATTRIBUTES.end)}) in vec3 a_end;
layout(location = ${String(LINE_ATTRIBUTES.elementId)}) in uint a_elementId;
layout(location = ${String(LINE_ATTRIBUTES.startDistance)}) in float a_startDistance;
layout(location = ${String(LINE_ATTRIBUTES.corner)}) in vec2 a_corner;

uniform vec2 u_viewportSize;
uniform float u_width;
uniform float u_emphasisWidth;

out vec3 v_worldPosition;
out float v_viewDepth;
out vec3 v_screenDistances;
flat out float v_halfWidth;
flat out uint v_elementId;

void main() {
  v_elementId = a_elementId;
  vec4 viewStart = u_view * vec4(a_start, 1.0);
  vec4 viewEnd = u_view * vec4(a_end, 1.0);
  vec4 clipStart = u_projection * viewStart;
  vec4 clipEnd = u_projection * viewEnd;
  float startInside = clipStart.z + clipStart.w;
  float endInside = clipEnd.z + clipEnd.w;
  if (startInside < 0.0 && endInside < 0.0) {
    gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
    return;
  }
  float startFraction = startInside < 0.0 ? startInside / (startInside - endInside) : 0.0;
  float endFraction = endInside < 0.0 ? startInside / (startInside - endInside) : 1.0;
  float fraction = mix(startFraction, endFraction, a_corner.x);
  vec4 clippedStart = mix(clipStart, clipEnd, startFraction);
  vec4 clippedEnd = mix(clipStart, clipEnd, endFraction);
  vec2 screenStart = (clippedStart.xy / clippedStart.w * 0.5 + 0.5) * u_viewportSize;
  vec2 screenEnd = (clippedEnd.xy / clippedEnd.w * 0.5 + 0.5) * u_viewportSize;
  vec2 delta = screenEnd - screenStart;
  float screenLength = length(delta);
  vec2 direction = screenLength > 1e-6 ? delta / screenLength : vec2(1.0, 0.0);
  vec2 side = vec2(-direction.y, direction.x);
#ifdef ID_PASS
  float halfWidth = u_width * 0.5;
  float reach = halfWidth;
#else
  uint state = elementState(a_elementId);
  bool emphasized = (state & (STATE_HOVER | STATE_SELECTED)) != 0u;
  float halfWidth = (emphasized ? max(u_width, u_emphasisWidth) : u_width) * 0.5;
  float reach = halfWidth + ${ANTIALIAS_WIDTH.toFixed(1)};
#endif
  vec2 screen = mix(screenStart, screenEnd, a_corner.x) + side * a_corner.y * reach + direction * (a_corner.x * 2.0 - 1.0) * reach;
  vec4 clip = mix(clipStart, clipEnd, fraction);
  float worldLength = length(a_end - a_start);
  float clippedWorldLength = max(worldLength * (endFraction - startFraction), 1e-9);
  v_worldPosition = mix(a_start, a_end, fraction);
  v_viewDepth = -mix(viewStart.z, viewEnd.z, fraction);
  float across = a_corner.y * reach;
  float along = (a_startDistance + worldLength * fraction) * (screenLength / clippedWorldLength);
  v_screenDistances = vec3(across, along, 1.0) * clip.w;
  v_halfWidth = halfWidth;
  gl_Position = vec4((screen / u_viewportSize * 2.0 - 1.0) * clip.w, clip.z, clip.w);
}
`

const LINES_INPUTS = `
in vec3 v_worldPosition;
in float v_viewDepth;
in vec3 v_screenDistances;
flat in float v_halfWidth;
flat in uint v_elementId;
`

// Anti-aliased with a 1-pixel smoothstep falloff; selected lines in highlight, hovered ones mixed toward it by u_hoverHighlight (3.6).
const LINES_COLOR = `
uniform bool u_dashed;
uniform float u_dashLength;
uniform float u_dashPeriod;
uniform float u_hoverHighlight;

void main() {
  clipSection(v_worldPosition);
  vec2 screenDistances = v_screenDistances.xy / v_screenDistances.z;
  float across = screenDistances.x;
  float along = screenDistances.y;
  if (u_dashed && mod(along, u_dashPeriod) > u_dashLength) {
    discard;
  }
  float coverage = 1.0 - smoothstep(v_halfWidth - 0.5, v_halfWidth + 0.5, abs(across));
  if (coverage <= 0.0) {
    discard;
  }
  uint state = elementState(v_elementId);
  vec3 color = elementColor(v_elementId);
  if ((state & STATE_PROBLEM) != 0u) {
    color = mix(color, u_problem, 0.5);
  }
  if ((state & STATE_SELECTED) != 0u) {
    color = u_highlight;
  } else if ((state & STATE_HOVER) != 0u) {
    color = mix(color, u_highlight, u_hoverHighlight);
  }
  o_color = vec4(encodeSrgb(color), coverage * u_opacity);
}
`

const LINES_ID = `
void main() {
  clipSection(v_worldPosition);
  writePick(v_elementId, v_viewDepth);
}
`

// Screen-space point quads: round dots in the color pass (hovered and selected ones at least 7 pixels), squares in the ID pass.
const POINTS_VERTEX = `
layout(location = ${String(POINT_ATTRIBUTES.center)}) in vec3 a_center;
layout(location = ${String(POINT_ATTRIBUTES.elementId)}) in uint a_elementId;
layout(location = ${String(POINT_ATTRIBUTES.corner)}) in vec2 a_corner;

uniform vec2 u_viewportSize;
uniform float u_size;
uniform float u_emphasisSize;

out vec3 v_worldPosition;
out float v_viewDepth;
out vec2 v_offset;
flat out float v_radius;
flat out uint v_elementId;

void main() {
  v_elementId = a_elementId;
  vec4 viewPosition = u_view * vec4(a_center, 1.0);
  vec4 clip = u_projection * viewPosition;
#ifdef ID_PASS
  float size = u_size;
  float reach = size * 0.5;
#else
  uint state = elementState(a_elementId);
  bool emphasized = (state & (STATE_HOVER | STATE_SELECTED)) != 0u;
  float size = emphasized ? max(u_size, u_emphasisSize) : u_size;
  float reach = size * 0.5 + ${ANTIALIAS_WIDTH.toFixed(1)};
#endif
  if (size <= 0.0 || clip.w <= 0.0) {
    gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
    return;
  }
  v_worldPosition = a_center;
  v_viewDepth = -viewPosition.z;
  v_offset = a_corner * reach;
  v_radius = size * 0.5;
  gl_Position = vec4(clip.xy + a_corner * reach / u_viewportSize * 2.0 * clip.w, clip.z, clip.w);
}
`

const POINTS_INPUTS = `
in vec3 v_worldPosition;
in float v_viewDepth;
in vec2 v_offset;
flat in float v_radius;
flat in uint v_elementId;
`

const POINTS_COLOR = `
void main() {
  clipSection(v_worldPosition);
  float coverage = 1.0 - smoothstep(v_radius - 0.5, v_radius + 0.5, length(v_offset));
  if (coverage <= 0.0) {
    discard;
  }
  uint state = elementState(v_elementId);
  vec3 color = elementColor(v_elementId);
  if ((state & STATE_PROBLEM) != 0u) {
    color = mix(color, u_problem, 0.5);
  }
  if ((state & (STATE_HOVER | STATE_SELECTED)) != 0u) {
    color = u_highlight;
  }
  o_color = vec4(encodeSrgb(color), coverage * u_opacity);
}
`

const POINTS_ID = `
void main() {
  clipSection(v_worldPosition);
  writePick(v_elementId, v_viewDepth);
}
`

// A full-screen triangle with the vertical background gradient (3.12 pass 1).
const BACKGROUND_VERTEX = `
out float v_height;

void main() {
  vec2 corner = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  v_height = corner.y;
  gl_Position = vec4(corner * 2.0 - 1.0, 0.0, 1.0);
}
`

const BACKGROUND_FRAGMENT = `
uniform vec3 u_backgroundTop;
uniform vec3 u_backgroundBottom;
in float v_height;
layout(location = 0) out vec4 o_color;

void main() {
  o_color = vec4(encodeSrgb(mix(u_backgroundBottom, u_backgroundTop, clamp(v_height, 0.0, 1.0))), 1.0);
}
`

const HEADER = `#version 300 es
precision highp float;
precision highp int;
`

function assemble(defines: readonly string[], ...chunks: readonly string[]): string {
  return [HEADER, ...defines.map((define) => `#define ${define}\n`), ...chunks].join('')
}

/** The vertex and fragment sources of a program for a pass. The background has only a color pass. */
export function shaderSources(kind: ProgramKind, pass: Pass): ShaderSources {
  const defines = pass === 'id' ? ['ID_PASS'] : []
  const fragmentOutput = pass === 'id' ? ID_OUTPUT : COLOR_OUTPUT
  switch (kind) {
    case 'background':
      if (pass === 'id') {
        throw new RangeError('The background has no ID pass.')
      }
      return {
        vertex: assemble(defines, BACKGROUND_VERTEX),
        fragment: assemble(defines, COLOR_SPACE, BACKGROUND_FRAGMENT),
      }
    case 'triangles':
    case 'fill':
      return {
        vertex: assemble(defines, TRANSFORMS, SURFACE_VERTEX),
        fragment: assemble(
          defines,
          STATE_BITS,
          ELEMENT_LOOKUP,
          CLIPPING,
          COLOR_SPACE,
          fragmentOutput,
          SURFACE_INPUTS,
          pass === 'id' ? SURFACE_ID : kind === 'triangles' ? TRIANGLES_COLOR : FILL_COLOR,
        ),
      }
    case 'lines':
      return {
        vertex: assemble(defines, STATE_BITS, ELEMENT_LOOKUP, TRANSFORMS, LINES_VERTEX),
        fragment: assemble(
          defines,
          STATE_BITS,
          ELEMENT_LOOKUP,
          CLIPPING,
          COLOR_SPACE,
          fragmentOutput,
          LINES_INPUTS,
          pass === 'id' ? LINES_ID : LINES_COLOR,
        ),
      }
    case 'points':
      return {
        vertex: assemble(defines, STATE_BITS, ELEMENT_LOOKUP, TRANSFORMS, POINTS_VERTEX),
        fragment: assemble(
          defines,
          STATE_BITS,
          ELEMENT_LOOKUP,
          CLIPPING,
          COLOR_SPACE,
          fragmentOutput,
          POINTS_INPUTS,
          pass === 'id' ? POINTS_ID : POINTS_COLOR,
        ),
      }
  }
}
