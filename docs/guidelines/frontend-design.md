# Frontend Design

Guidelines for distinctive, production-grade frontend interfaces that avoid generic "AI slop" aesthetics.

## Design Thinking

Before coding, understand context and commit to a BOLD aesthetic direction:
- **Purpose**: What problem does this solve? Who uses it?
- **Tone**: Pick an extreme: brutally minimal, maximalist chaos, retro-futuristic, organic, luxury/refined, playful, editorial, brutalist, art deco, soft/pastel, industrial, etc. Use these for inspiration but design something true to the direction.
- **Constraints**: Technical requirements (framework, performance, accessibility).
- **Differentiation**: What makes this unforgettable?

Choose a clear conceptual direction and execute with precision. Bold maximalism and refined minimalism both work Intentionality is key.

## Technical Implementation

- Avoid template strings for conditional classes. Use `clsx` (prefer `clsx/lite` when full feature set isn't needed).
- Production-grade, functional, visually striking, cohesive, meticulously refined.

## Aesthetics

- **Typography**: Distinctive fonts. Avoid generic choices (Arial, Roboto, system fonts). Pair a distinctive display font with a refined body font. Prefer Inter or Geist for body, JetBrains Mono for code. Use variable fonts when available.
- **Color & Theme**: Cohesive aesthetic via Tailwind only, no new CSS variables. Dominant colors with sharp accents over timid palettes.
- **Motion**: Animations for effects and micro-interactions. Use Motion library for React. Focus on high-impact moments: one orchestrated page load with staggered reveals beats scattered micro-interactions. Use scroll-triggering and surprising hover states.
- **Spatial Composition**: Unexpected layouts. Asymmetry, overlap, diagonal flow, grid-breaking elements. Generous negative space OR controlled density.
- **Visual Details**: Atmosphere and depth over solid colors. Gradient meshes, noise textures, geometric patterns, layered transparencies, dramatic shadows, decorative borders, custom cursors, grain overlays. Match the aesthetic.
- **Icons**: lucide icons (eg. `lucide-react` for React).
- **Charts**: visx from Airbnb for customizable data visualizations.
- **Alignment**: Default to left-align. Center only when necessary.

Avoid these default looks unless the direction calls for them: purple gradients on white, a cream or off-white background, italic accent words in headlines, numbered "01/02/03" section labels, monospace labels, and pill-shaped buttons. When a result still looks generic, name the pattern it fell back on and add it to this list. No two designs should be the same; vary themes, fonts, and aesthetics.

Match implementation complexity to the vision. Maximalist = elaborate code with extensive effects. Minimalist = restraint, precision, spacing, typography, subtle details.
