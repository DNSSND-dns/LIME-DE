# Native backend plan

## Backend roles

- `native` is the primary backend.
- `dev-winit` is a temporary nested preview backend using Winit and Softbuffer.
- `core` owns compositor, shell, layout, window, scene, and animation state shared
  by both backends.

CLI:

```text
lime-de --backend native
lime-de --backend dev-winit
```

The deprecated `--native2` and `--tty` aliases select `native` and print a
warning.

## Target renderer

```text
DRM/KMS
↓
GBM
↓
EGL
↓
GLES 3.x
↓
Smithay GlesRenderer
```

The native backend uses Smithay `renderer_gl`. Vulkan, wgpu, and a custom
renderer are outside the current plan.

## Hardware target

Primary target: Intel Mesa iGPU, tested on:

- Intel Core i3-1315U
- Intel Core i7-13620H

There is no CPU-model-specific branch. Device selection, formats, modifiers,
GLES version, extensions, and scanout capabilities must be discovered at
runtime through DRM, EGL, and Mesa.

## Native module boundaries

- `session.rs` — libseat ownership and pause/resume.
- `udev.rs` — DRM device discovery and hotplug.
- `drm.rs` — DRM/KMS device, connector, CRTC, pageflip, and vblank state.
- `gbm.rs` — GBM device and buffer allocation.
- `egl.rs` — EGL display and context over GBM.
- `gles.rs` — Smithay `GlesRenderer` and renderer capability checks.
- `input.rs` — libinput and input event translation.
- `output.rs` — KMS connector to compositor output lifecycle.
- `event_loop.rs` — calloop source registration and frame scheduling.

The next native implementation work is real KMS output creation and pageflip
submission. Placeholder frame submission and opt-in fake render paths are not
part of this architecture.
