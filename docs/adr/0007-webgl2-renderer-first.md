# 0007: WebGL2 renderer first, WebGPU behind the same interface

Status: Accepted, needs human review

## Context

The design calls for a custom thin renderer on WebGPU with a WebGL2 fallback. WebGL2 runs everywhere the app must run, including headless Chromium in CI for Playwright and screenshot tests, while WebGPU support is still uneven across Linux desktops and WebViews.

## Decision

Ship the WebGL2 backend first behind a `RenderBackend` interface that owns buffers, passes (shaded faces, edge lines, ID picking, section clipping, analysis colors) and readback. A WebGPU backend implementing the same interface follows once the WebGL2 backend meets the section 14 budgets.

## Consequences

One backend to make solid before doubling the surface. The interface keeps WebGPU a drop-in addition rather than a rewrite.
