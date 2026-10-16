import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach } from 'vitest'

// jsdom's window is 1024 CSS pixels wide, narrower than the 65rem from which the browser and the
// side panel dock (ui.md 3.2), so every rendered app would start with the browser closed. Tests of
// narrow windows stub `innerWidth` themselves.
Object.defineProperty(window, 'innerWidth', { configurable: true, writable: true, value: 1280 })

// jsdom lays nothing out and has no scrollIntoView, which the inspector calls to bring a whole
// field into view (ui.md 3.7); this stands in for it, and tests spy on it.
Object.defineProperty(Element.prototype, 'scrollIntoView', {
  configurable: true,
  writable: true,
  value: () => undefined,
})

// Unmounts what each test rendered with Testing Library. It runs after the test file's own
// afterEach hooks, so a file whose teardown must follow the unmount calls `cleanup()` itself first.
afterEach(cleanup)
