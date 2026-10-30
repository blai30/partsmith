/// <reference lib="dom" />
import { expect, type Page } from '@playwright/test'
import {
  bodyRow,
  download,
  dropFile,
  fitView,
  inspector,
  inspectorInput,
  openApp,
  originRow,
  renameBody,
  runCommand,
  test,
  timelineItem,
  viewCubeButton,
} from './helpers'
import {
  chooseOption,
  clickCanvas,
  confirmAndExpectOk,
  expectPreviewShown,
  fillInspector,
  focusWhereEnterConfirms,
  pickOnCanvas,
  runFromGroup,
  selectionField,
} from './support/modeling'
import {
  BLESS_IN_CI,
  asObject,
  asObjects,
  blessDocument,
  blessMode,
  blessWrites,
  committedDocument,
  differenceMessage,
  firstDifference,
  missingMessage,
  readCommittedDocument,
  type Json,
  type JsonObject,
} from './support/round-trip'

// The GUI to Python round trip (ui.md 6.5 E-160, delivery.md 2.8, ADR 0048): the documents the UI
// writes, made in the real UI, saved through the download and compared with the committed copies in
// fixtures/gui/, which python/tests/test_gui_documents.py puts through the Python round trip.

/** One row of E-160: the document it writes, the steps that make it and the facts it holds. */
type Row = {
  readonly file: string
  readonly steps: (page: Page) => Promise<void>
  readonly facts: (document: Json) => void
}

/** A matcher of an id the UI generates with `prefix` (`f` feature, `b` body, `p` parameter, `e` entity). */
function generatedId(prefix: 'f' | 'b' | 'p' | 'e'): unknown {
  return expect.stringMatching(new RegExp(`^${prefix}-[0-9a-f]{12}$`))
}

