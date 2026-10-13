import type { FileServiceError } from '../../files/file-service'
import { MESSAGES } from '../../messages'
import type { App } from '../createApp'
import { notify } from './notifications'

// Messages about document and export files (ui.md 3.19, 4.3).

/**
 * Shows a failed choice or write of a file with "Save as", which runs `saveAs` to try again
 * (3.19, 4.3).
 */
export function showSaveError(
  app: App,
  fileName: string,
  error: FileServiceError,
  saveAs: () => Promise<void>,
): void {
  const [code, message] =
    error.reason === 'permission'
      ? ['save_permission', MESSAGES.save_permission(fileName)]
      : error.reason === 'not_found'
        ? ['save_not_found', MESSAGES.save_not_found(fileName)]
        : ['save_io', MESSAGES.save_io(fileName, error.message)]
  notify(app, {
    severity: 'error',
    message,
    code,
    actions: [
      {
        label: 'Save as',
        run: () => {
          void saveAs()
        },
      },
    ],
  })
}
