import { readFile } from 'node:fs/promises'
import { expect, type Page } from '@playwright/test'
import type {
  Command,
  Document,
  Edit,
  Event,
  FeatureRecord,
  FeatureStatus,
  FixRef,
  Message,
  Outcome,
  Parameter,
} from '../../src/generated/protocol'
import type { Answer, PartsmithEngine } from '../pages/engine-worker'

// The engine's web-column budgets (engine.md section 5, test 155): the driver of the engine page and the rows of
// crates/partsmith-engine/benches/budgets.rs ported to the web core, under the same names and with the same documents,
// edits, samples and checks. A change to a row there changes its row here in the same change.

/**
 * The engine page's worker: each function runs the worker's function of the same name through `Worker.evaluate` and
 * resolves to what it returns, or rejects with the text it throws.
 */
export type Engine = {
  readonly [Name in keyof PartsmithEngine]: (
    ...args: Parameters<PartsmithEngine[Name]>
  ) => Promise<Awaited<ReturnType<PartsmithEngine[Name]>>>
}

/** Row 1: its name, and the bytes of wasm memory it measures. */
export type MemoryRow = {
  readonly name: string
  readonly measure: (engine: Engine) => Promise<number>
}

/** Rows 2 to 17: each one's name, web budget in milliseconds, and the samples it measures. */
export type TimedRow = {
  readonly name: string
  readonly budget: number
  readonly measure: (engine: Engine) => Promise<number[]>
}

/** The result of a command that succeeded. */
type OkResult = Extract<Outcome, { kind: 'ok' }>

/** The event of `type` as a message. */
type EventMessage<Type extends Event['type']> = Extract<Message, { kind: 'event'; type: Type }>

/** An instance of the web core, where a row of budgets.rs has a `Driver`. */
type Driver = {
  /** Sends one command and resolves to its answer, with the time `callCore` took. */
  readonly send: (command: Command) => Promise<Answer>
  /** Sends `open_document` of a text without assets. */
  readonly open: (text: string) => Promise<Answer>
  /** The byte length of the instance's wasm memory. */
  readonly memoryBytes: () => Promise<number>
}

/** How long the engine page may take to start its worker, which the Vite dev server compiles on first request. */
const READY_TIMEOUT_MILLISECONDS = 15_000

/** The samples each timed row takes, `RUNS` of budgets.rs. */
const SAMPLES = 7

/** The one 50-feature benchmark document (engine.md section 5). */
const REFERENCE_PART = new URL('../../../examples/bench/fifty-features/model.ptsm', import.meta.url)

/** The parameter of the reference part that drives its last five features. */
const LAST_FIVE_PARAMETER = 'lid_width'

/** The fillet of the reference part's lids, among its last five features. */
const LID_FILLET = 'f-fil8'

/** The pattern of the reference part's lids, its last feature. */
const LID_PATTERN = 'f-lp4'

/** The body of the reference part's lids. */
const LIDS = 'b-4'

/** The fillet the loss and ghost rows add on edges of the lid pattern's third copy. */
const THIRD_LID_FILLET = 'f-fil9'

/** The number of references the loss row loses. */
const LOST_REFERENCES = 5

/** Rectangles of the drag sketch; each has 4 lines and 8 constraints, 504 constraints in all. */
const DRAG_RECTANGLES = 63

/** Instances per direction of the grid body: 13 x 13 boxes of 6 faces are 1014 faces. */
const GRID_COUNT = 13

/** Features, each a box in its own body, of the memory document. */
const MEMORY_FEATURES = 300

/** Radius of the sphere whose `standard` display mesh with `generic-resin` has about 200 000 triangles. */
const SPHERE_RADIUS = '250 mm'

