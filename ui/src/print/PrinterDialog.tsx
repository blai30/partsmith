import { clsx } from 'clsx/lite'
import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react'
import { useApp, useAppState } from '../app/AppProvider'
import { deletePrinter, savePrinter, setPrinterDialog } from '../app/actions/printers'
import { reportFailure } from '../app/report'
import { Button } from '../components/Button'
import { SegmentedControl } from '../components/SegmentedControl'
import { TextField } from '../components/TextField'
import type { Bed, PrinterProfile, Technology } from '../generated/protocol'
import { MESSAGES } from '../messages'
import type { PrinterDialogRequest } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import {
  findPrinter,
  GENERIC_PRINTER_IDS,
  numberFieldProblem,
  printerForm,
  printerFromForm,
  printerIdFromName,
  shownNumberFields,
  type PrinterForm,
  type PrinterNumberField,
} from './printer'

// The printer dialog of ui.md 3.17: Add printer and Edit printer, and "Add printer '{id}'" of the
// unknown printer row.

/** Each number field's label and unit, in the dialog's order. */
const NUMBER_FIELDS: Readonly<Record<PrinterNumberField, { label: string; unit: string }>> = {
  width: { label: 'Width', unit: 'mm' },
  depth: { label: 'Depth', unit: 'mm' },
  height: { label: 'Height', unit: 'mm' },
  diameter: { label: 'Diameter', unit: 'mm' },
  nozzle: { label: 'Nozzle', unit: 'mm' },
  layerHeight: { label: 'Layer height', unit: 'mm' },
  overhang: { label: 'Steepest overhang', unit: '°' },
  bridge: { label: 'Longest bridge', unit: 'mm' },
  press: { label: 'Press fit', unit: 'mm' },
  snug: { label: 'Snug fit', unit: 'mm' },
  slip: { label: 'Slip fit', unit: 'mm' },
  loose: { label: 'Loose fit', unit: 'mm' },
}

const TECHNOLOGIES = [
  { value: 'fdm', label: 'FDM' },
  { value: 'resin', label: 'Resin' },
] as const satisfies readonly { value: Technology; label: string }[]

const BEDS = [
  { value: 'rectangle', label: 'Rectangle' },
  { value: 'circle', label: 'Round' },
] as const satisfies readonly { value: Bed['kind']; label: string }[]

/** Elements that take keyboard focus, for keeping Tab inside the dialog. */
const FOCUSABLE = 'button:not([disabled]), input:not([disabled])'

/** "My printer", or "My printer 2", "My printer 3" and so on while a listed profile has that name (3.17). */
function newPrinterName(printers: readonly PrinterProfile[]): string {
  const taken = new Set(printers.map((printer) => printer.name))
  let name = 'My printer'
  for (let number = 2; taken.has(name); number += 1) {
    name = `My printer ${String(number)}`
  }
  return name
}

/** What the dialog opens with for a request: the profile its values come from, its name and its fixed id. */
function opening(
  request: PrinterDialogRequest,
  printers: readonly PrinterProfile[],
): { profile: PrinterProfile; name: string; id: string | null } | null {
  if (request.kind === 'edit') {
    return { profile: request.profile, name: request.profile.name, id: request.profile.id }
  }
  const generic = findPrinter(printers, GENERIC_PRINTER_IDS.fdm)
  if (generic === undefined) {
    return null
  }
  return { profile: generic, name: request.id ?? newPrinterName(printers), id: request.id }
}

/** The printer dialog while one is requested; nothing otherwise. */
export function PrinterDialog() {
  const request = useAppState((state) => state.printerDialog)
  // A new request starts from its own values.
  return request === null ? null : <PrinterDialogPanel key={dialogKey(request)} request={request} />
}

function dialogKey(request: PrinterDialogRequest): string {
  return request.kind === 'edit' ? `edit ${request.profile.id}` : `new ${request.id ?? ''}`
}

