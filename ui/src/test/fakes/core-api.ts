import type { CoreApi } from '../../core/api'
import { CoreError, createCoreClient } from '../../core/client'
import type { Transport } from '../../core/transport'
import { PROTOCOL_VERSION } from '../../generated/protocol'
import { TEST_CATALOG_ENTRIES } from './catalog'
import { evaluateExpression, resolvedWithoutMatches } from './core'
import { TEST_PRINTERS } from './printers'
import { readRequest, responseFrame, type ErrorReply } from './protocol'

// A CoreApi for tests that build an App by hand (createTestApp) or test code that only needs a few
// core answers. Every call is recorded; each method answers with a plain default until a test stubs it.

/** One recorded CoreApi call. */
export type CoreApiCall = { readonly method: keyof CoreApi; readonly args: readonly unknown[] }

export type FakeCoreApi = CoreApi & {
  /** Every call so far, in call order. */
  readonly calls: readonly CoreApiCall[]
  /** The argument lists of the calls to one method, in call order. */
  callsTo(method: keyof CoreApi): readonly (readonly unknown[])[]
  /** Replaces one method's answers for later calls. */
  stub<Method extends keyof CoreApi>(method: Method, implementation: CoreApi[Method]): void
}

/**
 * A real `CoreError` for an error reply, made the way the app makes them: by the core client
 * decoding an error response frame. Tests use it to reject from stubbed CoreApi methods.
 */
export async function coreErrorFor(error: ErrorReply): Promise<CoreError> {
  const transport: Transport = {
    kind: 'worker',
    start: async () => ({ protocolVersion: PROTOCOL_VERSION, engineVersion: '0.1.0' }),
    send: async (frame) => responseFrame({ id: readRequest(frame).id, error }),
    stop: () => undefined,
    onFatal: () => () => undefined,
  }
  const client = createCoreClient(transport)
  try {
    await client.request({ type: 'engine_info' })
  } catch (rejection) {
    if (rejection instanceof CoreError) {
      return rejection
    }
    throw rejection
  } finally {
    client.close()
  }
  throw new Error('The core client resolved an error response.')
}

