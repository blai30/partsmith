// Temporary experiment page script (removed in the next commit). It asks a new canvas for a
// context as ui/src/main.tsx does and keeps the statusMessage of the webglcontextcreationerror
// event, which the WebGL specification has a browser fire when it refuses to create a context.

/** The context a new canvas gives for `kind`, or the reason the browser gave for none. */
function askForContext(kind) {
  const canvas = document.createElement('canvas')
  let reason = null
  canvas.addEventListener(
    'webglcontextcreationerror',
    (event) => {
      reason = event.statusMessage
    },
    { once: true },
  )
  const context = canvas.getContext(kind)
  if (context === null) {
    return { created: false, reason }
  }
  // RENDERER is "Mozilla" in Firefox; the extension names the driver, such as llvmpipe.
  const debugInfo = context.getExtension('WEBGL_debug_renderer_info')
  return {
    created: true,
    renderer: context.getParameter(
      debugInfo === null ? context.RENDERER : debugInfo.UNMASKED_RENDERER_WEBGL,
    ),
    version: context.getParameter(context.VERSION),
  }
}

const webgl2 = askForContext('webgl2')
// WebGL 1 is asked only when WebGL2 is refused: Firefox's reasons for the two tell its causes apart.
globalThis.probeResult = { webgl2, webgl: webgl2.created ? null : askForContext('webgl') }
