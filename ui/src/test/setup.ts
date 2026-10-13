import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach } from 'vitest'

// Unmounts what each test rendered with Testing Library. It runs after the test file's own
// afterEach hooks, so a file whose teardown must follow the unmount calls `cleanup()` itself first.
afterEach(cleanup)
