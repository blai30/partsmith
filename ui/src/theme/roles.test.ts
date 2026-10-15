import colors from 'tailwindcss/colors'
import { describe, expect, it } from 'vitest'
import {
  AXIS_ROLES,
  BODY_SWATCHES,
  COLOR_CLASSES,
  contrastRatio,
  encodedFromHex,
  FONT_CLASSES,
  linearFromHex,
  PALETTE_HEX,
  ROOT_CLASSES,
  UI_SCALE_CLASSES,
  VIEWPORT_COLORS,
  VIEWPORT_STEPS,
  viewportTheme,
  type PaletteStep,
  type ThemeName,
  type ViewportRole,
} from './roles'

const THEMES: readonly ThemeName[] = ['light', 'dark']

// Table of ui.md 2.16.5: step, Tailwind 4.3.3 value, sRGB hex.
const PALETTE_TABLE: readonly (readonly [step: string, tailwind: string, hex: string])[] = [
  ['white', '#fff', '#ffffff'],
  ['zinc-50', 'oklch(98.5% 0 none)', '#fafafa'],
  ['zinc-100', 'oklch(96.7% 0.001 286.375)', '#f4f4f5'],
  ['zinc-200', 'oklch(92% 0.004 286.32)', '#e4e4e7'],
  ['zinc-300', 'oklch(87.1% 0.006 286.286)', '#d4d4d8'],
  ['zinc-400', 'oklch(70.5% 0.015 286.067)', '#9f9fa9'],
  ['zinc-500', 'oklch(55.2% 0.016 285.938)', '#71717b'],
  ['zinc-600', 'oklch(44.2% 0.017 285.786)', '#52525c'],
  ['zinc-800', 'oklch(27.4% 0.006 286.033)', '#27272a'],
  ['zinc-900', 'oklch(21% 0.006 285.885)', '#18181b'],
  ['zinc-950', 'oklch(14.1% 0.005 285.823)', '#09090b'],
  ['orange-100', 'oklch(95.4% 0.038 75.164)', '#ffedd4'],
  ['orange-400', 'oklch(75% 0.183 55.934)', '#ff8904'],
  ['orange-500', 'oklch(70.5% 0.213 47.604)', '#ff6900'],
  ['orange-600', 'oklch(64.6% 0.222 41.116)', '#f54900'],
  ['orange-950', 'oklch(26.6% 0.079 36.259)', '#441306'],
  ['yellow-50', 'oklch(98.7% 0.026 102.212)', '#fefce8'],
  ['yellow-300', 'oklch(90.5% 0.182 98.111)', '#ffdf20'],
  ['yellow-400', 'oklch(85.2% 0.199 91.936)', '#fdc700'],
  ['yellow-800', 'oklch(47.6% 0.114 61.907)', '#894b00'],
  ['yellow-950', 'oklch(28.6% 0.066 53.813)', '#432004'],
  ['red-50', 'oklch(97.1% 0.013 17.38)', '#fef2f2'],
  ['red-300', 'oklch(80.8% 0.114 19.571)', '#ffa2a2'],
  ['red-500', 'oklch(63.7% 0.237 25.331)', '#fb2c36'],
  ['red-600', 'oklch(57.7% 0.245 27.325)', '#e7000b'],
  ['red-700', 'oklch(50.5% 0.213 27.518)', '#c10007'],
  ['red-950', 'oklch(25.8% 0.092 26.042)', '#460809'],
  ['rose-400', 'oklch(71.2% 0.194 13.428)', '#ff637e'],
  ['rose-600', 'oklch(58.6% 0.253 17.585)', '#ec003f'],
  ['lime-400', 'oklch(84.1% 0.238 128.85)', '#9ae600'],
  ['green-400', 'oklch(79.2% 0.209 151.711)', '#05df72'],
  ['green-600', 'oklch(62.7% 0.194 149.214)', '#00a63e'],
  ['green-700', 'oklch(52.7% 0.154 150.069)', '#008236'],
  ['emerald-700', 'oklch(50.8% 0.118 165.612)', '#007a55'],
  ['teal-600', 'oklch(60% 0.118 184.704)', '#009689'],
  ['sky-200', 'oklch(90.1% 0.058 230.902)', '#b8e6fe'],
  ['sky-300', 'oklch(82.8% 0.111 230.318)', '#74d4ff'],
  ['sky-700', 'oklch(50% 0.134 242.749)', '#0069a8'],
  ['blue-400', 'oklch(70.7% 0.165 254.624)', '#51a2ff'],
  ['blue-500', 'oklch(62.3% 0.214 259.815)', '#2b7fff'],
  ['blue-600', 'oklch(54.6% 0.245 262.881)', '#155dfc'],
  ['violet-700', 'oklch(49.1% 0.27 292.581)', '#7008e7'],
  ['purple-600', 'oklch(55.8% 0.288 302.321)', '#9810fa'],
  ['pink-600', 'oklch(59.2% 0.249 0.584)', '#e60076'],
  ['amber-800', 'oklch(47.3% 0.137 46.201)', '#973c00'],
]

