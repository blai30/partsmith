import type { WorldAxis } from '../core/labels'
import type { Settings } from '../settings/settings'
import type { EncodedRgb, Rgb, ViewportTheme } from '../viewport/backend'

// The documented role-to-color mapping of ui.md 2.16: the only place that names palette steps.
// Every class string is a complete literal so Tailwind's scanner finds it.

export type ThemeName = 'light' | 'dark'
export type Facet =
  | 'background'
  | 'active'
  | 'foreground'
  | 'placeholder'
  | 'border'
  | 'outline'
  | 'stroke'
  | 'fill'

/** sRGB '#rrggbb' of every Tailwind 4.3.3 palette step the mapping uses (ui.md 2.16.5). */
export const PALETTE_HEX = {
  white: '#ffffff',
  'zinc-50': '#fafafa',
  'zinc-100': '#f4f4f5',
  'zinc-200': '#e4e4e7',
  'zinc-300': '#d4d4d8',
  'zinc-400': '#9f9fa9',
  'zinc-500': '#71717b',
  'zinc-600': '#52525c',
  'zinc-800': '#27272a',
  'zinc-900': '#18181b',
  'zinc-950': '#09090b',
  'orange-100': '#ffedd4',
  'orange-400': '#ff8904',
  'orange-500': '#ff6900',
  'orange-600': '#f54900',
  'orange-950': '#441306',
  'yellow-50': '#fefce8',
  'yellow-300': '#ffdf20',
  'yellow-400': '#fdc700',
  'yellow-800': '#894b00',
  'yellow-950': '#432004',
  'red-50': '#fef2f2',
  'red-300': '#ffa2a2',
  'red-500': '#fb2c36',
  'red-600': '#e7000b',
  'red-700': '#c10007',
  'red-950': '#460809',
  'rose-400': '#ff637e',
  'rose-600': '#ec003f',
  'lime-400': '#9ae600',
  'green-400': '#05df72',
  'green-600': '#00a63e',
  'green-700': '#008236',
  'emerald-700': '#007a55',
  'teal-600': '#009689',
  'sky-200': '#b8e6fe',
  'sky-300': '#74d4ff',
  'sky-700': '#0069a8',
  'blue-400': '#51a2ff',
  'blue-500': '#2b7fff',
  'blue-600': '#155dfc',
  'violet-700': '#7008e7',
  'purple-600': '#9810fa',
  'pink-600': '#e60076',
  'amber-800': '#973c00',
} as const

/** A palette step the mapping uses, such as 'zinc-900' or 'white'. */
export type PaletteStep = keyof typeof PALETTE_HEX

/** Interface roles and their facets as literal class strings (ui.md 2.16.3). */
export const COLOR_CLASSES = {
  surface: { background: 'bg-zinc-100 dark:bg-zinc-950' },
  surfaceRaised: { background: 'bg-white dark:bg-zinc-900' },
  field: { background: 'bg-white dark:bg-zinc-950' },
  hover: {
    background: 'hover:bg-zinc-200 dark:hover:bg-zinc-800',
    active: 'bg-zinc-200 dark:bg-zinc-800',
  },
  selection: { background: 'bg-orange-100 dark:bg-orange-950' },
  border: { border: 'border-zinc-200 dark:border-zinc-800' },
  borderControl: { border: 'border-zinc-500' },
  text: { foreground: 'text-zinc-900 dark:text-zinc-100' },
  textMuted: {
    foreground: 'text-zinc-600 dark:text-zinc-400',
    placeholder: 'placeholder:text-zinc-600 dark:placeholder:text-zinc-400',
  },
  textDisabled: { foreground: 'text-zinc-400 dark:text-zinc-600' },
  accent: {
    background: 'bg-orange-600 dark:bg-orange-500',
    foreground: 'text-zinc-950',
    border: 'border-orange-600 dark:border-orange-500',
    outline: 'outline-orange-600 dark:outline-orange-500',
    stroke: 'stroke-orange-600 dark:stroke-orange-500',
  },
  accentHover: { background: 'hover:bg-orange-500 dark:hover:bg-orange-400' },
  focusRing: {
    outline:
      'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-orange-600 dark:focus-visible:outline-orange-500',
  },
  errorMark: {
    foreground: 'text-red-700 dark:text-red-500',
    border: 'border-red-700 dark:border-red-500',
    stroke: 'stroke-red-700 dark:stroke-red-500',
  },
  errorText: { foreground: 'text-red-700 dark:text-red-300' },
  errorSubtle: { background: 'bg-red-50 dark:bg-red-950' },
  errorFill: { background: 'bg-red-600', foreground: 'text-white' },
  warningMark: { fill: 'fill-yellow-400', foreground: 'text-zinc-950' },
  warningText: { foreground: 'text-yellow-800 dark:text-yellow-300' },
  warningSubtle: { background: 'bg-yellow-50 dark:bg-yellow-950' },
  warningFill: { background: 'bg-yellow-400', foreground: 'text-zinc-950' },
  successMark: { foreground: 'text-green-700 dark:text-green-400' },
  analysis: { background: 'bg-linear-to-r/srgb from-sky-200 to-violet-700' },
  axisX: {
    foreground: 'text-rose-600 dark:text-rose-400',
    border: 'border-rose-600 dark:border-rose-400',
  },
  axisY: {
    foreground: 'text-emerald-700 dark:text-lime-400',
    border: 'border-emerald-700 dark:border-lime-400',
  },
  axisZ: {
    foreground: 'text-sky-700 dark:text-sky-300',
    border: 'border-sky-700 dark:border-sky-300',
  },
} as const satisfies Readonly<Record<string, Readonly<Partial<Record<Facet, string>>>>>

