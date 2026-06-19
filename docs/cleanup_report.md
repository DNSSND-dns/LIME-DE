# Backend cleanup report

## Removed

- The wgpu renderer attempt, its WGSL blit shader, `wgpu`, and `pollster`.
- The duplicate `src/native2.rs` implementation and `native2` naming in runtime
  code.
- Old `tty-udev`, experimental backend aliases, fake native render gating, and
  placeholder native frame-submit branches.
- The obsolete native backend fork document.

## Current architecture

- `src/backend/native/` is the primary backend and is split into session, udev,
  DRM, GBM, EGL, GLES, input, output, and event-loop modules.
- `src/backend/dev_winit/` is the temporary nested preview backend.
- `src/core/` owns shared compositor, shell, layout, window, scene, animation,
  and application state.

The supported backend names are `native` and `dev-winit`. The `--native2` and
`--tty` CLI aliases remain temporarily and warn before selecting `native`.

## Temporary dependencies

Winit and Softbuffer remain because `dev-winit` still provides a convenient
nested preview while native KMS output/pageflip support is being completed.
Dev preview uses Softbuffer only; it has no wgpu or Vulkan path.

## Render target

The primary native render path is DRM/KMS → GBM → EGL → GLES 3.x → Smithay
`GlesRenderer` through `smithay/renderer_gl`.

Intel Core i3-1315U and i7-13620H are target test systems, both using their Mesa
Intel iGPU. The code does not branch on CPU model; DRM/EGL/Mesa capabilities are
queried at runtime.
