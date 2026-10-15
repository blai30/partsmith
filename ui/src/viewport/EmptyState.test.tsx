import { act, cleanup, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { documentState, readyState } from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import { DEFAULT_SETTINGS } from '../settings/settings'
import type { AppState } from '../state/store'
import { MAC_WEB } from '../test/fakes/app'
import { catalogOf, TEST_CATALOG_ENTRIES } from '../test/fakes/catalog'
import { CUBE_DOCUMENT, emptyDocument } from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import { EmptyState } from './EmptyState'
import { navigationLegend } from './navigation'

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

function renderEmpty(state: Partial<AppState> = {}, recipe = emptyDocument()) {
  const test = createModelApp({ state: readyState(documentState(recipe), state) })
  renderWithApp(test.app, <EmptyState />)
  return { ...test, user: userEvent.setup() }
}

describe('the empty-document first step (3.2)', () => {
  it('U-95 shows the "First step" group in a document without features', () => {
    renderEmpty()
    const chip = screen.getByRole('group', { name: 'First step' })
    expect(chip).toBe(screen.getByTestId('empty-state'))
    expect(chip).toHaveTextContent('Start with a sketch on a plane, or add a box.')
    expect(within(chip).getByRole('button', { name: 'Sketch' })).toBeInTheDocument()
    expect(within(chip).getByRole('button', { name: 'Box' })).toBeInTheDocument()
  })

  it('names the camera gestures of the preset and the palette shortcut', () => {
    renderEmpty()
    expect(screen.getByTestId('empty-state')).toHaveTextContent(
      `${navigationLegend('fusion')} Search every command: Ctrl+K.`,
    )
    expect(screen.getByTestId('empty-state')).toHaveTextContent('right-drag or drag the view cube')
  })

  it('keeps each gesture on one line, never broken at its hyphen', () => {
    renderEmpty()
    const legend = screen.getByText(/^Orbit:/)
    const kept = Array.from(
      legend.querySelectorAll('span.whitespace-nowrap'),
      (span) => span.textContent,
    )
    expect(kept).toEqual(['Shift+middle-drag,', 'right-drag', 'middle-drag', 'Shift+right-drag.'])
  })

  it('follows the navigation setting and the platform', () => {
    const test = createModelApp({
      state: readyState(documentState(emptyDocument()), {
        settings: { ...DEFAULT_SETTINGS, theme: 'light', navigation: 'freecad' },
      }),
      platform: MAC_WEB,
    })
    renderWithApp(test.app, <EmptyState />)
    expect(screen.getByTestId('empty-state')).toHaveTextContent(
      `${navigationLegend('freecad')} Search every command: ⌘K.`,
    )
  })

  it('leaves Box out when the catalog has no box', () => {
    renderEmpty({
      catalog: catalogOf(TEST_CATALOG_ENTRIES.filter((entry) => entry.type !== 'box')),
    })
    const chip = screen.getByTestId('empty-state')
    expect(within(chip).queryByRole('button', { name: 'Box' })).toBeNull()
    expect(within(chip).getByRole('button', { name: 'Sketch' })).toBeInTheDocument()
  })

  it('starts plane picking for a new sketch from "Sketch", and steps aside', async () => {
    const { app, user } = renderEmpty()
    await user.click(screen.getByRole('button', { name: 'Sketch' }))
    await settle()
    expect(app.store.getState().context).toBe('sketch')
    expect(app.store.getState().sketch?.sketchId).toBeNull()
    expect(screen.queryByTestId('empty-state')).toBeNull()
  })

  it('starts the Box command from "Box", and steps aside', async () => {
    const { app, core, user } = renderEmpty()
    await user.click(screen.getByRole('button', { name: 'Box' }))
    await settle()
    expect(core.callsTo('beginTransaction')).toEqual([[1, 'Box']])
    expect(app.store.getState().command?.featureType).toBe('box')
    expect(screen.queryByTestId('empty-state')).toBeNull()
  })

  it('is not shown in a document with features', () => {
    renderEmpty({}, CUBE_DOCUMENT)
    expect(screen.queryByTestId('empty-state')).toBeNull()
  })

  it('returns whenever the document has no features again', () => {
    const { app } = renderEmpty({}, CUBE_DOCUMENT)
    act(() => {
      app.store.setState({ document: documentState(emptyDocument()) })
    })
    expect(screen.getByTestId('empty-state')).toBeInTheDocument()
  })

  it('is not shown while the core loads', () => {
    renderEmpty({
      core: {
        status: 'loading',
        engineVersion: null,
        fatal: null,
        pendingMutations: 0,
        stillWorking: false,
      },
    })
    expect(screen.queryByTestId('empty-state')).toBeNull()
  })
})
