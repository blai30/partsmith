import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname } from 'node:path'
import { fixturePath } from '../helpers'

// Helpers of E-160 (ui.md 6.5, delivery.md 2.8, ADR 0048): the documents the UI writes, their bless
// and the comparison of a download with the committed document in fixtures/gui/.

/** A JSON value as `JSON.parse` returns it. */
export type Json = null | boolean | number | string | Json[] | JsonObject

/** A JSON object. */
export type JsonObject = { [key: string]: Json }

/** The first difference between a download and its committed document, in document order. */
export type Difference = {
  /** The JSON Pointer of the difference, empty at the root. */
  readonly pointer: string
  /** What the download holds there, as the message writes it. */
  readonly actual: string
  /** What the committed document holds there, as the message writes it. */
  readonly expected: string
}

/** The bless command of delivery.md section 4. */
const BLESS_COMMAND = 'PARTSMITH_BLESS=1 pnpm -C ui e2e round-trip.spec.ts'

/** The failure of a bless requested in CI (delivery.md section 4). */
export const BLESS_IN_CI =
  'PARTSMITH_BLESS is set in CI. Bless expected results locally and commit them.'

/** An id the UI generates (model.md): a parameter, body, feature or sketch entity prefix and 12 hex digits. */
const GENERATED_ID = /^([pbfe])-[0-9a-f]{12}$/

/** Object keys whose values hold ids and name faces in id order, so they differ from run to run. */
const REFERENCE_MEMBERS = new Set(['q', 'fp', 'nb', 'merged'])

/** How far apart two numbers may be and still match. */
const NUMBER_TOLERANCE = 1e-9

/** The path of a document the UI writes, as delivery.md section 4 names it. */
export function shownPath(file: string): string {
  return `ui/e2e/fixtures/gui/${file}`
}

/** The failure of a missing committed document (delivery.md section 4). */
export function missingMessage(file: string): string {
  return `${shownPath(file)} is missing. Run: ${BLESS_COMMAND}`
}

/** The failure of a committed document that differs from the download (delivery.md section 4). */
export function differenceMessage(file: string, difference: Difference): string {
  const where = difference.pointer === '' ? 'the document' : difference.pointer
  return (
    `${shownPath(file)} is not what the UI writes now: ${where} is ${difference.actual} in the ` +
    `download and ${difference.expected} in the committed file. Run: ${BLESS_COMMAND}`
  )
}

/**
 * Whether this run blesses the documents: `PARTSMITH_BLESS` is `1`. Throws the bless-in-CI message
 * when `PARTSMITH_BLESS` is set while `CI` is set too.
 */
export function blessMode(environment: Readonly<Record<string, string | undefined>>): boolean {
  if (environment['PARTSMITH_BLESS'] !== undefined && environment['CI'] !== undefined) {
    throw new Error(BLESS_IN_CI)
  }
  return environment['PARTSMITH_BLESS'] === '1'
}

/** Writes a download as the committed document `file` and announces it. */
export async function blessDocument(file: string, bytes: Buffer): Promise<void> {
  const path = fixturePath(`gui/${file}`)
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, bytes)
  console.log(`blessed ${shownPath(file)}`)
}

/** The JSON of the committed document `file`, or null when there is none. */
export async function readCommittedDocument(file: string): Promise<Json | null> {
  let text: string
  try {
    text = await readFile(fixturePath(`gui/${file}`), 'utf8')
  } catch (error) {
    if (error instanceof Error && 'code' in error && error.code === 'ENOENT') {
      return null
    }
    throw error
  }
  return JSON.parse(text) as Json
}

/** The JSON of the committed document `file`; throws the missing-document message when there is none. */
export async function committedDocument(file: string): Promise<Json> {
  const committed = await readCommittedDocument(file)
  if (committed === null) {
    throw new Error(missingMessage(file))
  }
  return committed
}

/**
 * Whether a bless writes the download over the committed document: only when that document is
 * missing (null) or the comparison finds a difference, as the examples' bless writes only files that
 * are missing or differ (delivery.md 2.8), so a document that still matches keeps its ids.
 */
export function blessWrites(downloaded: Json, committed: Json | null): boolean {
  return committed === null || firstDifference(downloaded, committed) !== null
}

/** Whether a JSON value is an object (not an array or null). */
function isObject(value: Json): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** A key as a JSON Pointer token (RFC 6901). */
function pointerToken(key: string): string {
  return key.replaceAll('~', '~0').replaceAll('/', '~1')
}

/** Whether two leaf values match: generated ids of one prefix, numbers within the tolerance, or equal values. */
function leavesMatch(actual: Json, expected: Json): boolean {
  if (typeof actual === 'string' && typeof expected === 'string') {
    const actualId = GENERATED_ID.exec(actual)
    const expectedId = GENERATED_ID.exec(expected)
    if (actualId !== null && expectedId !== null) {
      return actualId[1] === expectedId[1]
    }
  }
  if (typeof actual === 'number' && typeof expected === 'number') {
    return Math.abs(actual - expected) <= NUMBER_TOLERANCE
  }
  return actual === expected
}

/**
 * The first difference between the download's JSON and the committed document's, walked in document
 * order (keys in the download's order, items in array order), or null when they match (ui.md 6.5
 * E-160): objects need the same keys in the same order, arrays the same length; values of the
 * reference members `q`, `fp`, `nb` and `merged` are not compared.
 */
export function firstDifference(actual: Json, expected: Json, pointer = ''): Difference | null {
  if (isObject(actual) && isObject(expected)) {
    const actualKeys = Object.keys(actual)
    const expectedKeys = Object.keys(expected)
    if (JSON.stringify(actualKeys) !== JSON.stringify(expectedKeys)) {
      return {
        pointer,
        actual: `the keys ${JSON.stringify(actualKeys)}`,
        expected: `the keys ${JSON.stringify(expectedKeys)}`,
      }
    }
    for (const key of actualKeys) {
      if (REFERENCE_MEMBERS.has(key)) {
        continue
      }
      const difference = firstDifference(
        actual[key] ?? null,
        expected[key] ?? null,
        `${pointer}/${pointerToken(key)}`,
      )
      if (difference !== null) {
        return difference
      }
    }
    return null
  }
  if (Array.isArray(actual) && Array.isArray(expected)) {
    if (actual.length !== expected.length) {
      return {
        pointer,
        actual: `${String(actual.length)} items`,
        expected: `${String(expected.length)} items`,
      }
    }
    for (const [index, item] of actual.entries()) {
      const difference = firstDifference(
        item,
        expected[index] ?? null,
        `${pointer}/${String(index)}`,
      )
      if (difference !== null) {
        return difference
      }
    }
    return null
  }
  if (leavesMatch(actual, expected)) {
    return null
  }
  return { pointer, actual: JSON.stringify(actual), expected: JSON.stringify(expected) }
}

/** The object at `value`, or a failure naming `what`. */
export function asObject(value: Json | undefined, what: string): JsonObject {
  if (value === undefined || !isObject(value)) {
    throw new Error(`${what} is not a JSON object: ${JSON.stringify(value)}`)
  }
  return value
}

/** The objects of the array at `value`, or a failure naming `what`. */
export function asObjects(value: Json | undefined, what: string): JsonObject[] {
  if (!Array.isArray(value)) {
    throw new Error(`${what} is not a JSON array: ${JSON.stringify(value)}`)
  }
  return value.map((item, index) => asObject(item, `${what}/${String(index)}`))
}
