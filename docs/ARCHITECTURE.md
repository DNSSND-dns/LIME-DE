# LIME DE Architecture

LIME DE is currently a single Rust binary that runs a minimal Wayland compositor runtime. The Winit backend is the development output inside an existing desktop session, while the TTY/udev backend is the native-session path for display-manager or real-TTY launch. Smithay provides the Wayland protocol machinery.

## Runtime

Entry flow:

```text
main.rs
  -> run()
  -> App::new()
  -> App::initialize()
  -> App::run()
  -> App::shutdown()
```

`main.rs` owns process-level startup and error reporting. `app.rs` owns application lifecycle and passes config into the compositor. `error.rs` provides the shared `AppError` type.

Config loading order:

1. `--config <path>`
2. `LIME_CONFIG`
3. `config/lime.toml`
4. `$XDG_CONFIG_HOME/lime-de/lime.toml` or `~/.config/lime-de/lime.toml`
5. `/etc/lime-de/lime.toml`

## Compositor

`src/core/compositor.rs` is the current core. It owns:

- Smithay `Display`
- calloop `EventLoop`
- Wayland socket
- Smithay compositor, xdg-shell, shm, seat, data-device, and output states
- internal windows and z-order
- scene graph
- render backend
- Winit test backend bridge
- frame loop and redraw scheduling
- test client process

Current Wayland protocols/globals:

- `wl_compositor`
- `wl_shm`
- `xdg_wm_base`
- `wl_seat`
- `wl_output`
- `wl_data_device_manager`

Current client flow:

```text
Wayland client
  -> wl_surface.commit
  -> BufferAssignment::NewBuffer
  -> SHM read if supported
  -> Window.client_pixels
  -> RenderSceneFrame
  -> WinitBackend
  -> Softbuffer framebuffer
```

## Scene

`src/core/scene.rs` stores an internal tree of scene nodes:

- `Output`
- `Window`
- `Surface`

The scene graph currently tracks structure and logs node creation/removal. Rendering still mostly uses tracked windows and z-order directly. This is acceptable for the prototype, but the renderer should eventually consume scene nodes as the primary source of draw order.

## Window

`src/core/window.rs` defines:

- `WindowId`
- `WindowGeometry`
- `ClientBufferMetadata`
- `ClientBufferPixels`
- `Window`

Tracked windows live in `CompositorState` as `TrackedWindow`, which additionally stores the Smithay `ToplevelSurface`, scene node id, and pending frame callbacks.

Current window behavior:

- create on xdg toplevel
- update title/app_id
- map/unmap on buffer attach/remove
- destroy cleanup
- store latest readable SHM pixels
- maintain z-order
- focus on click
- visual drag
- visual resize

## Render

`src/render.rs` defines CPU render payloads:

- `RenderColor`
- `RenderRect`
- `RenderImage`
- `RenderSceneFrame`
- `RenderBackend`

`RenderBackend` is still a skeleton logger. The real pixel writes happen in `WinitBackend` using Softbuffer. This split is useful, but eventually `RenderBackend` should own more of the actual draw planning.

## Backend

`src/backend/dev_winit/` contains the temporary nested preview backend.
`src/backend/native/` contains the native session, udev, DRM/KMS, GBM, EGL, GLES,
input, output, and event-loop layers.

The Winit backend runs Winit on its own thread, sends events back to the compositor through an `mpsc` channel, and receives `RenderSceneFrame` through an `EventLoopProxy`.

Backend responsibilities:

- create `LIME DE Test Backend`
- allocate Softbuffer framebuffer
- clear background
- draw client images
- draw placeholder rectangles
- draw cursor above windows
- report frame presented
- report mouse movement/buttons
- report keyboard events

The TTY/udev backend initializes a libseat session, discovers DRM devices, opens the primary GPU, creates GBM/EGL/GLES state, and logs connectors. It is the native session backend, but it still needs real output/pageflip plumbing before it can present the compositor frame directly on screen.

System session packaging lives in `packaging/`:

- `lime-de-session`: wrapper used by display managers
- `lime.desktop`: Wayland session entry installed to `/usr/share/wayland-sessions`

## Input

`src/input.rs` currently stores `CursorState`.

Input path:

```text
Winit WindowEvent
  -> WinitBackendOutputEvent
  -> CompositorState::handle_backend_event()
  -> cursor/focus/drag/resize
  -> Smithay pointer/keyboard handles
```

Current input support:

- cursor position
- cursor clamping
- pointer hit testing
- hover enter/leave logs
- left click focus
- visual drag
- visual resize
- Smithay pointer enter/motion/button
- Smithay keyboard focus and key delivery

Keyboard mapping is intentionally minimal and maps common Winit physical keys to Linux input keycodes.

## Output

`src/output.rs` defines LIME's internal output model:

- `OutputId`
- `Output`
- virtual default: `LIME-Virtual-1`, `1280x720`, scale `1.0`, refresh `60000`

`CompositorState` also creates Smithay `Output` objects and advertises `wl_output` globals. These are protocol-visible outputs for clients, not DRM/KMS outputs.

## Dependency Graph

```text
main
  -> app
     -> config
     -> state
     -> compositor
        -> backend
           -> render
        -> input
        -> output
        -> render
        -> scene
           -> output
           -> window
        -> window
        -> error

Smithay
  -> compositor
  -> seat/input
  -> xdg-shell
  -> shm
  -> output
  -> data-device

Winit + Softbuffer
  -> backend
```

## Audit

### Dead Code

- `BackendState`, `InputState`, `OutputState`, `RenderState`, and `WindowState` are placeholders and currently unused.
- `Scene::window_geometries()` is only touched to keep the scene graph present in render flow. Rendering does not meaningfully consume the scene yet.
- `RenderBackend` logs frame operations but does not draw pixels directly.

### Duplicate Logic

- Window geometry exists in both `Window` and `SceneNode`. These can drift because interactive move/resize updates `Window` geometry, not scene node geometry.
- Output exists as both internal `crate::output::Output` and Smithay `smithay::output::Output`. This is intentional for now, but needs naming discipline.
- Draw order is represented by `z_order`, while scene hierarchy also implies order.

### Temporary Hacks

- SHM copy uses a small `unsafe` block because Smithay exposes shared memory as a raw pointer. The copy is immediate and does not store borrowed memory.
- `CompositorState::output_count()` reads some fields only to keep ownership explicit and avoid unused-field warnings.
- Keyboard mapping covers common keys only.
- Resize is visual only and does not send proper xdg configure resize requests.
- Real rendering is in `WinitBackend`, while `RenderBackend` is still a logger.
- The test client launcher is convenient but should become a dev-only mode.

### Future Bottlenecks

- `src/compositor.rs` is too large and owns too many responsibilities.
- CPU copying full SHM buffers every commit will become expensive.
- `RenderSceneFrame` clones client image buffers every frame.
- Frame loop still renders on interval even when idle.
- Scene graph is not yet the source of truth for rendering.
- No damage tracking.
- No proper xdg configure/ack_configure lifecycle for move/resize.
- No subsurface tree rendering.
- No popup positioning/rendering.
- No robust keyboard layout/input method support.

## Stabilization Targets

Before adding shell features, split or clarify:

- `wayland_runtime.rs`: display, socket, event loop, protocol states
- `client_buffer.rs`: SHM read/copy/format conversion
- `window_manager.rs`: z-order, focus, move, resize
- `input_router.rs`: Winit input to Smithay pointer/keyboard
- `frame_scheduler.rs`: dirty state, frame callbacks, timestamps
