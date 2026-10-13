// Reporting an action that failed after the user started it. The app already showed what it could,
// so the failure goes to the console for reports.

/** A rejection handler that logs "{action} failed." with the error. */
export function reportFailure(action: string): (error: unknown) => void {
  return (error) => {
    console.error(`${action} failed.`, error)
  }
}
