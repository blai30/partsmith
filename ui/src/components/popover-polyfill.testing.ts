// Test support: jsdom 30 has no Popover API, and its user-agent stylesheet hides every [popover]
// element because :popover-open never matches there. This stands in for the browser so component
// tests can open popovers, see them in the accessibility tree and focus their content.

const OPEN_ATTRIBUTE = 'data-test-popover-open'
const STYLE_RULE = `[popover][${OPEN_ATTRIBUTE}] { display: block !important; }`

type PopoverMethods = {
  showPopover(options?: { source?: HTMLElement }): void
  hidePopover(): void
  togglePopover(options?: boolean | { force?: boolean; source?: HTMLElement }): boolean
}

/** Open popovers, oldest first. */
const openStack: HTMLElement[] = []

/** The element that had focus when a popover opened, where focus returns when it closes. */
const previouslyFocused = new WeakMap<HTMLElement, HTMLElement>()

function kindOf(element: HTMLElement): string {
  const value = element.getAttribute('popover')
  return value === '' || value === null ? 'auto' : value
}

// Dispatches a ToggleEvent stand-in; returns false when a listener canceled it.
function fireToggle(
  element: HTMLElement,
  type: 'beforetoggle' | 'toggle',
  opening: boolean,
): boolean {
  const event = new Event(type, { cancelable: type === 'beforetoggle' && opening })
  Object.defineProperties(event, {
    oldState: { value: opening ? 'closed' : 'open' },
    newState: { value: opening ? 'open' : 'closed' },
  })
  return element.dispatchEvent(event)
}

/** Whether the element is an open popover. */
export function isPopoverOpen(element: Element): boolean {
  return element.hasAttribute(OPEN_ATTRIBUTE)
}

// Like the HTML hide popover algorithm: focus returns to the element focused at opening when
// focus is inside the popover and the close is not a light dismiss by pointer.
function hide(element: HTMLElement, focusPrevious: boolean): void {
  if (!isPopoverOpen(element)) {
    return
  }
  // Nested popovers close with their ancestor, innermost first.
  for (const nested of openStack.toReversed()) {
    if (nested !== element && element.contains(nested)) {
      hide(nested, false)
    }
  }
  fireToggle(element, 'beforetoggle', false)
  const focusInside = document.activeElement !== null && element.contains(document.activeElement)
  element.removeAttribute(OPEN_ATTRIBUTE)
  openStack.splice(openStack.indexOf(element), 1)
  const previous = previouslyFocused.get(element)
  previouslyFocused.delete(element)
  if (focusPrevious && focusInside && previous?.isConnected === true) {
    previous.focus()
  }
  fireToggle(element, 'toggle', false)
}

function show(element: HTMLElement, source: HTMLElement | undefined): void {
  if (!element.hasAttribute('popover')) {
    throw new DOMException(
      'Not supported on elements that do not have a valid popover attribute.',
      'NotSupportedError',
    )
  }
  if (!element.isConnected) {
    throw new DOMException('Invalid on disconnected popover elements.', 'InvalidStateError')
  }
  if (isPopoverOpen(element)) {
    return
  }
  const kind = kindOf(element)
  if (kind !== 'manual') {
    // Showing a popover light-dismisses the others of its kind that are not its ancestors.
    for (const other of openStack.toReversed()) {
      const nests = other.contains(element) || (source !== undefined && other.contains(source))
      if (kindOf(other) === kind && !nests) {
        hide(other, false)
      }
    }
  }
  if (!fireToggle(element, 'beforetoggle', true)) {
    return
  }
  const restoresFocus = kind !== 'manual' && topmostDismissible() === undefined
  element.setAttribute(OPEN_ATTRIBUTE, '')
  openStack.push(element)
  if (restoresFocus && document.activeElement instanceof HTMLElement) {
    previouslyFocused.set(element, document.activeElement)
  }
  fireToggle(element, 'toggle', true)
}

function topmostDismissible(): HTMLElement | undefined {
  return openStack.toReversed().find((element) => kindOf(element) !== 'manual')
}

function onKeyDown(event: KeyboardEvent): void {
  if (event.key !== 'Escape' || event.defaultPrevented) {
    return
  }
  const topmost = topmostDismissible()
  if (topmost !== undefined) {
    hide(topmost, true)
  }
}

function onPointerDown(event: PointerEvent): void {
  const target = event.target
  for (const element of openStack.toReversed()) {
    const inside = target instanceof Node && element.contains(target)
    if (kindOf(element) !== 'manual' && !inside) {
      hide(element, false)
    }
  }
}

/**
 * Installs showPopover, hidePopover and togglePopover with light dismiss (Esc and a pointer press
 * outside), the beforetoggle and toggle events, and a style that shows open popovers.
 * Returns a function that removes all of it.
 */
export function installPopoverPolyfill(): () => void {
  const prototype: Partial<PopoverMethods> = HTMLElement.prototype
  const installed = prototype.showPopover === undefined
  if (installed) {
    const methods: PopoverMethods = {
      showPopover(this: HTMLElement, options?: { source?: HTMLElement }) {
        show(this, options?.source)
      },
      hidePopover(this: HTMLElement) {
        hide(this, true)
      },
      togglePopover(
        this: HTMLElement,
        options?: boolean | { force?: boolean; source?: HTMLElement },
      ) {
        const force = typeof options === 'boolean' ? options : options?.force
        const source = typeof options === 'object' ? options.source : undefined
        const open = force ?? !isPopoverOpen(this)
        if (open) {
          show(this, source)
        } else {
          hide(this, true)
        }
        return isPopoverOpen(this)
      },
    }
    Object.assign(HTMLElement.prototype, methods)
  }
  const styleElement = document.createElement('style')
  styleElement.textContent = STYLE_RULE
  document.head.append(styleElement)
  document.addEventListener('keydown', onKeyDown)
  document.addEventListener('pointerdown', onPointerDown, true)
  return () => {
    document.removeEventListener('keydown', onKeyDown)
    document.removeEventListener('pointerdown', onPointerDown, true)
    styleElement.remove()
    for (const element of openStack.splice(0)) {
      element.removeAttribute(OPEN_ATTRIBUTE)
    }
    if (installed) {
      for (const name of ['showPopover', 'hidePopover', 'togglePopover']) {
        Reflect.deleteProperty(HTMLElement.prototype, name)
      }
    }
  }
}
