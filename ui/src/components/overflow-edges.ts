import { useEffect, type RefObject } from 'react'

/** How far, in CSS pixels, a body may fall short of its end and still count as scrolled to it. */
const END_TOLERANCE = 1

/** Sets `data-overflow-top` and `data-overflow-bottom` on a body from its scroll position. */
function markEdges(body: HTMLElement): void {
  body.toggleAttribute('data-overflow-top', body.scrollTop > 0)
  body.toggleAttribute(
    'data-overflow-bottom',
    body.scrollHeight - (body.scrollTop + body.clientHeight) > END_TOLERANCE,
  )
}

/**
 * Marks a scrolling body `data-overflow-top` while content is scrolled under a fixed header and
 * `data-overflow-bottom` while content continues under a fixed footer, so `divider`'s overflow
 * facets draw its rule only then (ui.md 3.2). It sets the attributes on the element itself, never
 * through React state, so scrolling renders nothing, and measures again on scroll, when the body
 * or one of its children resizes, and when its children change.
 */
export function useOverflowEdges(ref: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const body = ref.current
    if (body === null) {
      return undefined
    }
    const measure = () => {
      markEdges(body)
    }
    const resizes = typeof ResizeObserver === 'function' ? new ResizeObserver(measure) : null
    resizes?.observe(body)
    for (const child of body.children) {
      resizes?.observe(child)
    }
    const mutations = new MutationObserver((records) => {
      for (const record of records) {
        for (const node of record.removedNodes) {
          if (node instanceof Element) {
            resizes?.unobserve(node)
          }
        }
        for (const node of record.addedNodes) {
          if (node instanceof Element) {
            resizes?.observe(node)
          }
        }
      }
      measure()
    })
    mutations.observe(body, { childList: true })
    body.addEventListener('scroll', measure, { passive: true })
    measure()
    return () => {
      body.removeEventListener('scroll', measure)
      mutations.disconnect()
      resizes?.disconnect()
    }
  }, [ref])
}