/** A matcher of an edge reference whose `q` starts with `edge(`. */
const EDGE_REFERENCE = expect.objectContaining({ q: expect.stringMatching(/^edge\(/) })

/** Opens the inspector's More options unless they are open. */
async function openMore(page: Page): Promise<void> {
  const more = inspector(page).getByTestId('inspector-more')
  if ((await more.getAttribute('aria-expanded')) === 'false') {
    await more.click()
  }
  await expect(more).toHaveAttribute('aria-expanded', 'true')
}

/** Turns on the inspector switch labeled `label`. */
async function turnOn(page: Page, label: string): Promise<void> {
  const control = inspector(page).getByRole('switch', { name: label, exact: true })
  await control.click()
  await expect(control).toHaveAttribute('aria-checked', 'true')
}

/** Activates an inspector selection field unless it is active, for the next pick. */
async function activateSelection(page: Page, path: string): Promise<void> {
  const field = selectionField(page, path)
  if ((await field.getAttribute('data-active')) !== 'true') {
    await field.click()
  }
  await expect(field).toHaveAttribute('data-active', 'true')
}

/** Confirms the active command where Enter confirms it (ui.md 3.5) and expects `label` to be ok. */
async function confirm(page: Page, label: string): Promise<void> {
  await expectPreviewShown(page)
  await focusWhereEnterConfirms(page)
  await confirmAndExpectOk(page, label)
}

/** Renames a feature in the timeline: click its item, F2, type the new name, Enter (ui.md 3.9). */
async function renameFeature(page: Page, from: string, to: string): Promise<void> {
  await timelineItem(page, from).getByRole('button', { name: from, exact: true }).click()
  await page.keyboard.press('F2')
  const field = page.getByTestId('timeline').getByRole('textbox', { name: 'Name', exact: true })
  await expect(field).toBeFocused()
  await page.keyboard.type(to)
  await page.keyboard.press('Enter')
  await expect(timelineItem(page, to)).toHaveCount(1)
}

/** Closes the side panel (Parameters) with its close button. */
async function closeSidePanel(page: Page): Promise<void> {
  await page
    .getByTestId('side-panel')
    .getByRole('button', { name: /^Close/ })
    .click()
  await expect(page.getByTestId('parameters-panel')).toHaveCount(0)
}

/** Whether `vector` is within 1e-9 of `[1, 0, 0]` or of `[-1, 0, 0]`. */
function alongX(vector: Json | undefined): boolean {
  if (!Array.isArray(vector) || vector.length !== 3) {
    return false
  }
  const [x, y, z] = vector
  return (
    typeof x === 'number' &&
    typeof y === 'number' &&
    typeof z === 'number' &&
    Math.abs(Math.abs(x) - 1) <= 1e-9 &&
    Math.abs(y) <= 1e-9 &&
    Math.abs(z) <= 1e-9
  )
}

const ROWS: readonly Row[] = [
  {
    file: 'extrude-sides.ptsm',
    steps: async (page) => {
      await dropFile(page, 'parametric.ptsm')
      await timelineItem(page, 'Sketch 1')
        .getByRole('button', { name: 'Sketch 1', exact: true })
        .dblclick()
      await expect(page.getByTestId('app')).toHaveAttribute('data-context', 'sketch')
      await page
        .getByRole('toolbar', { name: 'Sketch tools' })
        .getByRole('button', { name: 'Finish sketch' })
        .click()
      await expect(page.getByTestId('selection-title')).toHaveText('1 region')
      await inspector(page).getByRole('button', { name: 'Extrude' }).click()
      await expect(inspector(page)).toHaveAccessibleName('Extrude')
      await chooseOption(page, 'Extent', 'Two sides')
      await fillInspector(page, 'Distance', 'size / 2')
      await fillInspector(page, 'Opposite distance', '5')
      const reverse = inspector(page).getByRole('button', { name: 'Reverse', exact: true })
      await reverse.click()
      await expect(reverse).toHaveAttribute('aria-pressed', 'true')
      await confirm(page, 'Extrude 2')
      await renameFeature(page, 'Extrude 2', 'Foot')
      await renameBody(page, 'Cube', 'Block')
      await runCommand(page, 'Parameters')
      const name = page.getByRole('textbox', { name: 'Name of size', exact: true })
      await name.fill('side')
      await name.press('Enter')
      await expect(page.getByRole('textbox', { name: 'Name of side', exact: true })).toHaveValue(
        'side',
      )
      await closeSidePanel(page)
    },
    facts: (document) => {
      const saved = asObject(document, 'the document')
      expect(saved['parameters']).toEqual([{ id: 'p-size', name: 'side', expr: '20 mm' }])
      expect(saved['bodies']).toEqual([{ id: 'b-cube', name: 'Block' }])
      const features = asObjects(saved['features'], '/features')
      expect(features).toHaveLength(3)
      expect(features[0]).toMatchObject({ id: 'f-sk1' })
      expect(features[1]).toMatchObject({ id: 'f-ext1' })
      expect(features[1]?.['extent']).toEqual({ kind: 'distance', expr: 'side' })
      expect(features[2]).toMatchObject({
        id: generatedId('f'),
        type: 'extrude',
        name: 'Foot',
        body: 'b-cube',
        profile: { sketch: 'f-sk1', regions: [{ outline: expect.any(String) }] },
        reverse: true,
        op: 'join',
      })
      expect(features[2]?.['extent']).toEqual({
        kind: 'two_sides',
        expr: 'side / 2',
        opposite_expr: '5',
      })
    },
  },
  {
    file: 'picks.ptsm',
    steps: async (page) => {
      await dropFile(page, 'cube.ptsm')
      // The hole comes first: after the pattern, the top fit centers between the two bodies.
      await fitView(page, 'Top')
      await clickCanvas(page, 0, 0)
      await expect(page.getByTestId('selection-title')).toHaveText('1 face')
      await inspector(page).getByRole('button', { name: 'Hole' }).click()
      await expect(inspector(page)).toHaveAccessibleName('Hole')
      await pickOnCanvas(page, 0, 0, 'points', 1, 'shown')
      await confirm(page, 'Hole 1')
      await expect(timelineItem(page, 'Sketch 2')).toHaveAttribute('data-status', 'ok')
      // From the top view the view cube's front face is edge-on, so it cannot be clicked.
      await viewCubeButton(page, 'Home view').click()
      await fitView(page, 'Front')
      await runCommand(page, 'Chamfer')
      await expect(inspector(page)).toHaveAccessibleName('Chamfer')
      await activateSelection(page, 'edges')
      await pickOnCanvas(page, 0, -0.4, 'edges', 1, 'shown')
      await chooseOption(page, 'Type', 'Two distances')
      await fillInspector(page, 'Distance', '2')
      await expect(inspectorInput(page, 'Second distance')).toHaveValue('1 mm')
      await openMore(page)
      await turnOn(page, 'Flip')
      await confirm(page, 'Chamfer 1')
      await runFromGroup(page, 'Pattern', 'Linear pattern')
      await expect(inspector(page)).toHaveAccessibleName('Linear pattern')
      await expect(selectionField(page, 'body')).toHaveText('Cube')
      await activateSelection(page, 'direction.axis')
      await pickOnCanvas(page, 0, 0.4, 'direction.axis', 1, 'shown')
      await fillInspector(page, 'Spacing', '30')
      await expect(inspectorInput(page, 'Count')).toHaveValue('2')
      await openMore(page)
      await turnOn(page, 'Reverse')
      await chooseOption(page, 'Operation', 'New body')
      await expect(selectionField(page, 'target')).toHaveText('Body 1 (new)')
      await confirm(page, 'Linear pattern 1')
      await expect(bodyRow(page, 'Cube')).toHaveCount(1)
      await expect(bodyRow(page, 'Body 1')).toHaveCount(1)
    },
    facts: (document) => {
      const saved = asObject(document, 'the document')
      expect(saved['bodies']).toEqual([
        { id: 'b-cube', name: 'Cube' },
        { id: generatedId('b'), name: 'Body 1' },
      ])
      const newBody = asObjects(saved['bodies'], '/bodies')[1]?.['id']
      const features = asObjects(saved['features'], '/features')
      expect(features).toHaveLength(6)
      const [sketch, extrude, faceSketch, hole, chamfer, pattern] = features
      expect(sketch).toMatchObject({ id: 'f-sk1' })
      expect(extrude).toMatchObject({ id: 'f-ext1' })
      expect(faceSketch).toMatchObject({
        id: generatedId('f'),
        type: 'sketch',
        plane: {
          kind: 'face',
          body: 'b-cube',
          face: { q: 'f-ext1/end-cap' },
          fallback: expect.any(Object),
          fallback_frame: 'body',
        },
      })
      const entities = asObjects(faceSketch?.['entities'], '/features/2/entities')
      expect(entities).toEqual([expect.objectContaining({ id: generatedId('e'), type: 'point' })])
      expect(hole).toMatchObject({
        id: generatedId('f'),
        type: 'hole',
        body: 'b-cube',
        sketch: faceSketch?.['id'],
        points: [entities[0]?.['id']],
        diameter: '3.2 mm',
      })
      expect(hole?.['extent']).toEqual({ kind: 'through_all' })
      expect(chamfer).toMatchObject({
        id: generatedId('f'),
        type: 'chamfer',
        body: 'b-cube',
        edges: [EDGE_REFERENCE],
      })
      expect(chamfer?.['distance']).toEqual({
        kind: 'two_distances',
        expr: '2',
        second_expr: '1 mm',
        flip: true,
      })
      expect(pattern).toMatchObject({
        id: generatedId('f'),
        type: 'linear_pattern',
        body: 'b-cube',
        op: 'new',
        target: newBody,
        direction: {
          count: '2',
          spacing: '30',
          reverse: true,
          axis: { kind: 'edge', body: 'b-cube', edge: EDGE_REFERENCE },
        },
      })
      expect(pattern).not.toHaveProperty('second_direction')
      const axis = asObject(
        asObject(pattern?.['direction'], '/features/5/direction')['axis'],
        'axis',
      )
      expect(alongX(axis['direction'])).toBe(true)
    },
  },
  {
    file: 'starting-values.ptsm',
    steps: async (page) => {
      const empty = page.getByTestId('empty-state')
      await empty.getByRole('button', { name: 'Box', exact: true }).click()
      await expect(inspector(page)).toHaveAccessibleName('Box')
      await confirm(page, 'Box 1')
      await fitView(page, 'Front')
      await clickCanvas(page, 0, -0.4)
      // A click selects once the core gives the edge's reference, which F then preselects (as E-32).
      await expect(page.getByTestId('selection-title')).toHaveText('1 edge')
      await page.keyboard.press('f')
      await expect(inspectorInput(page, 'Radius')).toHaveValue('1 mm')
      await confirm(page, 'Fillet 1')
      await runCommand(page, 'Split body')
      await expect(selectionField(page, 'body')).toHaveText('Body 1')
      await activateSelection(page, 'plane')
      await (await originRow(page, 'Front plane (XZ)')).click()
      await expect(selectionField(page, 'plane')).toHaveText('Front plane (XZ)')
      await confirm(page, 'Split body 1')
      await expect(bodyRow(page, 'Body 1')).toHaveCount(1)
      await expect(bodyRow(page, 'Body 2')).toHaveCount(1)
      await runCommand(page, 'Parameters')
      await page
        .getByTestId('parameters-panel')
        .getByRole('button', { name: 'Add parameter', exact: true })
        .click()
      await page.keyboard.type('width')
      await page.keyboard.press('Enter')
      await page.keyboard.type('40 mm')
      await page.keyboard.press('Enter')
      await expect(page.getByTestId('parameter-row')).toHaveCount(1)
      await closeSidePanel(page)
    },
    facts: (document) => {
      const saved = asObject(document, 'the document')
      expect(saved['parameters']).toEqual([{ id: generatedId('p'), name: 'width', expr: '40 mm' }])
      expect(saved['bodies']).toEqual([
        { id: generatedId('b'), name: 'Body 1' },
        { id: generatedId('b'), name: 'Body 2' },
      ])
      const [first, second] = asObjects(saved['bodies'], '/bodies').map((body) => body['id'])
      const features = asObjects(saved['features'], '/features')
      expect(features).toHaveLength(3)
      const [box, fillet, split] = features
      expect(box).toMatchObject({ id: generatedId('f'), type: 'box', body: first, op: 'new' })
      expect(box?.['size']).toEqual(['10 mm', '10 mm', '10 mm'])
      expect(fillet).toMatchObject({
        id: generatedId('f'),
        type: 'fillet',
        body: first,
        edges: [EDGE_REFERENCE],
        radius: '1 mm',
      })
      expect(split).toMatchObject({
        id: generatedId('f'),
        type: 'split_body',
        body: first,
        target: second,
      })
      expect(split?.['plane']).toEqual({ kind: 'origin', plane: 'XZ' })
    },
  },
]

/** The time one row of E-160 may take: its steps make up to six features in the real core. */
const ROW_TIMEOUT_MILLISECONDS = 60_000

test.describe('E-160 documents the UI writes', () => {
  for (const row of ROWS) {
    test(`E-160 ${row.file}`, async ({ page }) => {
      test.setTimeout(ROW_TIMEOUT_MILLISECONDS)
      const bless = blessMode(process.env)
      await openApp(page, { fileSystemAccess: false })
      await row.steps(page)
      await expect(page.locator('[data-testid=timeline-item]:not([data-status="ok"])')).toHaveCount(
        0,
      )
      const saved = await download(page, () => page.keyboard.press('ControlOrMeta+S'))
      const downloaded = JSON.parse(saved.bytes.toString('utf8')) as Json
      row.facts(downloaded)
      if (bless && blessWrites(downloaded, await readCommittedDocument(row.file))) {
        await blessDocument(row.file, saved.bytes)
      }
      const committed = await committedDocument(row.file)
      row.facts(committed)
      const difference = firstDifference(downloaded, committed)
      if (difference !== null) {
        throw new Error(differenceMessage(row.file, difference))
      }
    })
  }
})

/** A literal saved document for the comparison's own tests: one body and one fillet. */
const literalDocument: JsonObject = {
  format_version: 2,
  bodies: [{ id: 'b-0123456789ab', name: 'Body 1' }],
  features: [
    {
      id: 'f-0123456789ab',
      type: 'fillet',
      body: 'b-0123456789ab',
      edges: [{ q: 'edge(f-0123456789ab/front|f-0123456789ab/top)', fp: 'line|l=10' }],
      radius: '1 mm',
      position: [0.5, 0, 10],
    },
  ],
}

/** `literalDocument` with `change` applied to a deep copy. */
function changed(change: (copy: JsonObject) => void): JsonObject {
  const copy = structuredClone(literalDocument)
  change(copy)
  return copy
}

/** The first feature of a copy made by `changed`. */
function feature(copy: JsonObject): JsonObject {
  return asObjects(copy['features'], '/features')[0] ?? {}
}

test.describe('E-160 comparison of a download with its committed document', () => {
  test('E-160 the same document matches', () => {
    expect(firstDifference(literalDocument, structuredClone(literalDocument))).toBeNull()
  })

  test('E-160 generated ids of one prefix match', () => {
    const renumbered = changed((copy) => {
      feature(copy)['id'] = 'f-ba9876543210'
      feature(copy)['body'] = 'b-ba9876543210'
    })

    expect(firstDifference(renumbered, literalDocument)).toBeNull()
  })

  test('E-160 a generated id does not match an id of another prefix', () => {
    const otherPrefix = changed((copy) => {
      feature(copy)['body'] = 'f-0123456789ab'
    })

    expect(firstDifference(otherPrefix, literalDocument)).toEqual({
      pointer: '/features/0/body',
      actual: '"f-0123456789ab"',
      expected: '"b-0123456789ab"',
    })
  })

  test('E-160 reference members are not compared', () => {
    const repicked = changed((copy) => {
      const edges = asObjects(feature(copy)['edges'], 'edges')
      edges[0] = { q: 'edge(f-ba9876543210/top|f-ba9876543210/front)', fp: 'line|l=10.000001' }
      feature(copy)['edges'] = edges
    })

    expect(firstDifference(repicked, literalDocument)).toBeNull()
  })

  test('E-160 numbers within 1e-9 match and farther ones do not', () => {
    const close = changed((copy) => {
      feature(copy)['position'] = [0.5 + 5e-10, 0, 10]
    })
    const far = changed((copy) => {
      feature(copy)['position'] = [0.5001, 0, 10]
    })

    expect(firstDifference(close, literalDocument)).toBeNull()
    expect(firstDifference(far, literalDocument)).toEqual({
      pointer: '/features/0/position/0',
      actual: '0.5001',
      expected: '0.5',
    })
  })

  test('E-160 other values must be equal', () => {
    const otherRadius = changed((copy) => {
      feature(copy)['radius'] = '2 mm'
    })

    expect(firstDifference(otherRadius, literalDocument)).toEqual({
      pointer: '/features/0/radius',
      actual: '"2 mm"',
      expected: '"1 mm"',
    })
  })

  test('E-160 a field the UI starts writing is a difference of keys', () => {
    const newField = changed((copy) => {
      feature(copy)['suppressed'] = false
    })

    expect(firstDifference(newField, literalDocument)).toEqual({
      pointer: '/features/0',
      actual: 'the keys ["id","type","body","edges","radius","position","suppressed"]',
      expected: 'the keys ["id","type","body","edges","radius","position"]',
    })
  })

  test('E-160 keys in another order are a difference', () => {
    const reordered = changed((copy) => {
      const { radius, ...rest } = feature(copy)
      copy['features'] = [{ radius: radius ?? null, ...rest }]
    })

    expect(firstDifference(reordered, literalDocument)?.pointer).toBe('/features/0')
  })

  test('E-160 arrays of other lengths are a difference of items', () => {
    const extraBody = changed((copy) => {
      copy['bodies'] = [
        ...asObjects(copy['bodies'], 'bodies'),
        { id: 'b-ba9876543210', name: 'Body 2' },
      ]
    })

    expect(firstDifference(extraBody, literalDocument)).toEqual({
      pointer: '/bodies',
      actual: '2 items',
      expected: '1 items',
    })
  })

  test('E-160 a difference at the root names the document', () => {
    const rollback = changed((copy) => {
      copy['rollback'] = 'f-0123456789ab'
    })
    const difference = firstDifference(rollback, literalDocument)

    expect(difference?.pointer).toBe('')
    expect(
      differenceMessage('picks.ptsm', difference ?? { pointer: '', actual: '', expected: '' }),
    ).toBe(
      'ui/e2e/fixtures/gui/picks.ptsm is not what the UI writes now: the document is the keys ' +
        '["format_version","bodies","features","rollback"] in the download and the keys ' +
        '["format_version","bodies","features"] in the committed file. ' +
        'Run: PARTSMITH_BLESS=1 pnpm -C ui e2e round-trip.spec.ts',
    )
  })

  test('E-160 a pointer escapes ~ and / in keys', () => {
    expect(firstDifference({ 'a/b~c': 1 }, { 'a/b~c': 2 })?.pointer).toBe('/a~1b~0c')
  })

  test('E-160 a missing document names the command that writes it', () => {
    expect(missingMessage('picks.ptsm')).toBe(
      'ui/e2e/fixtures/gui/picks.ptsm is missing. Run: PARTSMITH_BLESS=1 pnpm -C ui e2e round-trip.spec.ts',
    )
  })

  test('E-160 a bless writes a missing document', () => {
    expect(blessWrites(literalDocument, null)).toBe(true)
  })

  test('E-160 a bless keeps a committed document that still matches, ids and references aside', () => {
    const rerun = changed((copy) => {
      feature(copy)['id'] = 'f-ba9876543210'
      feature(copy)['edges'] = [{ q: 'edge(f-ba9876543210/top|f-ba9876543210/front)', fp: 'line' }]
    })

    expect(blessWrites(rerun, literalDocument)).toBe(false)
  })

  test('E-160 a bless writes over a committed document that differs', () => {
    const otherRadius = changed((copy) => {
      feature(copy)['radius'] = '2 mm'
    })

    expect(blessWrites(otherRadius, literalDocument)).toBe(true)
  })

  test('E-160 a bless in CI is refused, and a bless runs only with PARTSMITH_BLESS=1', () => {
    expect(() => blessMode({ PARTSMITH_BLESS: '1', CI: 'true' })).toThrow(BLESS_IN_CI)
    expect(blessMode({ PARTSMITH_BLESS: '1' })).toBe(true)
    expect(blessMode({ PARTSMITH_BLESS: '0' })).toBe(false)
    expect(blessMode({ CI: 'true' })).toBe(false)
    expect(blessMode({})).toBe(false)
  })
})
