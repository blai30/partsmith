import { act, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { editFeature } from '../app/actions/command'
import type { App } from '../app/createApp'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Diagnostic, FeatureStatus } from '../generated/protocol'
import { cubeDocumentState, documentWithStatuses, readyState } from '../layout/fixtures.testing'
import {
  createModelApp,
  renderWithApp,
  stubAnimationFrames,
  stubElementSize,
  stubMatchMedia,
  type FrameQueue,
} from '../layout/test-app.testing'
import { INITIAL_STATE, type DocumentState } from '../state/store'
import type { FakeBackend } from '../test/fakes/backend'
import type { FakeCoreApi } from '../test/fakes/core-api'
import {
  R16_CANDIDATE_FRONT,
  R16_CANDIDATE_TOP,
  R16_DOCUMENT,
  R16_EDGES,
  R16_MESH_ID,
  R16_MESSAGE,
  R16_REPAIR,
  R16_STATUS,
  R16_STORE_CHANGES,
  r16Fix,
  r16ListElements,
  SPLIT_DISPLAY,
  SPLIT_SOLUTION,
  SPLIT_STATUS,
} from '../test/fakes/repairs'
import { settle } from '../test/fakes/settle'
import { COLOR_CLASSES } from '../theme/roles'
import type { ElementRepair } from './repair'

// Repair mode (ui.md 3.7, U-127 and U-149): while a feature with repairs is edited, the lost
// element's ghost or ring with its title, numbered candidate markers at their elements, and the
// candidates as buttons in the inspector banner, described by meaning and distance, never by score.

// The WebGL2 backend cannot run in jsdom; the viewport gets a FakeBackend instead.
const created = vi.hoisted(() => ({ backends: [] as FakeBackend[] }))

vi.mock('../viewport/webgl2/backend', async () => {
  const fakes = await import('../test/fakes/backend')
  return {
    createWebGl2Backend: () => {
      const backend = fakes.createFakeBackend()
      created.backends.push(backend)
      return backend
    },
  }
})

const WIDTH = 800
const HEIGHT = 600

let restoreSize: () => void
let frames: FrameQueue

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  restoreSize = stubElementSize(HTMLCanvasElement.prototype, { width: WIDTH, height: HEIGHT })
  stubMatchMedia({ reducedMotion: true })
  frames = stubAnimationFrames()
  created.backends.length = 0
})