// Table of ui.md 2.16.3: role, facet, classes.
const CLASS_TABLE: readonly (readonly [role: string, facet: string, classes: string])[] = [
  ['surface', 'background', 'bg-zinc-100 dark:bg-zinc-950'],
  ['surfaceRaised', 'background', 'bg-white dark:bg-zinc-900'],
  ['field', 'background', 'bg-white dark:bg-zinc-950'],
  ['hover', 'background', 'hover:bg-zinc-200 dark:hover:bg-zinc-800'],
  ['hover', 'active', 'bg-zinc-200 dark:bg-zinc-800'],
  ['selection', 'background', 'bg-orange-100 dark:bg-orange-950'],
  ['border', 'border', 'border-zinc-200 dark:border-zinc-800'],
  ['borderControl', 'border', 'border-zinc-500'],
  ['text', 'foreground', 'text-zinc-900 dark:text-zinc-100'],
  ['textMuted', 'foreground', 'text-zinc-600 dark:text-zinc-400'],
  ['textMuted', 'placeholder', 'placeholder:text-zinc-600 dark:placeholder:text-zinc-400'],
  ['textDisabled', 'foreground', 'text-zinc-400 dark:text-zinc-600'],
  ['accent', 'background', 'bg-orange-600 dark:bg-orange-500'],
  ['accent', 'foreground', 'text-zinc-950'],
  ['accent', 'border', 'border-orange-600 dark:border-orange-500'],
  ['accent', 'outline', 'outline-orange-600 dark:outline-orange-500'],
  ['accent', 'stroke', 'stroke-orange-600 dark:stroke-orange-500'],
  ['accentHover', 'background', 'hover:bg-orange-500 dark:hover:bg-orange-400'],
  [
    'focusRing',
    'outline',
    'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-orange-600 dark:focus-visible:outline-orange-500',
  ],
  ['errorMark', 'foreground', 'text-red-700 dark:text-red-500'],
  ['errorMark', 'border', 'border-red-700 dark:border-red-500'],
  ['errorMark', 'stroke', 'stroke-red-700 dark:stroke-red-500'],
  ['errorText', 'foreground', 'text-red-700 dark:text-red-300'],
  ['errorSubtle', 'background', 'bg-red-50 dark:bg-red-950'],
  ['errorFill', 'background', 'bg-red-600'],
  ['errorFill', 'foreground', 'text-white'],
  ['warningMark', 'fill', 'fill-yellow-400'],
  ['warningMark', 'foreground', 'text-zinc-950'],
  ['warningText', 'foreground', 'text-yellow-800 dark:text-yellow-300'],
  ['warningSubtle', 'background', 'bg-yellow-50 dark:bg-yellow-950'],
  ['warningFill', 'background', 'bg-yellow-400'],
  ['warningFill', 'foreground', 'text-zinc-950'],
  ['successMark', 'foreground', 'text-green-700 dark:text-green-400'],
  ['analysis', 'background', 'bg-linear-to-r/srgb from-sky-200 to-violet-700'],
  ['axisX', 'foreground', 'text-rose-600 dark:text-rose-400'],
  ['axisX', 'border', 'border-rose-600 dark:border-rose-400'],
  ['axisY', 'foreground', 'text-emerald-700 dark:text-lime-400'],
  ['axisY', 'border', 'border-emerald-700 dark:border-lime-400'],
  ['axisZ', 'foreground', 'text-sky-700 dark:text-sky-300'],
  ['axisZ', 'border', 'border-sky-700 dark:border-sky-300'],
]