export type ColorRole = keyof typeof COLOR_CLASSES

/** The role of each world axis, in the interface (ui.md 2.16.3) and in the viewport (2.16.4). */
export const AXIS_ROLES = {
  X: 'axisX',
  Y: 'axisY',
  Z: 'axisZ',
} as const satisfies Readonly<Record<WorldAxis, ColorRole>>

export type ViewportRole =
  | 'viewportTop'
  | 'viewportBottom'
  | 'body'
  | 'edgeDark'
  | 'edgeLight'
  | 'highlight'
  | 'problem'
  | 'sketchUnder'
  | 'sketchFull'
  | 'sketchOver'
  | 'sketchConstruction'
  | 'sketchRegion'
  | 'sketchReference'
  | 'datum'
  | 'bed'
  | 'grid'
  | 'analysisLow'
  | 'analysisHigh'
  | 'axisX'
  | 'axisY'
  | 'axisZ'

/** Palette step of each viewport role per theme (ui.md 2.16.4). */
export const VIEWPORT_STEPS: Readonly<
  Record<ViewportRole, Readonly<Record<ThemeName, PaletteStep>>>
> = {
  viewportTop: { light: 'white', dark: 'zinc-800' },
  viewportBottom: { light: 'zinc-100', dark: 'zinc-900' },
  body: { light: 'zinc-400', dark: 'zinc-400' },
  edgeDark: { light: 'zinc-900', dark: 'zinc-900' },
  edgeLight: { light: 'zinc-200', dark: 'zinc-200' },
  highlight: { light: 'orange-600', dark: 'orange-500' },
  problem: { light: 'red-700', dark: 'red-500' },
  sketchUnder: { light: 'blue-600', dark: 'blue-400' },
  sketchFull: { light: 'zinc-900', dark: 'zinc-50' },
  sketchOver: { light: 'red-700', dark: 'red-500' },
  sketchConstruction: { light: 'zinc-600', dark: 'zinc-300' },
  sketchRegion: { light: 'blue-500', dark: 'blue-400' },
  sketchReference: { light: 'zinc-500', dark: 'zinc-400' },
  datum: { light: 'zinc-400', dark: 'zinc-500' },
  bed: { light: 'zinc-500', dark: 'zinc-400' },
  grid: { light: 'zinc-300', dark: 'zinc-600' },
  analysisLow: { light: 'sky-200', dark: 'sky-200' },
  analysisHigh: { light: 'violet-700', dark: 'violet-700' },
  axisX: { light: 'rose-600', dark: 'rose-400' },
  axisY: { light: 'emerald-700', dark: 'lime-400' },
  axisZ: { light: 'sky-700', dark: 'sky-300' },
}

function viewportColorsOf(theme: ThemeName): Readonly<Record<ViewportRole, string>> {
  const entries = Object.entries(VIEWPORT_STEPS).map(([role, steps]) => [
    role,
    PALETTE_HEX[steps[theme]],
  ])
  return Object.freeze(Object.fromEntries(entries) as Record<ViewportRole, string>)
}

/** '#rrggbb' of each viewport role per theme: PALETTE_HEX[VIEWPORT_STEPS[role][theme]]. */
export const VIEWPORT_COLORS: Readonly<Record<ThemeName, Readonly<Record<ViewportRole, string>>>> =
  {
    light: viewportColorsOf('light'),
    dark: viewportColorsOf('dark'),
  }

export type BodySwatch = {
  readonly name: string
  readonly step: PaletteStep
  readonly background: string
  readonly hex: string | null
}

/** A colored swatch: its palette step, as a full background class and as the stored `Body.color`. */
function swatch(name: string, step: PaletteStep, background: string): BodySwatch {
  return { name, step, background, hex: PALETTE_HEX[step] }
}