afterEach(() => {
  restoreSize()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

/** The R16 document as the store holds it after its edit, with 'Fillet 1' in `status`. */
function r16Document(status: FeatureStatus = R16_STATUS): DocumentState {
  return documentWithStatuses(R16_DOCUMENT, [status], { name: 'rabbet', ...R16_STORE_CHANGES })
}

/** R16's status with its one repair changed. */
function withRepair(repair: ElementRepair): FeatureStatus {
  return { ...R16_STATUS, repairs: [repair] }
}

type Harness = { readonly app: App; readonly core: FakeCoreApi }

/** The whole shell over the R16 document, with `f-fil1` edited. */
async function editingFillet(status: FeatureStatus = R16_STATUS): Promise<Harness> {
  const { app, core } = createModelApp({ state: readyState(r16Document(status)) })
  core.stub('listElements', r16ListElements)
  renderWithApp(app)
  flush()
  await act(async () => {
    await editFeature(app, 'f-fil1')
    await settle()
  })
  flush()
  return { app, core }
}

function flush(): void {
  act(() => {
    frames.flush()
  })
}

function lastBackend(): FakeBackend {
  const last = created.backends.at(-1)
  if (last === undefined) {
    throw new Error('The viewport made no backend.')
  }
  return last
}

/** A column-major 4 by 4 matrix times a 4-vector. */
function apply(matrix: Float32Array, vector: readonly number[]): number[] {
  return [0, 1, 2, 3].map((row) =>
    [0, 1, 2, 3].reduce(
      (sum, column) => sum + (matrix[column * 4 + row] ?? 0) * (vector[column] ?? 0),
      0,
    ),
  )
}

/** Where the last frame draws a world point on the canvas, in CSS pixels. */
function canvasPosition(point: readonly [number, number, number]): { x: number; y: number } {
  const frame = lastBackend().lastFrame()
  if (frame === null) {
    throw new Error('No frame was rendered.')
  }
  const clip = apply(frame.projection, apply(frame.view, [...point, 1]))
  const homogeneous = clip[3] ?? 1
  return {
    x: (((clip[0] ?? 0) / homogeneous + 1) / 2) * WIDTH,
    y: ((1 - (clip[1] ?? 0) / homogeneous) / 2) * HEIGHT,
  }
}

/** The runtime position of a positioned overlay element (3.12): the `translate` of its style attribute. */
function translateOf(element: HTMLElement): { x: number; y: number } {
  const style = element.getAttribute('style') ?? ''
  const match = /^translate: (-?[\d.e-]+)px (-?[\d.e-]+)px;?$/.exec(style)
  if (match === null) {
    throw new Error(`The element is not positioned: "${style}".`)
  }
  return { x: Number(match[1]), y: Number(match[2]) }
}

function banner(): HTMLElement {
  return within(screen.getByTestId('inspector')).getByTestId('inspector-error')
}

function candidateButtons(): HTMLElement[] {
  return within(banner()).getAllByTestId('repair-candidate-button')
}

describe('repair mode (3.7, U-127)', () => {
  it('U-127 makes Edges the active field', async () => {
    const { app } = await editingFillet()
    expect(app.store.getState().command?.activeField).toBe('edges')
  })

  it('U-127 draws the lost edge\'s ghost as one dashed path titled "{Description} (missing)", and no ring', async () => {
    await editingFillet()
    const ghosts = screen.getAllByTestId('repair-ghost')
    expect(ghosts).toHaveLength(1)
    const [ghost] = ghosts
    expect(ghost?.tagName.toLowerCase()).toBe('path')
    expect(ghost).toHaveAttribute('stroke-dasharray', '4 3')
    expect(ghost?.querySelector('title')).toHaveTextContent(
      "The front top edge of 'Box 1' (missing)",
    )
    expect(screen.queryByTestId('repair-marker')).toBeNull()
  })

  it('U-127 numbers a marker for each candidate at the centroid of its element', async () => {
    await editingFillet()
    const markers = screen.getAllByTestId('repair-candidate')
    expect(markers.map((marker) => marker.getAttribute('data-number'))).toEqual(['1', '2'])
    const centroids = [R16_EDGES[3]?.centroid, R16_EDGES[4]?.centroid]
    markers.forEach((marker, index) => {
      const centroid = centroids[index]
      if (centroid === undefined) {
        throw new Error('The edge descriptor has no centroid.')
      }
      const position = translateOf(marker)
      const expected = canvasPosition(centroid)
      expect(position.x).toBeCloseTo(expected.x, 0)
      expect(position.y).toBeCloseTo(expected.y, 0)
    })
  })

  it('U-127 lists the candidates as numbered buttons with their relation, then the other actions and Copy details', async () => {
    await editingFillet()
    const names = within(banner())
      .getAllByRole('button')
      .map((button) => button.textContent ?? '')
      .filter((name) => name !== R16_MESSAGE.message)
    expect(names).toHaveLength(4)
    expect(names[0]).toMatch(/^1\. The front edge of the cut made by 'Extrude 1'/)
    expect(names[1]).toMatch(/^2\. The top edge of the cut made by 'Extrude 1'/)
    expect(names.slice(2)).toEqual(['Pick an edge again', 'Copy details'])
  })

  it("U-127 puts each candidate's relation on a second line in textMuted", async () => {
    await editingFillet()
    const relations = [
      "on the front side of 'Box 1', 5 mm from where it was",
      "on the top of 'Box 1', 5 mm from where it was",
    ]
    candidateButtons().forEach((button, index) => {
      const relation = within(button).getByText(relations[index] ?? '')
      expect(relation).toHaveClass(COLOR_CLASSES.textMuted.foreground)
    })
  })

  it('U-127 offers no "Use" action besides the candidates', async () => {
    await editingFillet()
    expect(
      within(screen.getByTestId('inspector')).queryAllByRole('button', { name: /^Use / }),
    ).toEqual([])
  })

  it('U-127 shows no score or percentage anywhere', async () => {
    await editingFillet()
    for (const element of [screen.getByTestId('inspector'), screen.getByTestId('viewport')]) {
      const text = element.textContent ?? ''
      for (const forbidden of ['%', '0.91', '0.88']) {
        expect(text).not.toContain(forbidden)
      }
    }
  })

  it('U-127 hovering button 2 draws edge 4 hovered', async () => {
    await editingFillet()
    const user = userEvent.setup()
    const [, second] = candidateButtons()
    await user.hover(second ?? banner())
    flush()
    const viewport = screen.getByTestId('viewport')
    expect(viewport).toHaveAttribute('data-hover-kind', 'edge')
    expect(viewport).toHaveAttribute('data-hover-key', `mesh:${String(R16_MESH_ID)}:edge:4`)
  })

  it('U-127 lists a candidate no action stores, aria-disabled, so the numbers stay', async () => {
    await editingFillet(
      withRepair({
        ...R16_REPAIR,
        candidates: [
          R16_CANDIDATE_FRONT,
          { ...R16_CANDIDATE_TOP, description: "the top edge of 'Box 1'" },
        ],
      }),
    )
    const [first, second] = candidateButtons()
    expect(first).not.toHaveAttribute('aria-disabled', 'true')
    expect(second).toHaveAttribute('aria-disabled', 'true')
    expect(second?.textContent).toMatch(/^2\. The top edge of 'Box 1'/)
  })

  it('U-127 Esc cancels the command and removes the ghost and the markers', async () => {
    const { app } = await editingFillet()
    const user = userEvent.setup()
    await user.keyboard('{Escape}')
    await act(async () => {
      await settle()
    })
    flush()
    expect(app.store.getState().command).toBeNull()
    expect(screen.queryByTestId('repair-ghost')).toBeNull()
    expect(screen.queryByTestId('repair-candidate')).toBeNull()
  })

  it('U-127 without a ghost, rings the stored centroid with the same title', async () => {
    await editingFillet(withRepair({ ...R16_REPAIR, ghost: null }))
    expect(screen.queryByTestId('repair-ghost')).toBeNull()
    const markers = screen.getAllByTestId('repair-marker')
    expect(markers).toHaveLength(1)
    const [marker] = markers
    expect(marker?.tagName.toLowerCase()).toBe('circle')
    expect(marker).toHaveAttribute('stroke-dasharray', '4 3')
    expect(marker?.querySelector('title')).toHaveTextContent(
      "The front top edge of 'Box 1' (missing)",
    )
    const expected = canvasPosition([0, -10, 20])
    expect(Number(marker?.getAttribute('cx'))).toBeCloseTo(expected.x, 0)
    expect(Number(marker?.getAttribute('cy'))).toBeCloseTo(expected.y, 0)
  })
})

/** The title of the one repair ring. */
function markerTitle(): string {
  return screen.getByTestId('repair-marker').querySelector('title')?.textContent ?? ''
}

describe('repair titles by resolution (3.7, U-149)', () => {
  const WIDENED: FeatureStatus = {
    id: 'f-fil1',
    status: 'warning',
    messages: [
      {
        severity: 'warning',
        code: 'reference_widened',
        message:
          "The back top edge of 'Extrude 1' selected for Edges now covers 2 edges, so all of them are rounded.",
        field: '/edges/0',
      },
    ],
    repairs: [
      {
        kind: 'element',
        path: '/edges/0',
        body: 'b-1',
        reference: { q: 'edge(f-ext1/end-cap|f-ext1/lateral/e-3)', fp: 'fp-back' },
        resolution: 'ambiguous',
        description: "the back top edge of 'Extrude 1'",
        centroid: [0, 10, 20],
        candidates: [],
      },
    ],
  }

  it('U-149 an ambiguous repair reads "{Description} (check it)"', async () => {
    await editingFillet(WIDENED)
    expect(markerTitle()).toBe("The back top edge of 'Extrude 1' (check it)")
  })

  it('U-149 the same repair missing reads "{Description} (missing)"', async () => {
    const missing = WIDENED.repairs[0]
    if (missing === undefined || missing.kind !== 'element') {
      throw new Error('The widened status has no element repair.')
    }
    await editingFillet({ ...WIDENED, repairs: [{ ...missing, resolution: 'missing' }] })
    expect(markerTitle()).toBe("The back top edge of 'Extrude 1' (missing)")
  })

  it('U-149 no title says "(missing)" while the resolution is ambiguous', async () => {
    await editingFillet(WIDENED)
    const titles = [...document.querySelectorAll('title')].map((title) => title.textContent ?? '')
    expect(titles.filter((title) => title.includes('(missing)'))).toEqual([])
  })
})

/** The whole shell over the R16 document with the Problems panel open and no command. */
function problemsShell(): Harness {
  const { app, core } = createModelApp({
    state: readyState(r16Document(), {
      panels: { ...INITIAL_STATE.panels, browserOpen: true, side: 'problems' },
    }),
  })
  core.stub('listElements', r16ListElements)
  renderWithApp(app)
  flush()
  return { app, core }
}

describe('repair mode from a Problems row (3.7, 3.9)', () => {
  it('clicking a candidate marker applies its candidate at rest, keeping the row focused', async () => {
    const { app, core } = problemsShell()
    const user = userEvent.setup()
    const row = screen
      .getAllByTestId('problem-row')
      .find((element) => element.getAttribute('data-feature-id') === 'f-fil1')
    await user.click(within(row ?? document.body).getByRole('button', { name: 'Fillet 1' }))
    await act(async () => {
      await settle()
    })
    flush()
    const [first] = screen.getAllByTestId('repair-candidate')
    await user.click(first ?? document.body)
    await act(async () => {
      await settle()
    })
    expect(core.callsTo('applyFix')).toEqual([[1, r16Fix(0)]])
    expect(core.callsTo('beginTransaction')).toEqual([])
    expect(app.store.getState().repair).toEqual({ feature: 'f-fil1' })
  })
})

describe('candidates under the message that carries them (3.7)', () => {
  const TOO_LARGE: Diagnostic = {
    severity: 'error',
    code: 'fillet_too_large',
    message:
      'Fillet radius 15 mm is too large for the selected edges. The largest radius that works is about 2.3 mm. Try 2.3 mm or less.',
    field: '/radius',
  }
  const LOST_WARNING: Diagnostic = {
    ...R16_MESSAGE,
    severity: 'warning',
    code: 'reference_missing',
  }

  it('lists no candidate under an error when the warning that carries the repair is not shown', async () => {
    await editingFillet({
      ...R16_STATUS,
      messages: [TOO_LARGE, LOST_WARNING],
    })
    expect(banner()).toHaveAttribute('data-severity', 'error')
    expect(within(banner()).queryAllByTestId('repair-candidate-button')).toEqual([])
    expect(screen.getAllByTestId('repair-candidate')).toHaveLength(2)
  })
})

describe('region repairs on the model (3.7)', () => {
  it("numbers each candidate at the centroid of its first region on the sketch's placement", () => {
    const { app } = createModelApp({
      state: readyState(
        cubeDocumentState({
          sketches: { 'f-sk1': { solution: SPLIT_SOLUTION, display: SPLIT_DISPLAY } },
          featureStatus: { 'f-ext1': SPLIT_STATUS },
        }),
        { repair: { feature: 'f-ext1' } },
      ),
    })
    renderWithApp(app)
    flush()
    const markers = screen.getAllByTestId('repair-candidate')
    expect(markers.map((marker) => marker.getAttribute('data-number'))).toEqual(['1', '2'])
    expect(markers.map((marker) => marker.getAttribute('data-anchor'))).toEqual(['5,0,0', '-5,0,0'])
  })
})
