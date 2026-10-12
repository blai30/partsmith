// Generates ui/src/generated/protocol.ts from docs/format/protocol.schema.json (ui.md 2.2).
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { compile } from 'json-schema-to-typescript'

const SCHEMA_PATH = 'docs/format/protocol.schema.json'
const schemaUrl = new URL('../../docs/format/protocol.schema.json', import.meta.url)
const outputUrl = new URL('../src/generated/protocol.ts', import.meta.url)

const OPTIONS = {
  bannerComment: `/* Generated from ${SCHEMA_PATH} by ui/scripts/generate-protocol-types.mjs. Do not edit. */`,
  unreachableDefinitions: true,
  additionalProperties: false,
  strictIndexSignatures: true,
  declareExternallyReferenced: true,
  enableConstEnums: false,
  format: false,
}

// Keywords that describe a schema without constraining it.
const ANNOTATIONS = new Set([
  '$comment',
  'default',
  'deprecated',
  'description',
  'examples',
  'readOnly',
  'title',
  'writeOnly',
])

function fail(message) {
  console.error(message)
  process.exit(1)
}

/**
 * Rewrites every `$ref` that has sibling keywords as an `allOf`. Draft 2020-12 applies a `$ref`
 * together with its siblings, which schemars emits for tagged envelopes (`{kind, ...Request}`) and
 * documented properties; json-schema-to-typescript instead merges the siblings into a copy of the
 * target, which renames `$defs` entries (`Request1`, `Document2`). The `allOf` form keeps the meaning
 * and lets every `$defs` entry keep its definition name.
 */
function normalize(node) {
  if (Array.isArray(node)) {
    return node.map(normalize)
  }
  if (typeof node !== 'object' || node === null) {
    return node
  }
  const result = Object.fromEntries(
    Object.entries(node).map(([key, value]) => [key, normalize(value)]),
  )
  if (typeof result.$ref !== 'string' || Object.keys(result).length === 1) {
    return result
  }
  const { $ref, ...siblings } = result
  const annotations = {}
  const constraints = {}
  for (const [key, value] of Object.entries(siblings)) {
    if (ANNOTATIONS.has(key)) {
      annotations[key] = value
    } else if (key !== 'unevaluatedProperties') {
      constraints[key] = value
    }
  }
  const parts = Object.keys(constraints).length === 0 ? [{ $ref }] : [{ $ref }, constraints]
  return { ...annotations, allOf: parts }
}

async function readSchema() {
  try {
    return JSON.parse(await readFile(schemaUrl, 'utf8'))
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error)
    return fail(`Cannot read ${SCHEMA_PATH}: ${reason}`)
  }
}

const schema = await readSchema()
const version = schema['x-protocol-version']
if (!Number.isInteger(version)) {
  fail(`${SCHEMA_PATH} has no integer "x-protocol-version".`)
}

const types = await compile(normalize(schema), 'Protocol', OPTIONS)
const text = `${types.replaceAll('\r\n', '\n').trimEnd()}\n\nexport const PROTOCOL_VERSION = ${String(version)} as const\n`

const outputPath = fileURLToPath(outputUrl)
await mkdir(dirname(outputPath), { recursive: true })
await writeFile(outputPath, text)