/** Body color swatches of the color popover (ui.md 3.10), in order; `hex` null is Default (removes `Body.color`). */
export const BODY_SWATCHES: readonly BodySwatch[] = [
  { name: 'Default', step: 'zinc-400', background: 'bg-zinc-400', hex: null },
  swatch('White', 'zinc-100', 'bg-zinc-100'),
  swatch('Black', 'zinc-800', 'bg-zinc-800'),
  swatch('Gray', 'zinc-500', 'bg-zinc-500'),
  swatch('Red', 'red-600', 'bg-red-600'),
  swatch('Orange', 'orange-500', 'bg-orange-500'),
  swatch('Yellow', 'yellow-400', 'bg-yellow-400'),
  swatch('Green', 'green-600', 'bg-green-600'),
  swatch('Teal', 'teal-600', 'bg-teal-600'),
  swatch('Blue', 'blue-600', 'bg-blue-600'),
  swatch('Purple', 'purple-600', 'bg-purple-600'),
  swatch('Pink', 'pink-600', 'bg-pink-600'),
  swatch('Brown', 'amber-800', 'bg-amber-800'),
]

/** Font family classes (ui.md 2.16.7): Inter for interface text, JetBrains Mono for values. */
export const FONT_CLASSES = {
  interfaceText: 'font-[Inter,ui-sans-serif,system-ui,sans-serif]',
  values: "font-['JetBrains_Mono',ui-monospace,monospace]",
} as const

/** Root font-size class of each UI scale (ui.md 2.16.9), a percentage of the browser default. */
export const UI_SCALE_CLASSES: Readonly<Record<Settings['uiScale'], string>> = {
  1: 'text-[100%]',
  1.25: 'text-[125%]',
  1.5: 'text-[150%]',
  2: 'text-[200%]',
  3: 'text-[300%]',
}

/** Classes the root element always carries: native controls and scrollbars follow the theme. */
export const ROOT_CLASSES = 'scheme-light dark:scheme-dark'

const HEX_COLOR = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i

/** '#rrggbb' to sRGB-encoded channels divided by 255. */
export function encodedFromHex(hex: string): EncodedRgb {
  const match = HEX_COLOR.exec(hex)
  if (match === null) {
    throw new Error(`Invalid color "${hex}". Use the form #rrggbb.`)
  }
  const [, red = '', green = '', blue = ''] = match
  return [
    Number.parseInt(red, 16) / 255,
    Number.parseInt(green, 16) / 255,
    Number.parseInt(blue, 16) / 255,
  ]
}

/** sRGB decoding of one encoded channel in 0 to 1. */
function linearChannel(encoded: number): number {
  return encoded <= 0.04045 ? encoded / 12.92 : ((encoded + 0.055) / 1.055) ** 2.4
}

/** '#rrggbb' to linear RGB (sRGB decoding), for shading. */
export function linearFromHex(hex: string): Rgb {
  const [red, green, blue] = encodedFromHex(hex)
  return [linearChannel(red), linearChannel(green), linearChannel(blue)]
}

/** WCAG 2.2 relative luminance of a '#rrggbb' color. */
function relativeLuminance(hex: string): number {
  const [red, green, blue] = linearFromHex(hex)
  return 0.2126 * red + 0.7152 * green + 0.0722 * blue
}

/** WCAG 2.2 contrast ratio of two '#rrggbb' colors (relative luminance with the 0.04045 sRGB threshold). */
export function contrastRatio(first: string, second: string): number {
  const firstLuminance = relativeLuminance(first)
  const secondLuminance = relativeLuminance(second)
  const lighter = Math.max(firstLuminance, secondLuminance)
  const darker = Math.min(firstLuminance, secondLuminance)
  return (lighter + 0.05) / (darker + 0.05)
}

function viewportThemeOf(theme: ThemeName): ViewportTheme {
  const colors = VIEWPORT_COLORS[theme]
  return Object.freeze({
    backgroundTop: linearFromHex(colors.viewportTop),
    backgroundBottom: linearFromHex(colors.viewportBottom),
    highlight: linearFromHex(colors.highlight),
    problem: linearFromHex(colors.problem),
    edgeDark: linearFromHex(colors.edgeDark),
    edgeLight: linearFromHex(colors.edgeLight),
  })
}

const VIEWPORT_THEMES: Readonly<Record<ThemeName, ViewportTheme>> = {
  light: viewportThemeOf('light'),
  dark: viewportThemeOf('dark'),
}

/** The backend's theme colors for one theme, linearized from VIEWPORT_COLORS. */
export function viewportTheme(theme: ThemeName): ViewportTheme {
  return VIEWPORT_THEMES[theme]
}