// Table of ui.md 2.16.4: role, light step, dark step, light hex, dark hex.
const VIEWPORT_TABLE: readonly (readonly [
  role: ViewportRole,
  light: PaletteStep,
  dark: PaletteStep,
  lightHex: string,
  darkHex: string,
])[] = [
  ['viewportTop', 'white', 'zinc-800', '#ffffff', '#27272a'],
  ['viewportBottom', 'zinc-100', 'zinc-900', '#f4f4f5', '#18181b'],
  ['body', 'zinc-400', 'zinc-400', '#9f9fa9', '#9f9fa9'],
  ['edgeDark', 'zinc-900', 'zinc-900', '#18181b', '#18181b'],
  ['edgeLight', 'zinc-200', 'zinc-200', '#e4e4e7', '#e4e4e7'],
  ['highlight', 'orange-600', 'orange-500', '#f54900', '#ff6900'],
  ['problem', 'red-700', 'red-500', '#c10007', '#fb2c36'],
  ['sketchUnder', 'blue-600', 'blue-400', '#155dfc', '#51a2ff'],
  ['sketchFull', 'zinc-900', 'zinc-50', '#18181b', '#fafafa'],
  ['sketchOver', 'red-700', 'red-500', '#c10007', '#fb2c36'],
  ['sketchConstruction', 'zinc-600', 'zinc-300', '#52525c', '#d4d4d8'],
  ['sketchRegion', 'blue-500', 'blue-400', '#2b7fff', '#51a2ff'],
  ['sketchReference', 'zinc-500', 'zinc-400', '#71717b', '#9f9fa9'],
  ['datum', 'zinc-400', 'zinc-500', '#9f9fa9', '#71717b'],
  ['bed', 'zinc-500', 'zinc-400', '#71717b', '#9f9fa9'],
  ['grid', 'zinc-300', 'zinc-600', '#d4d4d8', '#52525c'],
  ['analysisLow', 'sky-200', 'sky-200', '#b8e6fe', '#b8e6fe'],
  ['analysisHigh', 'violet-700', 'violet-700', '#7008e7', '#7008e7'],
  ['axisX', 'rose-600', 'rose-400', '#ec003f', '#ff637e'],
  ['axisY', 'emerald-700', 'lime-400', '#007a55', '#9ae600'],
  ['axisZ', 'sky-700', 'sky-300', '#0069a8', '#74d4ff'],
]

// Swatch table of ui.md 2.16.5: name, step, stored Body.color.
const SWATCH_TABLE: readonly (readonly [name: string, step: PaletteStep, hex: string | null])[] = [
  ['Default', 'zinc-400', null],
  ['White', 'zinc-100', '#f4f4f5'],
  ['Black', 'zinc-800', '#27272a'],
  ['Gray', 'zinc-500', '#71717b'],
  ['Red', 'red-600', '#e7000b'],
  ['Orange', 'orange-500', '#ff6900'],
  ['Yellow', 'yellow-400', '#fdc700'],
  ['Green', 'green-600', '#00a63e'],
  ['Teal', 'teal-600', '#009689'],
  ['Blue', 'blue-600', '#155dfc'],
  ['Purple', 'purple-600', '#9810fa'],
  ['Pink', 'pink-600', '#e60076'],
  ['Brown', 'amber-800', '#973c00'],
]

// Text table of ui.md 2.16.6: text role, background role, light ratio, dark ratio.
const TEXT_CONTRAST: readonly (readonly [text: string, on: string, light: number, dark: number])[] =
  [
    ['text', 'surface', 16.11, 18.1],
    ['text', 'surfaceRaised', 17.71, 16.11],
    ['text', 'field', 17.71, 18.1],
    ['text', 'hover', 13.96, 13.55],
    ['text', 'selection', 15.45, 14.24],
    ['text', 'errorSubtle', 16.19, 14.7],
    ['text', 'warningSubtle', 17.12, 13.2],
    ['textMuted', 'surface', 7.02, 7.58],
    ['textMuted', 'surfaceRaised', 7.71, 6.75],
    ['textMuted', 'field', 7.71, 7.58],
    ['textMuted', 'hover', 6.08, 5.67],
    ['textMuted', 'selection', 6.73, 5.96],
    ['errorText', 'surface', 5.84, 10.36],
    ['errorText', 'surfaceRaised', 6.42, 9.23],
    ['errorText', 'errorSubtle', 5.86, 8.42],
    ['warningText', 'surface', 6.22, 14.99],
    ['warningText', 'surfaceRaised', 6.84, 13.35],
    ['warningText', 'warningSubtle', 6.61, 10.93],
    ['onAccent', 'accent', 5.52, 6.89],
    ['onAccent', 'accentHover', 6.89, 8.36],
    ['onWarningFill', 'warningFill', 12.64, 12.64],
    ['onErrorFill', 'errorFill', 4.76, 4.76],
    ['axisX', 'field', 4.52, 6.95],
    ['axisX', 'surfaceRaised', 4.52, 6.19],
    ['axisY', 'field', 5.36, 12.97],
    ['axisY', 'surfaceRaised', 5.36, 11.55],
    ['axisZ', 'field', 5.85, 11.94],
    ['axisZ', 'surfaceRaised', 5.85, 10.63],
  ]

