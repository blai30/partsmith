import type { CoreApi, ExportedFile } from '../../core/api'
import { exportFailedText } from '../../export/status'
import { FileServiceError, type FileKind, type SaveTarget } from '../../files/file-service'
import type { Body, Diagnostic, ExportFormat, Orientation } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import type { App } from '../createApp'
import { documentOf, showCoreError } from './edits'
import { showSaveError } from './files'
import { notify } from './notifications'
import { showProblemsAction } from './panels'
import { showOrientationError } from './print'

// Export to 3MF, STL, OBJ and STEP (ui.md 3.18).

export type ExportRequest = {
  readonly format: '3mf' | 'stl' | 'obj' | 'step'
  readonly scope: 'visible' | 'selected'
}

/** Formats that go to a printer, so they are exported in the Lay on face orientation (3.17). */
const ORIENTED_FORMATS: ReadonlySet<ExportRequest['format']> = new Set(['3mf', 'stl'])

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
    notify(app, { severity: 'warning', message: warning.message, actions: [], code: warning.code })
  }
}

/** An exported file on its way to disk, with what the export reported about it. */
type ExportResult = {
  readonly suggestedName: string
  readonly kind: FileKind
  readonly file: ExportedFile
  readonly warnings: readonly Diagnostic[]
}

/**
 * Asks where to save an export file. A canceled picker resolves null; a picker that fails is shown
 * with "Save as", which runs `saveAs`, and resolves null.
 */
async function chooseTarget(
  app: App,
  suggestedName: string,
  kind: FileKind,
  saveAs: () => Promise<void>,
): Promise<SaveTarget | null> {
  try {
    return await app.files.chooseSaveTarget(suggestedName, kind)
  } catch (error) {
    if (error instanceof FileServiceError) {
      showSaveError(app, suggestedName, error, saveAs)
      return null
    }
    throw error
  }
}

/**
 * Writes an exported file to its target, then announces it with the export's warnings. A failed
 * write is shown with "Save as", which asks again where to save the same file.
 */
async function writeExport(app: App, target: SaveTarget, exported: ExportResult): Promise<void> {
  try {
    await app.files.write(target, exported.kind, exported.file.bytes)
  } catch (error) {
    if (error instanceof FileServiceError) {
      showSaveError(app, target.name, error, () => saveExport(app, exported))
      return
    }
    throw error
  }
  // The file leaves out features in error, so the announcement says which (3.18).
  const failed = exportFailedText(app.store.getState())
  if (failed === null) {
    notify(app, {
      severity: 'success',
      message: MESSAGES.exported(target.name),
      actions: [],
      code: 'exported',
    })
  } else {
    notify(app, {
      severity: 'warning',
      message: MESSAGES.exported_with_failures(target.name, failed),
      code: 'exported_with_failures',
      actions: [showProblemsAction(app)],
    })
  }
  showWarnings(app, exported.warnings)
}

/** "Save as" after a failed save: asks again where to save the file already exported, and writes it. */
async function saveExport(app: App, exported: ExportResult): Promise<void> {
  const target = await chooseTarget(app, exported.suggestedName, exported.kind, () =>
    saveExport(app, exported),
  )
  if (target !== null) {
    await writeExport(app, target, exported)
  }
}

/**
 * Exports the bodies the request names and saves the file where the user chooses. The save dialog
 * opens first, while the click that asked for the export still lets the page open it; a slow
 * export then cannot lose that permission (3.18). A canceled dialog exports nothing.
 */
export async function exportModel(app: App, request: ExportRequest): Promise<void> {
  const document = documentOf(app)
  const bodies = bodiesToExport(app, request)
  if (document === null || bodies.length === 0) {
    notify(app, {
      severity: 'error',
      message: MESSAGES.nothing_to_export(),
      actions: [],
      code: 'nothing_to_export',
    })
    return
  }
  const suggestedName = `${document.name}.${request.format}`
  const target = await chooseTarget(app, suggestedName, request.format, () =>
    exportModel(app, request),
  )
  if (target === null) {
    return
  }
  // 3MF and STL are placed on the bed as Lay on face set; OBJ and STEP stay as modeled (3.18).
  const sessionOrientation = app.store.getState().print.orientation
  const orientation: Orientation | null = ORIENTED_FORMATS.has(request.format)
    ? sessionOrientation
    : null
  let exported: Awaited<ReturnType<CoreApi['exportBodies']>>
  try {
    // Through the mutation queue, so the export sees every edit sent before it.
    exported = await app.mutations.run(
      () =>
        orientation === null
          ? app.core.exportBodies(document.documentHandle, FORMATS[request.format], bodies)
          : app.core.exportBodies(
              document.documentHandle,
              FORMATS[request.format],
              bodies,
              orientation,
            ),
      'other',
    )
  } catch (error) {
    if (!showOrientationError(app, error, orientation)) {
      showCoreError(app, error, {
        stopped: { code: 'export_stopped', message: MESSAGES.export_stopped(target.name) },
      })
    }
    return
  }
  // STL with per_body false, 3MF, OBJ and STEP come back as one file (engine.md 3.13).
  const [file] = exported.files
  if (file === undefined) {
    return
  }
  await writeExport(app, target, {
    suggestedName,
    kind: request.format,
    file,
    warnings: exported.warnings,
  })
}
