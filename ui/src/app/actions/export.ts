import type { ExportedFile } from '../../core/api'
import { FileServiceError, type FileKind } from '../../files/file-service'
import type { Diagnostic, ExportFormat } from '../../generated/protocol'
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

/** The bodies to export in document order: visible bodies with model geometry, or the selected bodies. */
function bodiesToExport(app: App, scope: ExportRequest['scope']): readonly string[] {
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
    .filter((body) => (scope === 'selected' ? selected.has(body.id) : body.visible !== false))
    .filter((body) => document.meshes[body.id] !== undefined)
    .map((body) => body.id)
}

function showWarnings(app: App, warnings: readonly Diagnostic[]): void {
  for (const warning of warnings) {
    notify(app, { severity: 'warning', message: warning.message, actions: [] })
  }
}

function showWriteError(app: App, fileName: string, error: FileServiceError): void {
  const message =
    error.reason === 'permission'
      ? MESSAGES.save_permission(fileName)
      : error.reason === 'not_found'
        ? MESSAGES.save_not_found(fileName)
        : MESSAGES.save_io(fileName, error.message)
  notify(app, { severity: 'error', message, actions: [] })
}

/** Exports the bodies the request names and saves the file where the user chooses. */
export async function exportModel(app: App, request: ExportRequest): Promise<void> {
  const document = documentOf(app)
  const bodies = bodiesToExport(app, request.scope)
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
  const [file] = exported.files
  if (file !== undefined) {
    const kind: FileKind = request.format
    const target = await app.files.chooseSaveTarget(`${document.name}.${request.format}`, kind)
    if (target === null) {
      return
    }
    try {
      await app.files.write(target, kind, file.bytes)
    } catch (error) {
      if (error instanceof FileServiceError) {
        showWriteError(app, target.name, error)
        return
      }
      throw error
    }
    notify(app, { severity: 'success', message: MESSAGES.exported(target.name), actions: [] })
  }
  showWarnings(app, exported.warnings)
}