// Non-text table of ui.md 2.16.6: mark role, background role, light ratio, dark ratio.
const NON_TEXT_CONTRAST: readonly (readonly [
  mark: string,
  against: string,
  light: number,
  dark: number,
])[] = [
  ['borderControl', 'surface', 4.39, 4.12],
  ['borderControl', 'surfaceRaised', 4.82, 3.67],
  ['borderControl', 'field', 4.82, 4.12],
  ['accent', 'surface', 3.27, 6.89],
  ['accent', 'surfaceRaised', 3.59, 6.13],
  ['accent', 'selection', 3.13, 5.42],
  ['accent', 'viewportTop', 3.59, 5.15],
  ['accent', 'viewportBottom', 3.27, 6.13],
  ['errorMark', 'surface', 5.84, 5.22],
  ['errorMark', 'surfaceRaised', 6.42, 4.65],
  ['errorMark', 'viewportTop', 6.42, 3.91],
  ['errorMark', 'viewportBottom', 5.84, 4.65],
  ['successMark', 'surface', 4.49, 11.18],
  ['successMark', 'surfaceRaised', 4.94, 9.95],
  ['warningMark', 'surface', 18.1, 12.64],
  ['warningMark', 'surfaceRaised', 19.89, 11.25],
  ['warningMark', 'hover', 15.67, 9.46],
  ['warningMark', 'selection', 17.35, 9.94],
  ['sketchUnder', 'viewportTop', 5.24, 5.64],
  ['sketchUnder', 'viewportBottom', 4.77, 6.71],
  ['sketchFull', 'viewportTop', 17.71, 14.27],
  ['sketchFull', 'viewportBottom', 16.11, 16.97],
  ['sketchOver', 'viewportTop', 6.42, 3.91],
  ['sketchOver', 'viewportBottom', 5.84, 4.65],
  ['sketchConstruction', 'viewportTop', 7.71, 10.07],
  ['sketchConstruction', 'viewportBottom', 7.02, 11.98],
  ['sketchReference', 'viewportTop', 4.82, 5.67],
  ['sketchReference', 'viewportBottom', 4.39, 6.75],
  ['bed', 'viewportTop', 4.82, 5.67],
  ['bed', 'viewportBottom', 4.39, 6.75],
  ['axisX', 'viewportTop', 4.52, 5.2],
  ['axisX', 'viewportBottom', 4.11, 6.19],
  ['axisX', 'surface', 4.11, 6.95],
  ['axisX', 'hover', 3.56, 5.2],
  ['axisX', 'selection', 3.94, 5.46],
  ['axisY', 'viewportTop', 5.36, 9.71],
  ['axisY', 'viewportBottom', 4.88, 11.55],
  ['axisY', 'surface', 4.88, 12.97],
  ['axisY', 'hover', 4.22, 9.71],
  ['axisY', 'selection', 4.67, 10.2],
  ['axisZ', 'viewportTop', 5.85, 8.93],
  ['axisZ', 'viewportBottom', 5.32, 10.63],
  ['axisZ', 'surface', 5.32, 11.94],
  ['axisZ', 'hover', 4.61, 8.93],
  ['axisZ', 'selection', 5.1, 9.39],
]

// Contrast-table names that are a facet of a 2.16.3 role, as role and facet.
const ROLE_FACETS: Readonly<Record<string, readonly [role: string, facet: string]>> = {
  surface: ['surface', 'background'],
  surfaceRaised: ['surfaceRaised', 'background'],
  field: ['field', 'background'],
  hover: ['hover', 'background'],
  selection: ['selection', 'background'],
  errorSubtle: ['errorSubtle', 'background'],
  warningSubtle: ['warningSubtle', 'background'],
  text: ['text', 'foreground'],
  textMuted: ['textMuted', 'foreground'],
  errorText: ['errorText', 'foreground'],
  warningText: ['warningText', 'foreground'],
  onAccent: ['accent', 'foreground'],
  accent: ['accent', 'background'],
  accentHover: ['accentHover', 'background'],
  onWarningFill: ['warningFill', 'foreground'],
  warningFill: ['warningFill', 'background'],
  onErrorFill: ['errorFill', 'foreground'],
  errorFill: ['errorFill', 'background'],
  borderControl: ['borderControl', 'border'],
  errorMark: ['errorMark', 'foreground'],
  successMark: ['successMark', 'foreground'],
}

