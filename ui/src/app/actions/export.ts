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
      request.scope === 'selected' ? selected.has(body.id) : exportsWhenVisible(body, request.format),
    )
    .filter((body) => document.meshes[body.id] !== undefined)
    .map((body) => body.id)
}

function showWarnings(app: App, warnings: readonly Diagnostic[]): void {
  for (const warning of warnings) {
    notify(app, { severity: 'warning', message: warning.message, actions: [] })
  }
}

function showSaveError(app: App, fileName: string, error: FileServiceError): void {
  const message =
    error.reason === 'permission'
      ? MESSAGES.save_permission(fileName)
      : error.reason === 'not_found'
        ? MESSAGES.save_not_found(fileName)
        : MESSAGES.save_io(fileName, error.message)
  notify(app, { severity: 'error', message, actions: [] })
}

/**
 * Asks where to save the file and writes it; resolves the saved file's name, or null when the
 * picker was canceled or saving failed (the failure is shown).
 */
async function saveFile(
  app: App,
  suggestedName: string,
  kind: FileKind,
  file: ExportedFile,
): Promise<string | null> {
  let target: SaveTarget | null = null
  try {
    target = await app.files.chooseSaveTarget(suggestedName, kind)
    if (target === null) {
      return null
    }
    await app.files.write(target, kind, file.bytes)
    return target.name
  } catch (error) {
    if (error instanceof FileServiceError) {
      showSaveError(app, target?.name ?? suggestedName, error)
      return null
    }
    throw error
  }
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
  const saved = await saveFile(app, `${document.name}.${request.format}`, request.format, file)
  if (saved === null) {
    return
  }
  notify(app, { severity: 'success', message: MESSAGES.exported(saved), actions: [] })
  showWarnings(app, exported.warnings)
}
