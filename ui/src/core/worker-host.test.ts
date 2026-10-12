import { describe, expect, it } from 'vitest'
import type { CommandResult, Request } from '../generated/protocol'
import { createFakeCoreModule, type FakeCoreModule } from '../test/fakes/core-module'
import { readRequest, readResponse, requestFrame, responseFrame } from '../test/fakes/protocol'
import { settle } from '../test/fakes/settle'
import type { CoreModule } from './bridge'
import { decodeFrame } from './frame'
import { createWorkerHost, type FromWorker, type WorkerHost } from './worker-host'

type Posted = { readonly message: FromWorker; readonly transfer: readonly Transferable[] }

function resultFor(request: Request, protocolVersion: number): CommandResult {
  const { command } = request
  switch (command.type) {
    case 'engine_info':
      return {
        type: 'engine_info',
        protocol_version: protocolVersion,
        engine_version: '0.1.0',
        format_version: 1,
        kernel: 'OCCT 7.9.3',
      }
    case 'drag_sketch':
      return {
        type: 'drag_sketch',
        solution: {
          feature: command.feature,
          status: 'under_constrained',
          failed: false,
          dof: 1,
          redundant: [],
          conflicting: [],
          placement: { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] },
          entities: [],
          points: [],
          constraints: [],
          regions: [],
          profile_findings: [],
        },
      }
    case 'analyze_print':
      return { type: 'analyze_print', health: 'ok' }
    case 'undo':
      return { type: 'undo', revision: request.id, label: 'Edit' }
    default:
      return { type: 'apply_edits', revision: request.id }
  }
}

type HostSetup = {
  readonly host: WorkerHost
  readonly module: FakeCoreModule
  readonly posted: Posted[]
  /** Runs scheduled steps until none is left, letting promises settle between them. */
  runScheduled(): Promise<void>
  /** The request ids the core was called with, without the host's own engine_info (id 0). */
  calledIds(): number[]
  /** Request ids of the posted frames, in posting order. */
  frameIds(): number[]
}

function setUp(
  options: {
    readonly protocolVersion?: number
    readonly load?: (module: CoreModule) => Promise<CoreModule>
  } = {},
): HostSetup {
  const module = createFakeCoreModule((frame) => {
    const request = readRequest(frame)
    return responseFrame({ id: request.id, result: resultFor(request, options.protocolVersion ?? 1) })
  })
  const posted: Posted[] = []
  const tasks: (() => void)[] = []
  const host = createWorkerHost({
    load: () => (options.load === undefined ? Promise.resolve(module) : options.load(module)),
    post: (message, transfer) => {
      posted.push({ message, transfer })
    },
    schedule: (task) => {
      tasks.push(task)
    },
  })
  return {
    host,
    module,
    posted,
    async runScheduled() {
      await settle()
      for (let task = tasks.shift(); task !== undefined; task = tasks.shift()) {
        task()
        await settle()
      }
    },
    calledIds() {
      return module.requests.map((frame) => readRequest(frame).id).filter((id) => id !== 0)
    },
    frameIds() {
      return posted.flatMap(({ message }) =>
        message.type === 'frame' ? [readResponse(message.frame).response.id] : [],
      )
    },
  }
}

function drag(id: number, feature = 'f-sk1'): ArrayBuffer {
  return requestFrame(id, { type: 'drag_sketch', doc: 1, feature, point: 'e-1.end', to: [20, 5] })
}

function edit(id: number): ArrayBuffer {
  return requestFrame(id, {
    type: 'apply_edits',
    doc: 1,
    label: 'Change units to cm',
    edits: [{ type: 'set_units', units: 'cm' }],
  })
}

function analyze(id: number): ArrayBuffer {
  return requestFrame(id, { type: 'analyze_print', doc: 1, orientation: { kind: 'as_modeled' } })
}

function errorCodeOf(frame: ArrayBuffer): string | null {
  const { result } = readResponse(frame).response
  return result.kind === 'error' ? result.code : null
}

async function readyHost(options: Parameters<typeof setUp>[0] = {}): Promise<HostSetup> {
  const setup = setUp(options)
  await setup.runScheduled()
  expect(setup.posted[0]?.message.type).toBe('ready')
  return setup
}

