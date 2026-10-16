// The type roles of ui.md 2.16.7 as jsdom sees them. jsdom applies no stylesheet, so an element has
// tabular figures when it or an ancestor carries `tabular-nums`, and is code when it or an ancestor
// carries `font-mono` (ui.md 6.4, U-202 to U-208).

/** The class editable code adds, so contextual alternates never reshape the text around the caret. */
export const NO_CONTEXTUAL_LIGATURES = '[font-variant-ligatures:no-contextual]'

/** Whether the element sets its digits in tabular figures. */
export function hasTabularFigures(element: Element): boolean {
  return element.closest('.tabular-nums') !== null
}

/** Whether the element is set in the code face. */
export function isCode(element: Element): boolean {
  return element.closest('.font-mono') !== null
}

/** Whether the element carries the ligature class of editable code itself. */
export function hasEditableCodeLigatures(element: Element): boolean {
  return element.classList.contains(NO_CONTEXTUAL_LIGATURES)
}

/** Every element under `root`, `root` included, that carries the class `name`. */
export function elementsWithClass(root: Element, name: string): Element[] {
  return [root, ...root.querySelectorAll('*')].filter((element) => element.classList.contains(name))
}
