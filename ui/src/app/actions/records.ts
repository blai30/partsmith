// Copies of document records with one optional property changed (ui.md 2.9 record edits).

/** The record with one optional property set, or removed when `value` is undefined. */
export function withOptional<Value extends object, Key extends keyof Value>(
  record: Value,
  key: Key,
  value: Value[Key] | undefined,
): Value {
  const copy: Value = { ...record }
  if (value === undefined) {
    Reflect.deleteProperty(copy, key)
  } else {
    copy[key] = value
  }
  return copy
}
