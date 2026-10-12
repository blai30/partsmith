import type {
  CatalogEntry,
  Command,
  CommandResult,
  Diagnostic,
  Dimension,
  DocHandle,
  Edit,
  ElementDescriptor,
  ElementKind,
  ElementRef,
  ExportFormat,
  Health,
  Orientation,
  Pick,
  PointRef,
  PrinterProfile,
  Resolution,
  SketchParams,
  SketchSolution,
  Units,
} from '../generated/protocol'
import { CoreError, type CoreClient } from './client'
import { byteBuffer, sketchDisplayData, type SketchDisplayData } from './events'
import type { BufferView } from './frame'
import { CoreFatalError } from './transport'

// Typed wrappers around every command the M1 UI uses (ui.md 2.6). This is the only module that names
// protocol commands.

/** A sketch solution with its region fill buffers (2.7). */
export type SolvedSketch = {
  readonly solution: SketchSolution
  readonly display: SketchDisplayData | null
}

/** An export file with its bytes resolved from the frame buffer. */
export type ExportedFile = {
  readonly name: string
  readonly mediaType: string
  readonly bytes: Uint8Array
}

export type CoreApi = {
  engineInfo(): Promise<{
    protocolVersion: number
    engineVersion: string
    formatVersion: number
    kernel: string
  }>
  featureCatalog(): Promise<readonly CatalogEntry[]>
  listPrinterProfiles(): Promise<readonly PrinterProfile[]>
  newDocument(units?: Units): Promise<DocHandle>
  openDocument(text: string): Promise<{ documentHandle: DocHandle; migratedFrom: number | null }>
  saveDocument(documentHandle: DocHandle): Promise<{ text: string; revision: number }>
  markSaved(documentHandle: DocHandle, revision: number): Promise<void>
  closeDocument(documentHandle: DocHandle): Promise<void>
  applyEdits(
    documentHandle: DocHandle,
    label: string | null,
    edits: readonly Edit[],
  ): Promise<number>
  renameParameter(documentHandle: DocHandle, parameter: string, name: string): Promise<number>
  beginTransaction(documentHandle: DocHandle, label: string): Promise<void>
  setFocus(documentHandle: DocHandle, feature: string | null): Promise<void>
  commitTransaction(documentHandle: DocHandle): Promise<{ revision: number; recorded: boolean }>
  cancelTransaction(documentHandle: DocHandle): Promise<number>
  undo(documentHandle: DocHandle): Promise<{ revision: number; label: string }>
  redo(documentHandle: DocHandle): Promise<{ revision: number; label: string }>
  solveSketch(
    documentHandle: DocHandle,
    feature: string,
    candidate?: SketchParams,
  ): Promise<SolvedSketch | null>
  dragSketch(
    documentHandle: DocHandle,
    feature: string,
    point: PointRef,
    to: readonly [number, number],
  ): Promise<SolvedSketch | null>
  endSketchDrag(documentHandle: DocHandle, feature: string, commit: boolean): Promise<number>
  evaluate(
    documentHandle: DocHandle,
    expression: string,
  ): Promise<{ value: number; dimension: Dimension }>
  listElements(
    documentHandle: DocHandle,
    body: string,
    kind?: ElementKind,
  ): Promise<readonly ElementDescriptor[]>
  pickReference(documentHandle: DocHandle, pick: Pick): Promise<ElementDescriptor>
  resolveReferences(
    documentHandle: DocHandle,
    references: readonly ElementRef[],
  ): Promise<readonly Resolution[]>
  analyzePrint(
    documentHandle: DocHandle,
    options: { profile?: string; orientation: Orientation },
  ): Promise<Health | null>
  exportBodies(
    documentHandle: DocHandle,
    format: ExportFormat,
    bodies?: readonly string[],
  ): Promise<{ files: readonly ExportedFile[]; warnings: readonly Diagnostic[] }>
}