// Tailwind palette values by step, such as 'zinc-100' or 'white'.
const TAILWIND_VALUES = new Map<string, string>()
for (const [family, value] of Object.entries(colors)) {
  if (typeof value === 'string') {
    TAILWIND_VALUES.set(family, value)
  } else {
    for (const [shade, color] of Object.entries(value)) {
      TAILWIND_VALUES.set(`${family}-${shade}`, color)
    }
  }
}
const COLOR_FAMILIES = Object.entries(colors)
  .filter(([, value]) => typeof value !== 'string')
  .map(([family]) => family)
const COLOR_UTILITY = new RegExp(
  `^(?:bg|text|border|outline|stroke|fill|from|via|to)-(white|black|(?:${COLOR_FAMILIES.join('|')})-\\d+)$`,
)

/** The palette steps a class string names for the light and the dark theme. */
function stepsOf(classes: string): { readonly light: string; readonly dark: string } {
  let light: string | null = null
  let dark: string | null = null
  for (const token of classes.split(' ')) {
    const parts = token.split(':')
    const match = COLOR_UTILITY.exec(parts.at(-1) ?? '')
    if (match?.[1] !== undefined) {
      if (parts.includes('dark')) {
        dark = match[1]
      } else {
        light = match[1]
      }
    }
  }
  if (light === null) {
    throw new Error(`No light color step in "${classes}".`)
  }
  return { light, dark: dark ?? light }
}

function tableClasses(role: string, facet: string): string {
  const row = CLASS_TABLE.find(([rowRole, rowFacet]) => rowRole === role && rowFacet === facet)
  if (row === undefined) {
    throw new Error(`No ${facet} facet for ${role} in the table.`)
  }
  return row[2]
}

/** The 2.16.5 step of a contrast-table role in a theme, from the tables of 2.16.3 and 2.16.4. */
function stepOfRole(name: string, theme: ThemeName): PaletteStep {
  const steps = (() => {
    if (name === 'warningMark') {
      return { light: 'zinc-950', dark: 'yellow-400' }
    }
    const viewport = VIEWPORT_TABLE.find(([role]) => role === name)
    if (viewport !== undefined) {
      return { light: viewport[1], dark: viewport[2] }
    }
    const facet = ROLE_FACETS[name]
    if (facet === undefined) {
      throw new Error(`Unknown role ${name}.`)
    }
    return stepsOf(tableClasses(facet[0], facet[1]))
  })()
  const step = theme === 'light' ? steps.light : steps.dark
  const known: readonly string[] = Object.keys(PALETTE_HEX)
  if (!known.includes(step)) {
    throw new Error(`PALETTE_HEX has no step ${step}.`)
  }
  return step as PaletteStep
}

function truncated(ratio: number): number {
  return Math.trunc(ratio * 100) / 100
}

/** CSS Color 4: OKLCH to OKLab to linear sRGB, clipped, sRGB-encoded and rounded to 8 bits. */
function hexFromTailwind(value: string): string {
  if (value === '#fff') {
    return '#ffffff'
  }
  const match = /^oklch\(([\d.]+)%\s+([\d.]+)\s+([\d.]+|none)\)$/.exec(value)
  if (match === null) {
    throw new Error(`Unexpected Tailwind value ${value}.`)
  }
  const lightness = Number(match[1]) / 100
  const chroma = Number(match[2])
  const hue = match[3] === 'none' ? 0 : (Number(match[3]) * Math.PI) / 180
  const greenRed = chroma * Math.cos(hue)
  const blueYellow = chroma * Math.sin(hue)
  const long = (lightness + 0.3963377773761749 * greenRed + 0.2158037573099136 * blueYellow) ** 3
  const medium = (lightness - 0.1055613458156586 * greenRed - 0.0638541728258133 * blueYellow) ** 3
  const short = (lightness - 0.0894841775298119 * greenRed - 1.2914855480194092 * blueYellow) ** 3
  const x = 1.2268798758459243 * long - 0.5578149944602171 * medium + 0.2813910456659647 * short
  const y = -0.0405757452148008 * long + 1.112286803280317 * medium - 0.0717110580655164 * short
  const z = -0.0763729366746601 * long - 0.4214933324022432 * medium + 1.5869240198367816 * short
  const linear = [
    (12831 / 3959) * x - (329 / 214) * y - (1974 / 3959) * z,
    (-851781 / 878810) * x + (1648619 / 878810) * y + (36519 / 878810) * z,
    (705 / 12673) * x - (2585 / 12673) * y + (705 / 667) * z,
  ]
  const channels = linear.map((channel) => {
    const clipped = Math.min(1, Math.max(0, channel))
    const encoded = clipped <= 0.0031308 ? 12.92 * clipped : 1.055 * clipped ** (1 / 2.4) - 0.055
    return Math.round(encoded * 255)
      .toString(16)
      .padStart(2, '0')
  })
  return `#${channels.join('')}`
}

