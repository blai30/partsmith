import { createDesktopFileService, tauriGlobalOf } from './desktop-file-service'
import { createWebFileService, type FileService } from './file-service'

/** The desktop file service when the Tauri global is present, else the web file service (mirrors chooseTransport). */
export function chooseFileService(environment: Window & typeof globalThis): FileService {
  const tauri = tauriGlobalOf(environment)
  return tauri === null ? createWebFileService(environment) : createDesktopFileService(tauri)
}