export function createFakeCoreApi(stubs: Partial<CoreApi> = {}): FakeCoreApi {
  const calls: CoreApiCall[] = []
  let revision = 0
  let nextHandle = 1

  const defaults: CoreApi = {
    engineInfo: async () => ({
      protocolVersion: PROTOCOL_VERSION,
      engineVersion: '0.1.0',
      formatVersion: 1,
      kernel: 'Fake kernel',
    }),
    featureCatalog: async () => TEST_CATALOG_ENTRIES,
    listPrinterProfiles: async () => TEST_PRINTERS,
    newDocument: async () => nextHandle++,
    openDocument: async () => ({ documentHandle: nextHandle++, migratedFrom: null }),
    saveDocument: async () => ({ text: '', revision }),
    markSaved: async () => undefined,
    closeDocument: async () => undefined,
    applyEdits: async () => ++revision,
    renameParameter: async () => ++revision,
    beginTransaction: async () => undefined,
    setFocus: async () => undefined,
    commitTransaction: async () => ({ revision: ++revision, recorded: true }),
    cancelTransaction: async () => ++revision,
    undo: async () => ({ revision: ++revision, label: 'Edit' }),
    redo: async () => ({ revision: ++revision, label: 'Edit' }),
    solveSketch: async () => null,
    dragSketch: async () => null,
    endSketchDrag: async () => ++revision,
    evaluate: async (_documentHandle, expression) => {
      const result = evaluateExpression(expression, new Map())
      if ('error' in result) {
        throw await coreErrorFor({ code: 'expression_error', message: result.error })
      }
      return result
    },
    listElements: async () => [],
    pickReference: async () => {
      throw await coreErrorFor({
        code: 'stale_pick',
        message: 'The picked geometry changed since it was displayed. Pick again.',
      })
    },
    resolveReferences: async (_documentHandle, references) =>
      references.map(resolvedWithoutMatches),
    measure: async () => ({}),
    analyzePrint: async () => null,
    exportBodies: async () => ({ files: [], warnings: [] }),
  }
  const implementation: CoreApi = { ...defaults, ...stubs }

  function record(method: keyof CoreApi, args: readonly unknown[]): void {
    calls.push({ method, args })
  }

  return {
    get calls() {
      return calls
    },
    callsTo(method) {
      return calls.filter((call) => call.method === method).map((call) => call.args)
    },
    stub(method, replacement) {
      implementation[method] = replacement
    },
    engineInfo: (...args: Parameters<CoreApi['engineInfo']>) => {
      record('engineInfo', args)
      return implementation.engineInfo(...args)
    },
    featureCatalog: (...args: Parameters<CoreApi['featureCatalog']>) => {
      record('featureCatalog', args)
      return implementation.featureCatalog(...args)
    },
    listPrinterProfiles: (...args: Parameters<CoreApi['listPrinterProfiles']>) => {
      record('listPrinterProfiles', args)
      return implementation.listPrinterProfiles(...args)
    },
    newDocument: (...args: Parameters<CoreApi['newDocument']>) => {
      record('newDocument', args)
      return implementation.newDocument(...args)
    },
    openDocument: (...args: Parameters<CoreApi['openDocument']>) => {
      record('openDocument', args)
      return implementation.openDocument(...args)
    },
    saveDocument: (...args: Parameters<CoreApi['saveDocument']>) => {
      record('saveDocument', args)
      return implementation.saveDocument(...args)
    },
    markSaved: (...args: Parameters<CoreApi['markSaved']>) => {
      record('markSaved', args)
      return implementation.markSaved(...args)
    },
    closeDocument: (...args: Parameters<CoreApi['closeDocument']>) => {
      record('closeDocument', args)
      return implementation.closeDocument(...args)
    },
    applyEdits: (...args: Parameters<CoreApi['applyEdits']>) => {
      record('applyEdits', args)
      return implementation.applyEdits(...args)
    },
    renameParameter: (...args: Parameters<CoreApi['renameParameter']>) => {
      record('renameParameter', args)
      return implementation.renameParameter(...args)
    },
    beginTransaction: (...args: Parameters<CoreApi['beginTransaction']>) => {
      record('beginTransaction', args)
      return implementation.beginTransaction(...args)
    },
    setFocus: (...args: Parameters<CoreApi['setFocus']>) => {
      record('setFocus', args)
      return implementation.setFocus(...args)
    },
    commitTransaction: (...args: Parameters<CoreApi['commitTransaction']>) => {
      record('commitTransaction', args)
      return implementation.commitTransaction(...args)
    },
    cancelTransaction: (...args: Parameters<CoreApi['cancelTransaction']>) => {
      record('cancelTransaction', args)
      return implementation.cancelTransaction(...args)
    },
    undo: (...args: Parameters<CoreApi['undo']>) => {
      record('undo', args)
      return implementation.undo(...args)
    },
    redo: (...args: Parameters<CoreApi['redo']>) => {
      record('redo', args)
      return implementation.redo(...args)
    },
    solveSketch: (...args: Parameters<CoreApi['solveSketch']>) => {
      record('solveSketch', args)
      return implementation.solveSketch(...args)
    },
    dragSketch: (...args: Parameters<CoreApi['dragSketch']>) => {
      record('dragSketch', args)
      return implementation.dragSketch(...args)
    },
    endSketchDrag: (...args: Parameters<CoreApi['endSketchDrag']>) => {
      record('endSketchDrag', args)
      return implementation.endSketchDrag(...args)
    },
    evaluate: (...args: Parameters<CoreApi['evaluate']>) => {
      record('evaluate', args)
      return implementation.evaluate(...args)
    },
    listElements: (...args: Parameters<CoreApi['listElements']>) => {
      record('listElements', args)
      return implementation.listElements(...args)
    },
    pickReference: (...args: Parameters<CoreApi['pickReference']>) => {
      record('pickReference', args)
      return implementation.pickReference(...args)
    },
    resolveReferences: (...args: Parameters<CoreApi['resolveReferences']>) => {
      record('resolveReferences', args)
      return implementation.resolveReferences(...args)
    },
    measure: (...args: Parameters<CoreApi['measure']>) => {
      record('measure', args)
      return implementation.measure(...args)
    },
    analyzePrint: (...args: Parameters<CoreApi['analyzePrint']>) => {
      record('analyzePrint', args)
      return implementation.analyzePrint(...args)
    },
    exportBodies: (...args: Parameters<CoreApi['exportBodies']>) => {
      record('exportBodies', args)
      return implementation.exportBodies(...args)
    },
  }
}