/**
 * Commands after which the core rebuilds a document: they change its recipe, focus or what it
 * shows, so it recomputes features and remeshes bodies (engine.md 3.3). Saving, closing, starting a
 * transaction, new_document (nothing to build) and queries rebuild nothing.
 */
const REBUILDING_COMMANDS: ReadonlySet<Command['type']> = new Set([
  'open_document',
  'apply_edits',
  'rename_parameter',
  'set_focus',
  'commit_transaction',
  'cancel_transaction',
  'undo',
  'redo',
  'end_sketch_drag',
])

/** Whether the core rebuilds a document for this command (ui.md 2.8 `pendingMutations`, 3.21). */
export function rebuildsDocument(command: Command): boolean {
  return REBUILDING_COMMANDS.has(command.type)
}

/**
 * The document a rebuilding command rebuilt, named by the command, or by the result of
 * open_document, which creates it; null for a command that rebuilds nothing.
 */
export function rebuiltDocument(command: Command, result: CommandResult): DocHandle | null {
  if (!rebuildsDocument(command)) {
    return null
  }
  if ('doc' in command) {
    return command.doc
  }
  return result.type === 'open_document' ? result.doc : null
}

type CommandOfType<Type extends Command['type']> = Extract<Command, { type: Type }>
type ResultOf<Type extends Command['type']> = Extract<CommandResult, { type: Type }>
type TypedReply<Type extends Command['type']> = {
  readonly result: ResultOf<Type>
  readonly buffers: readonly BufferView[]
}

function isResultOf<Type extends Command['type']>(
  result: CommandResult,
  type: Type,
): result is ResultOf<Type> {
  return result.type === type
}

/** A superseded answer to a request that a newer one replaced (protocol.md 2.11). */
async function unlessSuperseded<Value>(request: Promise<Value>): Promise<Value | null> {
  try {
    return await request
  } catch (error) {
    if (error instanceof CoreError && error.code === 'superseded') {
      return null
    }
    throw error
  }
}