/** Reference sRGB decoding of one 8-bit channel. */
function linearChannel(byte: number): number {
  const encoded = byte / 255
  return encoded <= 0.04045 ? encoded / 12.92 : ((encoded + 0.055) / 1.055) ** 2.4
}

/** The largest absolute difference between two vectors, or Infinity when their lengths differ. */
function maximumDifference(actual: readonly number[], expected: readonly number[]): number {
  if (actual.length !== expected.length) {
    return Number.POSITIVE_INFINITY
  }
  return Math.max(
    ...expected.map((value, index) => Math.abs((actual[index] ?? Number.NaN) - value)),
  )
}

describe('contrast (U-70)', () => {
  it('U-70 contrastRatio(#ffffff, #000000) is 21', () => {
    expect(contrastRatio('#ffffff', '#000000')).toBeCloseTo(21, 12)
  })

  it('is symmetric and 1 for equal colors', () => {
    expect(contrastRatio('#000000', '#ffffff')).toBeCloseTo(21, 12)
    expect(contrastRatio('#71717b', '#71717b')).toBeCloseTo(1, 12)
  })

  describe.each(THEMES)('%s theme', (theme) => {
    it.each(TEXT_CONTRAST)(
      'U-70 text %s on %s matches the table and is at least 4.5',
      (text, on, light, dark) => {
        const expected = theme === 'light' ? light : dark
        const ratio = contrastRatio(
          PALETTE_HEX[stepOfRole(text, theme)],
          PALETTE_HEX[stepOfRole(on, theme)],
        )
        expect(truncated(ratio)).toBe(expected)
        expect(truncated(ratio)).toBeGreaterThanOrEqual(4.5)
      },
    )

    it.each(NON_TEXT_CONTRAST)(
      'U-70 non-text %s against %s matches the table and is at least 3',
      (mark, against, light, dark) => {
        const expected = theme === 'light' ? light : dark
        const ratio = contrastRatio(
          PALETTE_HEX[stepOfRole(mark, theme)],
          PALETTE_HEX[stepOfRole(against, theme)],
        )
        expect(truncated(ratio)).toBe(expected)
        expect(truncated(ratio)).toBeGreaterThanOrEqual(3)
      },
    )
  })
})