describe('createWorkerHost', () => {
  it('protocol test 65: posts ready with the core versions first, then answers calls received before it, in order', async () => {
    let finishLoading: (() => void) | null = null
    const setup = setUp({
      load: (module) =>
        new Promise((resolve) => {
          finishLoading = () => {
            resolve(module)
          }
        }),
    })
    setup.host.receive({ type: 'call', frame: edit(1) })
    setup.host.receive({ type: 'call', frame: edit(2) })
    await setup.runScheduled()
    expect(setup.posted).toEqual([])
    finishLoading?.()
    await setup.runScheduled()
    expect(setup.posted[0]?.message).toEqual({
      type: 'ready',
      protocolVersion: 1,
      engineVersion: '0.1.0',
    })
    expect(setup.frameIds()).toEqual([1, 2])
    expect(setup.posted).toHaveLength(3)
  })

  it('calls engine_info with the reserved request id 0 before posting ready', async () => {
    const setup = await readyHost()
    const first = setup.module.requests[0]
    expect(first).toBeDefined()
    expect(readRequest(first ?? new ArrayBuffer(0))).toEqual({ id: 0, command: { type: 'engine_info' } })
  })

  it('protocol test 58: a queued drag_sketch superseded by a later one is answered with superseded, and only the later one reaches the core', async () => {
    const setup = await readyHost()
    setup.host.receive({ type: 'call', frame: drag(10) })
    setup.host.receive({ type: 'call', frame: drag(11) })
    await setup.runScheduled()
    expect(setup.calledIds()).toEqual([11])
    const frames = setup.posted.slice(1).map(({ message }) => message)
    expect(frames.map((message) => message.type)).toEqual(['frame', 'frame'])
    expect(setup.frameIds()).toEqual([10, 11])
    const [first, second] = frames
    expect(first?.type === 'frame' ? errorCodeOf(first.frame) : 'not a frame').toBe('superseded')
    expect(second?.type === 'frame' ? errorCodeOf(second.frame) : 'not a frame').toBeNull()
  })

  it('protocol test 59: queued drags of two different sketches both run', async () => {
    const setup = await readyHost()
    setup.host.receive({ type: 'call', frame: drag(10, 'f-sk1') })
    setup.host.receive({ type: 'call', frame: drag(11, 'f-sk2') })
    await setup.runScheduled()
    expect(setup.calledIds()).toEqual([10, 11])
  })

  it('protocol test 60: a request for the same document between two drags keeps both drags', async () => {
    const setup = await readyHost()
    setup.host.receive({ type: 'call', frame: drag(10) })
    setup.host.receive({ type: 'call', frame: edit(11) })
    setup.host.receive({ type: 'call', frame: drag(12) })
    await setup.runScheduled()
    expect(setup.calledIds()).toEqual([10, 11, 12])
  })

  it('protocol test 61: an analyze_print is superseded by a later one even across an edit', async () => {
    const setup = await readyHost()
    setup.host.receive({ type: 'call', frame: analyze(20) })
    setup.host.receive({ type: 'call', frame: edit(21) })
    setup.host.receive({ type: 'call', frame: analyze(22) })
    await setup.runScheduled()
    expect(setup.calledIds()).toEqual([21, 22])
    expect(setup.frameIds()).toEqual([20, 21, 22])
  })

  it('protocol test 62: answers ids 1 to 5 in order, each frame transferred', async () => {
    const setup = await readyHost()
    for (let id = 1; id <= 5; id += 1) {
      setup.host.receive({ type: 'call', frame: edit(id) })
    }
    await setup.runScheduled()
    expect(setup.frameIds()).toEqual([1, 2, 3, 4, 5])
    for (const { message, transfer } of setup.posted.slice(1)) {
      expect(message.type).toBe('frame')
      if (message.type === 'frame') {
        expect(transfer).toContain(message.frame)
      }
    }
  })

  it('protocol test 63: the synthesized superseded frame has exactly the superseded error response', async () => {
    const setup = await readyHost()
    setup.host.receive({ type: 'call', frame: drag(10) })
    setup.host.receive({ type: 'call', frame: drag(11) })
    await setup.runScheduled()
    const message = setup.posted[1]?.message
    expect(message?.type).toBe('frame')
    if (message?.type !== 'frame') {
      return
    }
    expect(decodeFrame(message.frame).header).toEqual({
      protocol_version: 1,
      messages: [
        {
          kind: 'response',
          id: 10,
          result: { kind: 'error', code: 'superseded', message: 'A newer request replaced this one.' },
        },
      ],
      buffers: [],
    })
  })

  it('protocol test 64: a failed load posts fatal with the reason, and later calls get fatal instead of a frame', async () => {
    const setup = setUp({ load: () => Promise.reject(new Error('404')) })
    await setup.runScheduled()
    expect(setup.posted).toHaveLength(1)
    const fatal = setup.posted[0]?.message
    expect(fatal?.type).toBe('fatal')
    expect(fatal?.type === 'fatal' ? fatal.message : '').toContain('404')
    setup.host.receive({ type: 'call', frame: edit(1) })
    await setup.runScheduled()
    expect(setup.posted.map(({ message }) => message.type)).toEqual(['fatal', 'fatal'])
  })

  it('a core speaking another protocol version is fatal', async () => {
    const setup = setUp({ protocolVersion: 2 })
    await setup.runScheduled()
    expect(setup.posted.map(({ message }) => message.type)).toEqual(['fatal'])
    setup.host.receive({ type: 'call', frame: edit(1) })
    await setup.runScheduled()
    expect(setup.posted.map(({ message }) => message.type)).toEqual(['fatal', 'fatal'])
    expect(setup.calledIds()).toEqual([])
  })

  it('protocol test 66: a WebAssembly trap during a call posts fatal and later calls get fatal', async () => {
    const setup = await readyHost()
    setup.module.callError = new WebAssembly.RuntimeError('unreachable')
    setup.host.receive({ type: 'call', frame: edit(1) })
    await setup.runScheduled()
    expect(setup.posted.map(({ message }) => message.type)).toEqual(['ready', 'fatal'])
    setup.module.callError = null
    setup.host.receive({ type: 'call', frame: edit(2) })
    await setup.runScheduled()
    expect(setup.posted.map(({ message }) => message.type)).toEqual(['ready', 'fatal', 'fatal'])
    expect(setup.frameIds()).toEqual([])
  })

  it('running out of memory during a call is fatal', async () => {
    const setup = await readyHost()
    setup.module.failAllocation = true
    setup.host.receive({ type: 'call', frame: edit(1) })
    await setup.runScheduled()
    expect(setup.posted.map(({ message }) => message.type)).toEqual(['ready', 'fatal'])
  })
})
