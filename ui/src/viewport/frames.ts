// Animation frames for code that draws outside React (ADR 0002).

/** Animation frames, or a timer where the environment has none. */
export function requestFrame(callback: (time: number) => void): () => void {
  if (typeof requestAnimationFrame === 'function') {
    const id = requestAnimationFrame(callback)
    return () => {
      cancelAnimationFrame(id)
    }
  }
  const timer = setTimeout(() => {
    callback(performance.now())
  }, 16)
  return () => {
    clearTimeout(timer)
  }
}