describe('mapping tables (U-90)', () => {
  it('U-90 PALETTE_HEX has exactly the steps of 2.16.5 with their hex values', () => {
    expect(PALETTE_HEX).toEqual(
      Object.fromEntries(PALETTE_TABLE.map(([step, , hex]) => [step, hex])),
    )
  })

  it.each(PALETTE_TABLE)(
    'U-90 %s is %s in Tailwind 4.3.3 and converts to %s',
    (step, tailwind, hex) => {
      const value = TAILWIND_VALUES.get(step)
      expect(value).toBe(tailwind)
      expect(hexFromTailwind(value ?? '')).toBe(hex)
    },
  )

  it('U-90 every PALETTE_HEX value is the conversion of its step from tailwindcss/colors', () => {
    for (const [step, hex] of Object.entries(PALETTE_HEX)) {
      expect([step, hexFromTailwind(TAILWIND_VALUES.get(step) ?? '')]).toEqual([step, hex])
    }
  })

  it('U-90 COLOR_CLASSES equals the table of 2.16.3', () => {
    const expected: Record<string, Record<string, string>> = {}
    for (const [role, facet, classes] of CLASS_TABLE) {
      expected[role] = { ...expected[role], [facet]: classes }
    }
    expect(COLOR_CLASSES).toEqual(expected)
  })

  it('U-90 every color class in COLOR_CLASSES names a step of PALETTE_HEX', () => {
    const steps: string[] = []
    for (const facets of Object.values(COLOR_CLASSES)) {
      for (const classes of Object.values(facets)) {
        for (const token of classes.split(' ')) {
          const match = COLOR_UTILITY.exec(token.split(':').at(-1) ?? '')
          if (match?.[1] !== undefined) {
            steps.push(match[1])
          }
        }
      }
    }
    expect(steps.length).toBeGreaterThan(0)
    expect(Object.keys(PALETTE_HEX)).toEqual(expect.arrayContaining(steps))
  })

  it('U-90 VIEWPORT_STEPS equals the table of 2.16.4', () => {
    expect(VIEWPORT_STEPS).toEqual(
      Object.fromEntries(VIEWPORT_TABLE.map(([role, light, dark]) => [role, { light, dark }])),
    )
  })

  it.each(VIEWPORT_TABLE)(
    'U-90 VIEWPORT_COLORS of %s is PALETTE_HEX of its steps and the table hex',
    (role, light, dark, lightHex, darkHex) => {
      expect(VIEWPORT_COLORS.light[role]).toBe(PALETTE_HEX[VIEWPORT_STEPS[role].light])
      expect(VIEWPORT_COLORS.dark[role]).toBe(PALETTE_HEX[VIEWPORT_STEPS[role].dark])
      expect(VIEWPORT_COLORS.light[role]).toBe(lightHex)
      expect(VIEWPORT_COLORS.dark[role]).toBe(darkHex)
      expect(PALETTE_HEX[light]).toBe(lightHex)
      expect(PALETTE_HEX[dark]).toBe(darkHex)
    },
  )

  it('U-90 VIEWPORT_COLORS has exactly the viewport roles in both themes', () => {
    const roles = VIEWPORT_TABLE.map(([role]) => role).toSorted()
    expect(Object.keys(VIEWPORT_COLORS).toSorted()).toEqual(['dark', 'light'])
    expect(Object.keys(VIEWPORT_COLORS.light).toSorted()).toEqual(roles)
    expect(Object.keys(VIEWPORT_COLORS.dark).toSorted()).toEqual(roles)
  })

  it('U-90 highlight has the steps of the accent background', () => {
    expect(VIEWPORT_STEPS.highlight).toEqual(stepsOf(COLOR_CLASSES.accent.background))
  })

  it('U-90 problem and sketchOver have the steps of the errorMark foreground', () => {
    const errorMark = stepsOf(COLOR_CLASSES.errorMark.foreground)
    expect(VIEWPORT_STEPS.problem).toEqual(errorMark)
    expect(VIEWPORT_STEPS.sketchOver).toEqual(errorMark)
  })

  it('U-90 body has the step of the Default swatch in both themes', () => {
    const defaultSwatch = BODY_SWATCHES.find((swatch) => swatch.name === 'Default')
    expect(defaultSwatch).toBeDefined()
    expect(VIEWPORT_STEPS.body).toEqual({ light: defaultSwatch?.step, dark: defaultSwatch?.step })
  })

  it('U-90 the analysis classes name the steps of analysisLow and analysisHigh', () => {
    const tokens = COLOR_CLASSES.analysis.background.split(' ')
    for (const theme of THEMES) {
      expect(tokens).toContain(`from-${VIEWPORT_STEPS.analysisLow[theme]}`)
      expect(tokens).toContain(`to-${VIEWPORT_STEPS.analysisHigh[theme]}`)
    }
  })

  it('U-90 BODY_SWATCHES equals the swatch table of 2.16.5', () => {
    expect(BODY_SWATCHES).toEqual(
      SWATCH_TABLE.map(([name, step, hex]) => ({ name, step, background: `bg-${step}`, hex })),
    )
  })

  it('U-90 each non-null swatch hex is PALETTE_HEX of its step', () => {
    for (const swatch of BODY_SWATCHES) {
      expect(swatch.background).toBe(`bg-${swatch.step}`)
      expect(swatch.hex === null || swatch.hex === PALETTE_HEX[swatch.step]).toBe(true)
    }
  })

  it('U-90 viewportTheme(dark).highlight equals linearFromHex(#ff6900) within 1e-12', () => {
    expect(
      maximumDifference(viewportTheme('dark').highlight, linearFromHex('#ff6900')),
    ).toBeLessThan(1e-12)
  })

  it('U-90 FONT_CLASSES equal 2.16.7', () => {
    expect(FONT_CLASSES).toEqual({
      interfaceText: 'font-[Inter,ui-sans-serif,system-ui,sans-serif]',
      values: "font-['JetBrains_Mono',ui-monospace,monospace]",
    })
  })

  it('U-90 UI_SCALE_CLASSES equal 2.16.9', () => {
    expect(UI_SCALE_CLASSES).toEqual({
      1: 'text-[100%]',
      1.25: 'text-[125%]',
      1.5: 'text-[150%]',
      2: 'text-[200%]',
      3: 'text-[300%]',
    })
  })

  it('U-90 ROOT_CLASSES equals 2.16.2', () => {
    expect(ROOT_CLASSES).toBe('scheme-light dark:scheme-dark')
  })
})

