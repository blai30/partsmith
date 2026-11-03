import { MESSAGES } from '../../messages'
import type { App, Platform } from '../createApp'
import { notify } from './notifications'

// Third-party notices (ui.md 3.5, delivery.md 2.9 and open issue 11): the way from the app to the
// THIRD-PARTY-NOTICES.txt that every copy of Partsmith carries. It reads no file and calls no host
// command.

/** Where the desktop bundle of each operating system installs the notices (desktop.md 2.8). */
const NOTICES_PLACES: Readonly<Record<Platform['operatingSystem'], string>> = {
  mac: 'Partsmith.app/Contents/Resources/THIRD-PARTY-NOTICES.txt',
  linux: '/usr/lib/Partsmith/THIRD-PARTY-NOTICES.txt',
  windows:
    '%LOCALAPPDATA%\\Partsmith\\THIRD-PARTY-NOTICES.txt, or in THIRD-PARTY-NOTICES.txt next to partsmith-desktop.exe if Partsmith was installed in another folder',
  other: 'THIRD-PARTY-NOTICES.txt in the folder Partsmith is installed in',
}

/** Shows the third-party notices (3.5): the deployment's notices in a new tab on the web, the notification `notices_installed` on the desktop. */
export function showNotices(app: App): void {
  if (app.platform.host === 'web') {
    // Before any await: a browser opens a tab only from the click or key press itself.
    window.open(`${import.meta.env.BASE_URL}THIRD-PARTY-NOTICES.txt`, '_blank', 'noopener')
    return
  }
  notify(
    app,
    {
      severity: 'info',
      message: MESSAGES.notices_installed(NOTICES_PLACES[app.platform.operatingSystem]),
      actions: [],
      code: 'notices_installed',
    },
    { persistent: true },
  )
}