export function createCoreApi(client: CoreClient): CoreApi {
  async function call<Type extends Command['type']>(
    command: CommandOfType<Type>,
  ): Promise<TypedReply<Type>> {
    const reply = await client.request(command)
    const { result } = reply
    if (!isResultOf(result, command.type)) {
      throw new CoreFatalError(
        `The geometry core answered ${command.type} with a ${result.type} result.`,
      )
    }
    return { result, buffers: reply.buffers }
  }

  async function solved(
    command: CommandOfType<'solve_sketch'> | CommandOfType<'drag_sketch'>,
  ): Promise<SolvedSketch> {
    const { result, buffers } = await call(command)
    return {
      solution: result.solution,
      display: sketchDisplayData(result.solution.display, buffers),
    }
  }

  return {
    async engineInfo() {
      const { result } = await call({ type: 'engine_info' })
      return {
        protocolVersion: result.protocol_version,
        engineVersion: result.engine_version,
        formatVersion: result.format_version,
        kernel: result.kernel,
      }
    },
    async featureCatalog() {
      return (await call({ type: 'feature_catalog' })).result.features
    },
    async listPrinterProfiles() {
      return (await call({ type: 'list_printer_profiles' })).result.profiles
    },
    async newDocument(units) {
      const command: CommandOfType<'new_document'> =
        units === undefined ? { type: 'new_document' } : { type: 'new_document', units }
      return (await call(command)).result.doc
    },
    async openDocument(text) {
      const { result } = await call({ type: 'open_document', text, assets: [] })
      return { documentHandle: result.doc, migratedFrom: result.migrated_from ?? null }
    },
    async saveDocument(documentHandle) {
      const { result } = await call({ type: 'save_document', doc: documentHandle })
      return { text: result.text, revision: result.revision }
    },
    async markSaved(documentHandle, revision) {
      await call({ type: 'mark_saved', doc: documentHandle, revision })
    },
    async closeDocument(documentHandle) {
      await call({ type: 'close_document', doc: documentHandle })
    },
    async applyEdits(documentHandle, label, edits) {
      const command: CommandOfType<'apply_edits'> =
        label === null
          ? { type: 'apply_edits', doc: documentHandle, edits: [...edits] }
          : { type: 'apply_edits', doc: documentHandle, label, edits: [...edits] }
      return (await call(command)).result.revision
    },
    async renameParameter(documentHandle, parameter, name) {
      const { result } = await call({
        type: 'rename_parameter',
        doc: documentHandle,
        parameter,
        name,
      })
      return result.revision
    },
    async beginTransaction(documentHandle, label) {
      await call({ type: 'begin_transaction', doc: documentHandle, label })
    },
    async setFocus(documentHandle, feature) {
      await call(
        feature === null
          ? { type: 'set_focus', doc: documentHandle }
          : { type: 'set_focus', doc: documentHandle, feature },
      )
    },
    async commitTransaction(documentHandle) {
      const { result } = await call({ type: 'commit_transaction', doc: documentHandle })
      return { revision: result.revision, recorded: result.recorded }
    },
    async cancelTransaction(documentHandle) {
      return (await call({ type: 'cancel_transaction', doc: documentHandle })).result.revision
    },
    async undo(documentHandle) {
      const { result } = await call({ type: 'undo', doc: documentHandle })
      return { revision: result.revision, label: result.label }
    },
    async redo(documentHandle) {
      const { result } = await call({ type: 'redo', doc: documentHandle })
      return { revision: result.revision, label: result.label }
    },
    solveSketch(documentHandle, feature, candidate) {
      return unlessSuperseded(
        solved(
          candidate === undefined
            ? { type: 'solve_sketch', doc: documentHandle, feature }
            : { type: 'solve_sketch', doc: documentHandle, feature, candidate },
        ),
      )
    },
    dragSketch(documentHandle, feature, point, to) {
      return unlessSuperseded(
        solved({ type: 'drag_sketch', doc: documentHandle, feature, point, to: [to[0], to[1]] }),
      )
    },
    async endSketchDrag(documentHandle, feature, commit) {
      const { result } = await call({
        type: 'end_sketch_drag',
        doc: documentHandle,
        feature,
        commit,
      })
      return result.revision
    },
    async evaluate(documentHandle, expression) {
      const { result } = await call({ type: 'evaluate', doc: documentHandle, expr: expression })
      return { value: result.value, dimension: result.dimension }
    },
    async listElements(documentHandle, body, kind) {
      const command: CommandOfType<'list_elements'> =
        kind === undefined
          ? { type: 'list_elements', doc: documentHandle, body }
          : { type: 'list_elements', doc: documentHandle, body, kind }
      return (await call(command)).result.elements
    },
    async pickReference(documentHandle, pick) {
      return (await call({ type: 'pick_reference', doc: documentHandle, pick })).result.element
    },
    async resolveReferences(documentHandle, references) {
      const { result } = await call({
        type: 'resolve_references',
        doc: documentHandle,
        references: [...references],
      })
      return result.results
    },
    analyzePrint(documentHandle, options) {
      const command: CommandOfType<'analyze_print'> =
        options.profile === undefined
          ? { type: 'analyze_print', doc: documentHandle, orientation: options.orientation }
          : {
              type: 'analyze_print',
              doc: documentHandle,
              profile: options.profile,
              orientation: options.orientation,
            }
      return unlessSuperseded(call(command).then(({ result }) => result.health))
    },
    async exportBodies(documentHandle, format, bodies) {
      const command: CommandOfType<'export'> =
        bodies === undefined
          ? { type: 'export', doc: documentHandle, format }
          : { type: 'export', doc: documentHandle, format, bodies: [...bodies] }
      const { result, buffers } = await call(command)
      return {
        // Copied so the files outlive the response frame.
        files: result.files.map((file) => ({
          name: file.name,
          mediaType: file.media_type,
          bytes: byteBuffer(buffers, file.data).slice(),
        })),
        warnings: result.warnings,
      }
    },
  }
}