describe('axis roles (U-179)', () => {
  it('U-179 AXIS_ROLES names the role of each world axis', () => {
    expect(AXIS_ROLES).toEqual({ X: 'axisX', Y: 'axisY', Z: 'axisZ' })
  })

  it('U-179 the axis roles have a foreground and a border facet in rose, emerald or lime, and sky', () => {
    expect(COLOR_CLASSES.axisX).toEqual({
      foreground: 'text-rose-600 dark:text-rose-400',
      border: 'border-rose-600 dark:border-rose-400',
    })
    expect(COLOR_CLASSES.axisY).toEqual({
      foreground: 'text-emerald-700 dark:text-lime-400',
      border: 'border-emerald-700 dark:border-lime-400',
    })
    expect(COLOR_CLASSES.axisZ).toEqual({
      foreground: 'text-sky-700 dark:text-sky-300',
      border: 'border-sky-700 dark:border-sky-300',
    })
  })

  it.each(Object.values(AXIS_ROLES))(
    'U-179 the viewport role %s names the steps of the interface role',
    (role) => {
      expect(VIEWPORT_STEPS[role]).toEqual(stepsOf(COLOR_CLASSES[role].foreground))
      expect(VIEWPORT_STEPS[role]).toEqual(stepsOf(COLOR_CLASSES[role].border))
    },
  )
})

describe('color conversions', () => {
  it('linearFromHex decodes sRGB per channel', () => {
    expect(maximumDifference(linearFromHex('#ffffff'), [1, 1, 1])).toBeLessThan(1e-12)
    expect(maximumDifference(linearFromHex('#000000'), [0, 0, 0])).toBeLessThan(1e-12)
    expect(
      maximumDifference(linearFromHex('#ff8004'), [
        linearChannel(255),
        linearChannel(128),
        linearChannel(4),
      ]),
    ).toBeLessThan(1e-12)
  })

  it('linearFromHex uses the linear segment at and below the 0.04045 threshold', () => {
    expect(
      maximumDifference(linearFromHex('#0a0a0a'), [
        10 / 255 / 12.92,
        10 / 255 / 12.92,
        10 / 255 / 12.92,
      ]),
    ).toBeLessThan(1e-12)
  })

  it('encodedFromHex divides each channel by 255', () => {
    expect(maximumDifference(encodedFromHex('#ff8000'), [1, 128 / 255, 0])).toBeLessThan(1e-12)
    expect(
      maximumDifference(encodedFromHex('#b8e6fe'), [184 / 255, 230 / 255, 254 / 255]),
    ).toBeLessThan(1e-12)
  })

  it.each(THEMES)('viewportTheme(%s) linearizes the six backend roles', (theme) => {
    const backend = viewportTheme(theme)
    const colorsOfTheme = VIEWPORT_COLORS[theme]
    expect(
      maximumDifference(backend.backgroundTop, linearFromHex(colorsOfTheme.viewportTop)),
    ).toBeLessThan(1e-12)
    expect(
      maximumDifference(backend.backgroundBottom, linearFromHex(colorsOfTheme.viewportBottom)),
    ).toBeLessThan(1e-12)
    expect(
      maximumDifference(backend.highlight, linearFromHex(colorsOfTheme.highlight)),
    ).toBeLessThan(1e-12)
    expect(maximumDifference(backend.problem, linearFromHex(colorsOfTheme.problem))).toBeLessThan(
      1e-12,
    )
    expect(maximumDifference(backend.edgeDark, linearFromHex(colorsOfTheme.edgeDark))).toBeLessThan(
      1e-12,
    )
    expect(
      maximumDifference(backend.edgeLight, linearFromHex(colorsOfTheme.edgeLight)),
    ).toBeLessThan(1e-12)
  })
})
