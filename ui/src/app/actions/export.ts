import type { ExportedFile } from '../../core/api'
import { FileServiceError, type FileKind, type SaveTarget } from '../../files/file-service'
import type { Body, Diagnostic, ExportFormat } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import type { App } from '../createApp'
import { documentOf, showCoreError } from './edits'
import { notify } from './notifications'

// Export to 3MF, STL, OBJ and STEP (ui.md 3.18).

export type ExportRequest = {
  readonly format: '3mf' | 'stl' | 'obj' | 'step'
  readonly scope: 'visible' | 'selected'
}

const FORMATS: Readonly<Record<ExportRequest['format'], ExportFormat>> = {
  '3mf': { kind: '3mf' },
  stl: { kind: 'stl', binary: true, per_body: false },
  obj: { kind: 'obj' },
  step: { kind: 'step' },
}

/**
 * Whether "All visible bodies" exports a body: a visible body, and for formats other than 3MF only
 * a part body, as the core's own default (engine.md 3.13), so a modifier never ends up as solid
 * geometry in an STL, OBJ or STEP file.
 */
function exportsWhenVisible(body: Body, format: ExportRequest['format']): boolean {
  return body.visible !== false && (format === '3mf' || (body.role ?? 'part') === 'part')
}

/** The bodies to export in document order: visible bodies with model geometry, or the selected bodies with geometry. */
function bodiesToExport(app: App, request: ExportRequest): readonly string[] {
  const state = app.store.getState()
  const document = state.document
  if (document === null) {
    return []
  }
  const selected = new Set(
    state.selection.flatMap((item) =>
      item.kind === 'body' && item.body !== null ? [item.body] : [],
    ),
  )
  return (document.recipe.bodies ?? [])
    .filter((body) =>
      request.scope === 'selected'
        ? selected.has(body.id)
        : exportsWhenVisible(body, request.format),
    )
    .filter((body) => document.meshes[body.id] !== undefined)
    .map((body) => body.id)
}

function showWarnings(app: App, warnings: readonly Diagnostic[]): void {
  for (const warning of warnings) {
    notify(app, { severity: 'warning', message: warning.message, actions: [] })
  }
}

/** An exported file on its way to disk, with what the export reported about it. */
type ExportResult = {
  readonly suggestedName: string
  readonly kind: FileKind
  readonly file: ExportedFile
  readonly warnings: readonly Diagnostic[]
}

/** A failed save of an exported file (4.3), with "Save as", which asks again where to save the same file. */
function showSaveError(
  app: App,
  fileName: string,
  error: FileServiceError,
  exported: ExportResult,
): void {
  const message =
    error.reason === 'permission'
      ? MESSAGES.save_permission(fileName)
      : error.reason === 'not_found'
        ? MESSAGES.save_not_found(fileName)
        : MESSAGES.save_io(fileName, error.message)
  notify(app, {
    severity: 'error',
    message,
    actions: [
      {
        label: 'Save as',
        run: () => {
          void saveExport(app, exported)
        },
      },
    ],
  })
}

/**
 * Asks where to save the exported file and writes it, then announces it with the export's
 * warnings. A canceled picker does nothing; a failed save is shown with "Save as".
 */
async function saveExport(app: App, exported: ExportResult): Promise<void> {
  let target: SaveTarget | null = null
  try {
    target = await app.files.chooseSaveTarget(exported.suggestedName, exported.kind)
    if (target === null) {
      return
    }
    await app.files.write(target, exported.kind, exported.file.bytes)
  } catch (error) {
    if (error instanceof FileServiceError) {
      showSaveError(app, target?.name ?? exported.suggestedName, error, exported)
      return
    }
    throw error
  }
  notify(app, { severity: 'success', message: MESSAGES.exported(target.name), actions: [] })
  showWarnings(app, exported.warnings)
}

/** Exports the bodies the request names and saves the file where the user chooses. */
export async function exportModel(app: App, request: ExportRequest): Promise<void> {
  const document = documentOf(app)
  const bodies = bodiesToExport(app, request)
  if (document === null || bodies.length === 0) {
    notify(app, { severity: 'error', message: MESSAGES.nothing_to_export(), actions: [] })
    return
  }
  let exported: { files: readonly ExportedFile[]; warnings: readonly Diagnostic[] }
  try {
    // Through the mutation queue, so the export sees every edit sent before it.
    exported = await app.mutations.run(() =>
      app.core.exportBodies(document.documentHandle, FORMATS[request.format], bodies),
    )
  } catch (error) {
    showCoreError(app, error)
    return
  }
  // STL with per_body false, 3MF, OBJ and STEP come back as one file (engine.md 3.13).
  const [file] = exported.files
  if (file === undefined) {
    return
  }
  await saveExport(app, {
    suggestedName: `${document.name}.${request.format}`,
    kind: request.format,
    file,
    warnings: exported.warnings,
  })
}