function PrinterDialogPanel({ request }: { readonly request: PrinterDialogRequest }) {
  const app = useApp()
  const printers = useAppState((state) => state.printers)
  const [start] = useState(() => opening(request, printers))
  const [opened, setOpened] = useState(start?.profile ?? null)
  const [form, setForm] = useState(() =>
    start === null ? null : printerForm(start.profile, start.name),
  )
  const [error, setError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const panel = useRef<HTMLDivElement>(null)
  const titleId = useId()

  // Focus moves into the dialog, and back to where it was when the dialog closes.
  useEffect(() => {
    const element = panel.current
    const previous = element?.ownerDocument.activeElement
    element?.querySelector<HTMLElement>('[data-autofocus]')?.focus()
    return () => {
      if (previous instanceof HTMLElement && previous.isConnected) {
        previous.focus()
      }
    }
  }, [])

  if (start === null || opened === null || form === null) {
    return null
  }
  const fixedId = start.id
  const problems = shownNumberFields(form).flatMap((field) => {
    const problem = numberFieldProblem(form[field])
    return problem === null ? [] : [[field, MESSAGES[problem](NUMBER_FIELDS[field].label)] as const]
  })
  const fieldErrors = new Map<PrinterNumberField, string>(problems)

  function close(): void {
    setPrinterDialog(app, null)
  }

  function update(change: Partial<PrinterForm>): void {
    setForm((current) => (current === null ? current : { ...current, ...change }))
  }

  function switchTechnology(technology: Technology): void {
    const generic = findPrinter(printers, GENERIC_PRINTER_IDS[technology])
    if (generic === undefined || form === null) {
      return
    }
    // Every value but Name comes from the technology's generic profile.
    setOpened(generic)
    setForm(printerForm(generic, form.name))
  }

  async function save(): Promise<void> {
    if (form === null || opened === null) {
      return
    }
    const id =
      fixedId ??
      printerIdFromName(
        form.name,
        printers.map((printer) => printer.id),
      )
    setSaving(true)
    const refusal = await savePrinter(app, printerFromForm(form, opened, id))
    // A saved printer closes the dialog; a refused one keeps it open with the typed values.
    if (refusal !== null) {
      setError(refusal)
      setSaving(false)
    }
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    if (event.key === 'Escape') {
      // Default-prevented, so the Esc ladder skips it.
      event.preventDefault()
      close()
      return
    }
    if (event.key !== 'Tab' || panel.current === null) {
      return
    }
    // Tab and Shift+Tab stay inside the dialog.
    const focusable = [...panel.current.querySelectorAll<HTMLElement>(FOCUSABLE)]
    const first = focusable[0]
    const last = focusable.at(-1)
    const active = panel.current.ownerDocument.activeElement
    if (event.shiftKey && active === first) {
      event.preventDefault()
      last?.focus()
    } else if (!event.shiftKey && active === last) {
      event.preventDefault()
      first?.focus()
    }
  }

  return (
    <div className="pointer-events-none fixed inset-4 z-50 flex items-center justify-center">
      {/* The dialog handles Esc and keeps Tab inside it, as the WAI-ARIA modal dialog pattern asks. */}
      {/* oxlint-disable-next-line jsx-a11y/no-noninteractive-element-interactions */}
      <div
        ref={panel}
        // A native modal dialog would bring browser placement styles to a panel the layer above
        // positions, as for the command palette (3.4).
        // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        data-testid="printer-dialog"
        onKeyDown={onKeyDown}
        className={clsx(
          'pointer-events-auto flex max-h-full w-120 max-w-full flex-col gap-4 overflow-y-auto rounded-xl border p-4 text-sm shadow-lg transition-opacity duration-150 ease-out starting:opacity-0 motion-reduce:transition-none',
          COLOR_CLASSES.surfaceRaised.background,
          COLOR_CLASSES.forcedEdge.border,
          COLOR_CLASSES.elevation.shadow,
          COLOR_CLASSES.text.foreground,
        )}
      >
        <h2 id={titleId} className="text-base font-semibold">
          {request.kind === 'edit' ? 'Edit printer' : 'New printer'}
        </h2>
        <TextField
          label="Name"
          value={form.name}
          maxLength={100}
          data-autofocus
          onChange={(name) => {
            update({ name })
          }}
        />
        <div className="flex flex-wrap gap-4">
          <Choice
            label="Technology"
            options={TECHNOLOGIES}
            value={form.technology}
            onChange={switchTechnology}
          />
          <Choice
            label="Bed"
            options={BEDS}
            value={form.bed}
            onChange={(bed) => {
              update({ bed })
            }}
          />
        </div>
        <div className="grid grid-cols-3 gap-x-3 gap-y-2">
          {shownNumberFields(form).map((field) => (
            <NumberField
              key={field}
              label={NUMBER_FIELDS[field].label}
              unit={NUMBER_FIELDS[field].unit}
              value={form[field]}
              error={fieldErrors.get(field) ?? null}
              onChange={(text) => {
                update({ [field]: text })
              }}
            />
          ))}
        </div>
        {error === null ? null : (
          <p
            role="alert"
            data-testid="printer-dialog-error"
            className={COLOR_CLASSES.errorText.foreground}
          >
            {error}
          </p>
        )}
        <div className="flex items-center gap-2">
          {request.kind === 'edit' ? (
            <Button
              data-testid="printer-delete"
              onClick={() => {
                deletePrinter(app, request.profile.id).catch(reportFailure('Deleting the printer'))
              }}
            >
              Delete printer
            </Button>
          ) : null}
          <span className="flex-1" />
          <Button onClick={close}>Cancel</Button>
          <Button
            variant="primary"
            disabled={fieldErrors.size > 0 || saving}
            onClick={() => {
              save().catch(reportFailure('Saving the printer'))
            }}
          >
            Save
          </Button>
        </div>
      </div>
    </div>
  )
}

type ChoiceProps<Value extends string> = {
  readonly label: string
  readonly options: readonly { readonly value: Value; readonly label: string }[]
  readonly value: Value
  readonly onChange: (value: Value) => void
}

/** A labeled radio group of the dialog. */
function Choice<Value extends string>({ label, options, value, onChange }: ChoiceProps<Value>) {
  return (
    <div className="flex flex-col gap-1">
      {/* The group's accessible name repeats this visual label. */}
      <span
        aria-hidden="true"
        className={clsx('text-xs font-medium', COLOR_CLASSES.textMuted.foreground)}
      >
        {label}
      </span>
      <SegmentedControl label={label} options={options} value={value} onChange={onChange} />
    </div>
  )
}

type NumberFieldProps = {
  readonly label: string
  readonly unit: string
  readonly value: string
  readonly error: string | null
  readonly onChange: (text: string) => void
}

/** A plain decimal number field with its unit after it; the unit describes the field. */
function NumberField({ label, unit, value, error, onChange }: NumberFieldProps) {
  const unitId = useId()
  return (
    <div className="flex min-w-0 items-start gap-1.5">
      <TextField
        label={label}
        value={value}
        error={error}
        onChange={onChange}
        inputMode="decimal"
        aria-describedby={unitId}
        tabular
        className="flex-1"
      />
      {/* Level with the input: under the label's line and the gap above the input. */}
      <span
        id={unitId}
        className={clsx('mt-5 flex h-8 items-center text-xs', COLOR_CLASSES.textMuted.foreground)}
      >
        {unit}
      </span>
    </div>
  )
}