/** A length in millimeters written as a decimal number, which `without_millimeters` of budgets.rs writes bare. */
const MILLIMETERS = /^([+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?) mm$/

/**
 * Opens the engine page at `url` on `page` (engine.md section 5): goes to it, waits until its worker has put
 * `partsmithEngine` on its global scope, and resolves to the `Engine` that runs that worker's functions; rejects with
 * the worker's error when the page marks one instead.
 */
export async function openEngine(page: Page, url: string): Promise<Engine> {
  await page.goto(url)
  await expect(page.locator('body[data-ready="true"], body[data-error]')).toBeAttached({
    timeout: READY_TIMEOUT_MILLISECONDS,
  })
  const error = await page.locator('body').getAttribute('data-error')
  if (error !== null) {
    throw new Error(`The engine page's worker did not start: ${error}`)
  }
  const [worker] = page.workers()
  if (worker === undefined) {
    throw new Error('The engine page started no worker.')
  }
  const run = <Name extends keyof PartsmithEngine>(
    name: Name,
    ...args: Parameters<PartsmithEngine[Name]>
  ) =>
    worker.evaluate(
      ({ functionName, functionArgs }) => {
        // In the worker, whose functions are on its global scope.
        const engine: Record<string, (...values: unknown[]) => unknown> = Reflect.get(
          globalThis,
          'partsmithEngine',
        )
        return engine[functionName](...functionArgs)
      },
      { functionName: name as string, functionArgs: args as unknown[] },
    ) as Promise<Awaited<ReturnType<PartsmithEngine[Name]>>>
  return {
    start: () => run('start'),
    call: (instance, command) => run('call', instance, command),
    memoryBytes: (instance) => run('memoryBytes', instance),
    end: (instance) => run('end', instance),
  }
}

/**
 * The result of `answer`'s last message when it is a response that succeeded, the counterpart of `Answer::ok` of
 * budgets.rs; throws when the command failed or no response came.
 */
export function ok(answer: Answer): OkResult {
  const last = answer.messages.at(-1)
  if (last?.kind !== 'response') {
    throw new Error(`${requestOf(answer)} got no response from the web core.`)
  }
  if (last.result.kind === 'error') {
    throw new Error(
      `${requestOf(answer)} failed in the web core with ${last.result.code}: ${last.result.message}`,
    )
  }
  return last.result
}

/** The words that name the request of `answer` in an error. */
function requestOf(answer: Answer): string {
  return `${answer.type} (request ${String(answer.id)})`
}

/** The result of `answer`, which succeeded with a result of `type`. */
function resultOf<Type extends OkResult['type']>(
  answer: Answer,
  type: Type,
): Extract<OkResult, { type: Type }> {
  const result = ok(answer)
  if (result.type !== type) {
    throw new Error(`${requestOf(answer)} answered ${result.type}.`)
  }
  return result as Extract<OkResult, { type: Type }>
}

/** The time `callCore` took for `answer`, which succeeded. */
function timeOf(answer: Answer): number {
  ok(answer)
  return answer.milliseconds
}

/** The document an `open_document` answer opened, as `Answer::doc` reads it. */
function docOf(answer: Answer): number {
  return resultOf(answer, 'open_document').doc
}

/** The events of `type` that `answer`, which succeeded, sent. */
function eventsOf<Type extends Event['type']>(answer: Answer, type: Type): EventMessage<Type>[] {
  ok(answer)
  return answer.messages.filter(
    (message): message is EventMessage<Type> => message.kind === 'event' && message.type === type,
  )
}

/** The `mesh_id` of the model-layer `body_mesh` event of `body` in `answer`. */
function meshId(answer: Answer, body: string): number {
  const mesh = eventsOf(answer, 'body_mesh').find(
    (event) => event.body === body && event.layer === 'model',
  )
  if (mesh === undefined) {
    throw new Error(`no model body_mesh for ${body}`)
  }
  return mesh.mesh_id
}

/** The status of `feature` in the only `recomputed` event of `answer`. */
function featureStatus(answer: Answer, feature: string): FeatureStatus {
  const recomputed = eventsOf(answer, 'recomputed')
  if (recomputed.length !== 1) {
    throw new Error(
      `${requestOf(answer)} sent ${String(recomputed.length)} recomputed events, not exactly one.`,
    )
  }
  const status = recomputed[0].features.find((candidate) => candidate.id === feature)
  if (status === undefined) {
    throw new Error(`recomputed has no status for ${feature}`)
  }
  return status
}

/** The codes of the messages of `feature` in `answer`, in order. */
function messageCodes(answer: Answer, feature: string): string[] {
  return featureStatus(answer, feature).messages.map((message) => message.code)
}

/** `value` millimeters, as the reference part writes lengths. */
function millimeters(value: number): string {
  return `${String(value)} mm`
}

/** The lid width of a sample, new for each, so no result a sample times is cached. */
function lidWidth(sample: number): number {
  return 24.5 + 0.5 * sample
}

/** Takes a row's `SAMPLES` samples one after another, `sample` counting from 0, as `median` takes them. */
async function sampled(take: (sample: number) => Promise<number>): Promise<number[]> {
  const samples: number[] = []
  for (let sample = 0; sample < SAMPLES; sample++) {
    // oxlint-disable-next-line no-await-in-loop
    samples.push(await take(sample))
  }
  return samples
}

/** Starts an instance, runs `row` on it and ends it, where a desktop row makes and drops its `Driver`. */
async function withDriver<Result>(
  engine: Engine,
  row: (driver: Driver) => Promise<Result>,
): Promise<Result> {
  const instance = await engine.start()
  const send = (command: Command) => engine.call(instance, command)
  const result = await row({
    send,
    open: (text) => send({ type: 'open_document', text, assets: [] }),
    memoryBytes: () => engine.memoryBytes(instance),
  })
  await engine.end(instance)
  return result
}

/** Opens `text` in a new instance and resolves to `row`'s samples on the document, ending the instance after them. */
function onDocument(
  engine: Engine,
  text: string,
  row: (driver: Driver, doc: number) => Promise<number[]>,
): Promise<number[]> {
  return withDriver(engine, async (driver) => row(driver, docOf(await driver.open(text))))
}

/** Begins a transaction labeled `label` in `doc`. */
async function begin(driver: Driver, doc: number, label: string): Promise<void> {
  ok(await driver.send({ type: 'begin_transaction', doc, label }))
}

/** The samples of `command` sent once for each. */
function timedSamples(driver: Driver, command: Command): Promise<number[]> {
  return sampled(async () => timeOf(await driver.send(command)))
}

/** The reference part's text. */
function referencePart(): Promise<string> {
  return readFile(REFERENCE_PART, 'utf8')
}

/** The recipe of a document text. */
function recipeOf(text: string): Document {
  return JSON.parse(text) as Document
}

/** The text of a document in millimeters with `content`. */
function documentText(content: Omit<Document, 'format' | 'format_version' | 'units'>): string {
  return JSON.stringify({ format: 'partsmith', format_version: 1, units: 'mm', ...content })
}

/** The `update_parameter` edit that gives the last five features' parameter `expr`, keeping its id and comment. */
function lastFiveEdit(text: string, expr: string): Edit {
  const parameter = recipeOf(text).parameters?.find(
    (candidate) => candidate.name === LAST_FIVE_PARAMETER,
  )
  if (parameter === undefined) {
    throw new Error(`the reference part has no parameter ${LAST_FIVE_PARAMETER}`)
  }
  return { type: 'update_parameter', parameter: { ...parameter, expr } }
}

/** The record of `feature` in the reference part. */
function referenceRecord(text: string, feature: string): FeatureRecord {
  const record = recipeOf(text).features?.find((candidate) => candidate.id === feature)
  if (record === undefined) {
    throw new Error(`the reference part has no feature ${feature}`)
  }
  return record
}

/** The edits that make the lid fillet too large for lids `width` mm wide: a radius 1 mm more than half of it. */
function tooLargeLidFillet(text: string, width: number): Edit[] {
  return [
    lastFiveEdit(text, millimeters(width)),
    {
      type: 'update_feature',
      feature: { ...referenceRecord(text, LID_FILLET), radius: millimeters(width / 2 + 1) },
    },
  ]
}

/** The `FixRef` of the `SetField` action the lid fillet's `fillet_too_large` message offers in `answer`. */
function largestRadiusFix(answer: Answer): FixRef {
  const status = featureStatus(answer, LID_FILLET)
  const apply = status.messages
    .filter((message) => message.code === 'fillet_too_large')
    .flatMap((message) => message.actions ?? [])
    .map((action) => action.run)
    .find((run) => run.kind === 'apply')
  if (apply?.kind !== 'apply') {
    throw new Error(`the lid fillet offers no fix: ${JSON.stringify(status)}`)
  }
  return apply.fix
}

/**
 * The reference part's text with every length of its last five features written without a unit (`6 mm` becomes
 * `6`), the `update_feature` edits that write those records so again, and the number of lengths written so: the
 * numbers Keep sizes rewrites when the units change.
 */
function withBareLastFive(text: string): {
  readonly bare: string
  readonly rewrites: Edit[]
  readonly lengths: number
} {
  const recipe = recipeOf(text)
  const features = recipe.features ?? []
  const first = Math.max(features.length - 5, 0)
  const lastFive = features.slice(first).map(withoutMillimeters)
  return {
    bare: JSON.stringify({
      ...recipe,
      features: [...features.slice(0, first), ...lastFive.map(({ record }) => record)],
    }),
    rewrites: lastFive
      .filter(({ written }) => written > 0)
      .map(({ record }) => ({ type: 'update_feature', feature: record })),
    lengths: lastFive.reduce((sum, { written }) => sum + written, 0),
  }
}

/** `record` with every length in millimeters written as the number alone, and how many it wrote so. */
function withoutMillimeters(record: FeatureRecord): {
  readonly record: FeatureRecord
  readonly written: number
} {
  let written = 0
  const bare = JSON.parse(JSON.stringify(record), (_key, value: unknown) => {
    const number = typeof value === 'string' ? MILLIMETERS.exec(value)?.[1] : undefined
    if (number === undefined) {
      return value
    }
    written += 1
    return number
  }) as FeatureRecord
  return { record: bare, written }
}

/** The lid count parameter the loss and ghost rows add, with `expr`. */
function lidCountParameter(expr: string): Parameter {
  return { id: 'p-lid-count', name: 'lid_count', expr }
}

/** The `update_parameter` edit that gives the lid count parameter `expr`. */
function lidCountEdit(expr: string): Edit {
  return { type: 'update_parameter', parameter: lidCountParameter(expr) }
}

/**
 * Makes the count of the reference part's lid pattern the parameter `lid_count` (3) and adds a fillet of 0.2 mm on
 * the first `count` sharp outer edges of the pattern's third copy, in name order (for `LOST_REFERENCES`, its four top
 * edges and one bottom edge), as their short references; resolves to the answer of the last edit.
 */
async function addThirdLidFillet(
  driver: Driver,
  doc: number,
  text: string,
  count: number,
): Promise<Answer> {
  const pattern = referenceRecord(text, LID_PATTERN)
  pattern['direction'] = { ...(pattern['direction'] as object), count: 'lid_count' }
  ok(
    await driver.send({
      type: 'apply_edits',
      doc,
      edits: [
        { type: 'add_parameter', index: 2, parameter: lidCountParameter('3') },
        { type: 'update_feature', feature: pattern },
      ],
    }),
  )
  const listed = await driver.send({ type: 'list_elements', doc, body: LIDS, kind: 'edge' })
  const edges = resultOf(listed, 'list_elements')
    .elements.filter(
      ({ name, reference }) =>
        // Edges between the lid's flat outer faces, so neither is tangent to a round face.
        name.startsWith('edge(f-ext10/') &&
        name.includes('@f-lp4:2') &&
        !name.includes('/fillet/') &&
        reference.fp.startsWith('line|'),
    )
    .slice(0, count)
    .map((element) => element.reference)
  expect(edges, 'straight edges of the third lid').toHaveLength(count)
  const added = await driver.send({
    type: 'apply_edits',
    doc,
    edits: [
      {
        type: 'add_feature',
        index: recipeOf(text).features?.length ?? 0,
        feature: { id: THIRD_LID_FILLET, type: 'fillet', body: LIDS, edges, radius: '0.2 mm' },
      },
    ],
  })
  ok(added)
  return added
}

/**
 * Checks that the message of the fillet on the third lid in `answer` has a cause and that `lost` of its repairs have
 * a ghost, so that a row that loses references times its causes and ghosts (engine.md sections 3.21 and 3.23).
 */
function expectCauseAndGhosts(answer: Answer, lost: number): void {
  const status = featureStatus(answer, THIRD_LID_FILLET)
  const hasCause = Boolean(status.messages[0]?.cause)
  const ghosts = status.repairs.filter((repair) => Boolean(repair.ghost))
  expect(
    hasCause && ghosts.length === lost,
    `the lost references have a cause and ${String(lost)} ghosts: ${JSON.stringify(status)}`,
  ).toBe(true)
}

/** The `apply_edits` of the ghost row that gives the lids `width` and their count `count`. */
function lidState(text: string, doc: number, width: string, count: string): Command {
  return { type: 'apply_edits', doc, edits: [lastFiveEdit(text, width), lidCountEdit(count)] }
}

/** One direction of the grid's pattern: `GRID_COUNT` instances 20 mm apart along `axis`. */
function gridDirection(axis: 'X' | 'Y'): object {
  return { axis: { kind: 'origin', axis }, count: String(GRID_COUNT), spacing: '20 mm' }
}

/**
 * A one-body document whose body is a box sized by the parameter `size` (10 mm), repeated `GRID_COUNT` times along X
 * and along Y: 1014 faces.
 */
function grid(): string {
  return documentText({
    parameters: [{ id: 'p-size', name: 'size', expr: '10 mm' }],
    bodies: [{ id: 'b-1', name: 'Grid' }],
    features: [
      { id: 'f-box1', type: 'box', body: 'b-1', size: ['size', 'size', 'size'], op: 'new' },
      {
        id: 'f-lp1',
        type: 'linear_pattern',
        body: 'b-1',
        direction: gridDirection('X'),
        second_direction: gridDirection('Y'),
      },
    ],
  })
}

/**
 * Gives the grid's box a new size, so its body has a state no query has seen yet; resolves to the body's new model
 * mesh id.
 */
async function regrow(driver: Driver, doc: number, sample: number): Promise<number> {
  const size = millimeters(10 + 0.01 * (sample + 1))
  const answer = await driver.send({
    type: 'apply_edits',
    doc,
    label: 'Size',
    edits: [{ type: 'update_parameter', parameter: { id: 'p-size', name: 'size', expr: size } }],
  })
  return meshId(answer, 'b-1')
}

/** The `pick_reference` of face `index` of the grid's model mesh `mesh`. */
function gridPick(doc: number, mesh: number, index: number): Command {
  return { type: 'pick_reference', doc, pick: { mesh_id: mesh, kind: 'face', index } }
}

/** The `list_elements` of the grid's body. */
function gridList(doc: number): Command {
  return { type: 'list_elements', doc, body: 'b-1' }
}

/**
 * A document with one sketch of `DRAG_RECTANGLES` rectangles, each closed by 4 coincident constraints and squared by 2
 * horizontal and 2 vertical ones.
 */
function dragSketch(): string {
  const entities: object[] = []
  const constraints: object[] = []
  // Each constraint gets the id of its place in the list.
  const constrain = (...added: object[]) => {
    for (const constraint of added) {
      constraints.push({ ...constraint, id: `c-${String(constraints.length)}` })
    }
  }
  for (let rectangle = 0; rectangle < DRAG_RECTANGLES; rectangle++) {
    const x = (rectangle % 9) * 30
    const y = Math.floor(rectangle / 9) * 30
    const corners = [
      [x, y],
      [x + 20, y],
      [x + 20, y + 10],
      [x, y + 10],
    ]
    const lines = [0, 1, 2, 3].map((side) => `e-${String(rectangle)}-${String(side)}`)
    for (const [side, line] of lines.entries()) {
      const next = (side + 1) % 4
      entities.push({ id: line, type: 'line', start: corners[side], end: corners[next] })
      constrain({ type: 'coincident', points: [`${line}.end`, `${lines[next]}.start`] })
    }
    constrain(
      { type: 'horizontal', line: lines[0] },
      { type: 'horizontal', line: lines[2] },
      { type: 'vertical', line: lines[1] },
      { type: 'vertical', line: lines[3] },
    )
  }
  return documentText({
    parameters: [],
    bodies: [],
    features: [
      {
        id: 'f-sk1',
        type: 'sketch',
        plane: { kind: 'origin', plane: 'XY' },
        entities,
        constraints,
      },
    ],
  })
}

/** A sphere of `SPHERE_RADIUS` for the `generic-resin` profile. */
function sphere(): string {
  return documentText({
    printer: 'generic-resin',
    parameters: [],
    bodies: [{ id: 'b-1', name: 'Ball' }],
    features: [{ id: 'f-sph1', type: 'sphere', body: 'b-1', radius: SPHERE_RADIUS, op: 'new' }],
  })
}

/** A document of `MEMORY_FEATURES` boxes, each in its own body. */
function manyBodies(): string {
  const indices = Array.from({ length: MEMORY_FEATURES }, (_, i) => i + 1)
  return documentText({
    parameters: [],
    bodies: indices.map((index) => ({ id: `b-${String(index)}`, name: `Box ${String(index)}` })),
    features: indices.map((index) => ({
      id: `f-box${String(index)}`,
      type: 'box',
      body: `b-${String(index)}`,
      size: ['10', '10', '10'],
      position: [String((index % 20) * 15), String(Math.floor(index / 20) * 15), '0'],
      op: 'new',
    })),
  })
}

/** Row 1: the wasm memory of an instance that opened the 300 boxes. */
function measureMemory(engine: Engine): Promise<number> {
  return withDriver(engine, async (driver) => {
    ok(await driver.open(manyBodies()))
    return driver.memoryBytes()
  })
}

/** Row 2: a new lid width per sample, inside a transaction. */
async function timeParameterChange(engine: Engine): Promise<number[]> {
  const text = await referencePart()
  return onDocument(engine, text, async (driver, doc) => {
    await begin(driver, doc, 'Lid')
    return sampled(async (sample) =>
      timeOf(
        await driver.send({
          type: 'apply_edits',
          doc,
          edits: [lastFiveEdit(text, millimeters(lidWidth(sample)))],
        }),
      ),
    )
  })
}

/** Row 3: the reference part opened by a new instance for each sample. */
async function timeColdOpen(engine: Engine): Promise<number[]> {
  const text = await referencePart()
  return sampled(() => withDriver(engine, async (driver) => timeOf(await driver.open(text))))
}

/** Row 4: the reference part closed and opened again in one instance. */
async function timeCloseAndOpen(engine: Engine): Promise<number[]> {
  const text = await referencePart()
  return onDocument(engine, text, (driver, opened) => {
    let doc = opened
    return sampled(async () => {
      const close = timeOf(await driver.send({ type: 'close_document', doc }))
      const reopened = await driver.open(text)
      doc = docOf(reopened)
      return close + reopened.milliseconds
    })
  })
}

/** Row 5: the reference part exported to one binary STL file. */
async function timeStlExport(engine: Engine): Promise<number[]> {
  return onDocument(engine, await referencePart(), (driver, doc) =>
    timedSamples(driver, {
      type: 'export',
      doc,
      format: { kind: 'stl', binary: true, per_body: false },
    }),
  )
}

/** Row 6: the reference part written as Python. */
async function timePythonExport(engine: Engine): Promise<number[]> {
  return onDocument(engine, await referencePart(), (driver, doc) =>
    timedSamples(driver, { type: 'export_python', doc }),
  )
}

/** Row 7: the lid fillet made too large for a new lid width per sample, then fixed with its largest radius. */
async function timeFix(engine: Engine): Promise<number[]> {
  const text = await referencePart()
  return onDocument(engine, text, async (driver, doc) => {
    await begin(driver, doc, 'Lid')
    return sampled(async (sample) => {
      // A new lid width per sample, so the fixed fillet is never a cached result.
      const broken = await driver.send({
        type: 'apply_edits',
        doc,
        edits: tooLargeLidFillet(text, lidWidth(sample)),
      })
      const fixed = await driver.send({ type: 'apply_fix', doc, fix: largestRadiusFix(broken) })
      const time = timeOf(fixed)
      expect(messageCodes(fixed, LID_FILLET), 'the largest radius builds').toEqual([])
      return time
    })
  })
}

/** Row 8: the lid count from 3 to 2, which loses the five edges of the fillet on the third lid. */
async function timeLoss(engine: Engine): Promise<number[]> {
  const text = await referencePart()
  return onDocument(engine, text, async (driver, doc) => {
    await begin(driver, doc, 'Lids')
    const added = await addThirdLidFillet(driver, doc, text, LOST_REFERENCES)
    expect(messageCodes(added, THIRD_LID_FILLET), 'the fillet on the third lid builds').toEqual([])
    return sampled(async (sample) => {
      // A new lid width per sample, so neither the pattern nor the losses are cached.
      ok(
        await driver.send({
          type: 'apply_edits',
          doc,
          edits: [lastFiveEdit(text, millimeters(lidWidth(sample))), lidCountEdit('3')],
        }),
      )
      const lost = await driver.send({ type: 'apply_edits', doc, edits: [lidCountEdit('2')] })
      const time = timeOf(lost)
      expect(messageCodes(lost, THIRD_LID_FILLET), "the third lid's edges are lost").toEqual([
        'references_not_found',
      ])
      expectCauseAndGhosts(lost, LOST_REFERENCES)
      return time
    })
  })
}

/** Row 9: the units from mm to in with Keep sizes, with the last five features' lengths bare again before each. */
async function timeUnitsKeepingSizes(engine: Engine): Promise<number[]> {
  const { bare, rewrites, lengths } = withBareLastFive(await referencePart())
  expect(lengths, 'the last five features hold lengths').toBeGreaterThan(0)
  return onDocument(engine, bare, async (driver, doc) => {
    await begin(driver, doc, 'Units')
    return sampled(async (sample) => {
      // A new lid width per sample, so no record Keep sizes rewrites is a cached result, and the lengths without a
      // unit again, for Keep sizes to rewrite.
      ok(
        await driver.send({
          type: 'apply_edits',
          doc,
          edits: [lastFiveEdit(bare, millimeters(lidWidth(sample))), ...rewrites],
        }),
      )
      const changed = await driver.send({
        type: 'set_document_units',
        doc,
        units: 'in',
        keep_sizes: true,
      })
      expect(
        resultOf(changed, 'set_document_units').kept,
        'Keep sizes rewrites the lengths of the last five features',
      ).toBe(lengths)
      ok(await driver.send({ type: 'set_document_units', doc, units: 'mm', keep_sizes: false }))
      return changed.milliseconds
    })
  })
}

/** Row 10: `list_uses` of the reference part. */
async function timeUses(engine: Engine): Promise<number[]> {
  return onDocument(engine, await referencePart(), (driver, doc) =>
    sampled(async () => {
      const listed = await driver.send({ type: 'list_uses', doc })
      expect(
        resultOf(listed, 'list_uses').uses.length,
        "the reference part's features use others",
      ).toBeGreaterThan(0)
      return listed.milliseconds
    }),
  )
}

/**
 * Row 11: two cached `apply_edits` that end in the state where the third lid's filleted edge is lost, one capturing
 * its ghost and one keeping it; each sample is the first time less the second, at least 0.
 */
async function timeGhostCapture(engine: Engine): Promise<number[]> {
  const text = await referencePart()
  return onDocument(engine, text, async (driver, doc) => {
    await begin(driver, doc, 'Lids')
    await addThirdLidFillet(driver, doc, text, 1)
    const [width, otherWidth] = ['24.5 mm', '25 mm']
    // Each state the samples visit, once, so that every timed recompute is cached.
    for (const [lids, count] of [
      [width, '2'],
      [otherWidth, '2'],
      [width, '3'],
    ]) {
      // oxlint-disable-next-line no-await-in-loop
      ok(await driver.send(lidState(text, doc, lids, count)))
    }
    return sampled(async () => {
      const capturing = await driver.send(lidState(text, doc, width, '2'))
      expectCauseAndGhosts(capturing, 1)
      ok(await driver.send(lidState(text, doc, otherWidth, '2')))
      const keeping = await driver.send(lidState(text, doc, width, '2'))
      expectCauseAndGhosts(keeping, 1)
      ok(await driver.send(lidState(text, doc, width, '3')))
      return Math.max(capturing.milliseconds - keeping.milliseconds, 0)
    })
  })
}

/** Row 12: one drag frame of a point of the 504-constraint sketch to a new place per sample. */
function timeDrag(engine: Engine): Promise<number[]> {
  return onDocument(engine, dragSketch(), async (driver, doc) => {
    const frame = (to: number) =>
      driver.send({ type: 'drag_sketch', doc, feature: 'f-sk1', point: 'e-0-0.end', to: [to, 3] })
    ok(await frame(20))
    return sampled(async (sample) => timeOf(await frame(20.5 + 0.5 * sample)))
  })
}

/** Row 13: a pick in a new state of the grid for each sample. */
function timeFirstPicks(engine: Engine): Promise<number[]> {
  return onDocument(engine, grid(), (driver, doc) =>
    sampled(async (sample) => {
      const mesh = await regrow(driver, doc, sample)
      return timeOf(await driver.send(gridPick(doc, mesh, 500)))
    }),
  )
}

/** Row 14: picks of other faces in one new state of the grid, after a first pick in it. */
function timeLaterPicks(engine: Engine): Promise<number[]> {
  return onDocument(engine, grid(), async (driver, doc) => {
    const mesh = await regrow(driver, doc, 0)
    ok(await driver.send(gridPick(doc, mesh, 0)))
    return sampled(async (sample) => timeOf(await driver.send(gridPick(doc, mesh, 500 + sample))))
  })
}

/** Row 15: the grid's elements listed in a new state for each sample. */
function timeFirstLists(engine: Engine): Promise<number[]> {
  return onDocument(engine, grid(), (driver, doc) =>
    sampled(async (sample) => {
      await regrow(driver, doc, sample)
      return timeOf(await driver.send(gridList(doc)))
    }),
  )
}

/** Row 16: the grid's elements listed again in a state listed before. */
function timeRepeatedLists(engine: Engine): Promise<number[]> {
  return onDocument(engine, grid(), async (driver, doc) => {
    ok(await driver.send(gridList(doc)))
    return timedSamples(driver, gridList(doc))
  })
}

/** Row 17: the print analysis of the resin sphere as modeled. */
function timePrintAnalysis(engine: Engine): Promise<number[]> {
  return onDocument(engine, sphere(), (driver, doc) =>
    timedSamples(driver, { type: 'analyze_print', doc, orientation: { kind: 'as_modeled' } }),
  )
}

/**
 * Opens the reference part in a new instance and ends it, untimed, before the rows (engine.md section 5): the code
 * that open runs is then optimized for every row, while code only one row runs warms up in that row's first samples.
 */
export async function warmUp(engine: Engine): Promise<void> {
  const text = await referencePart()
  await withDriver(engine, async (driver) => {
    ok(await driver.open(text))
  })
}

/** Row 1 of engine.md section 5's list, which the desktop bench measures as resident memory. */
export const MEMORY_ROW: MemoryRow = {
  name: 'wasm memory with a 300-feature document open',
  measure: measureMemory,
}

/** Rows 2 to 17 of engine.md section 5's list, in its order, each named as the desktop bench names it. */
export const TIMED_ROWS: readonly TimedRow[] = [
  {
    name: 'apply_edits inside a transaction changing one parameter that affects the last 5 features of the reference part',
    budget: 250,
    measure: timeParameterChange,
  },
  {
    name: 'open_document of the reference part, cold session',
    budget: 3_000,
    measure: timeColdOpen,
  },
  {
    name: 'close_document then open_document of the same text in the same session',
    budget: 1_000,
    measure: timeCloseAndOpen,
  },
  {
    name: 'export of the reference part to binary STL at standard quality',
    budget: 900,
    measure: timeStlExport,
  },
  { name: 'export_python of the reference part', budget: 50, measure: timePythonExport },
  {
    name: 'apply_fix of a SetField of one of the last 5 features of the reference part, inside a transaction',
    budget: 250,
    measure: timeFix,
  },
  {
    name: 'apply_edits inside a transaction of a parameter change that loses 5 references of the last features of the reference part',
    budget: 250,
    measure: timeLoss,
  },
  {
    name: 'set_document_units with keep_sizes on the reference part, including the edit set, recompute, tessellation of changed bodies and frame encoding',
    budget: 250,
    measure: timeUnitsKeepingSizes,
  },
  { name: 'list_uses of the reference part', budget: 15, measure: timeUses },
  {
    name: 'ghost capture of one lost edge on the reference part',
    budget: 15,
    measure: timeGhostCapture,
  },
  {
    name: 'drag_sketch frame on a 500-constraint sketch, including frame encoding',
    budget: 12,
    measure: timeDrag,
  },
  {
    name: 'pick_reference, first pick in a new state of a 1000-face body',
    budget: 2,
    measure: timeFirstPicks,
  },
  {
    name: 'pick_reference, later picks in the same state',
    budget: 2,
    measure: timeLaterPicks,
  },
  {
    name: 'list_elements of a 1000-face body, first call for a new state',
    budget: 25,
    measure: timeFirstLists,
  },
  {
    name: 'list_elements of a 1000-face body, repeated',
    budget: 25,
    measure: timeRepeatedLists,
  },
  {
    name: 'analyze_print of one 200 000-triangle body',
    budget: 900,
    measure: timePrintAnalysis,
  },
]
