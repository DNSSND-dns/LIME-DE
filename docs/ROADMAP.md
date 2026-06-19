# LIME DE Roadmap

This roadmap favors compositor correctness before desktop shell features.

## Phase 1: Core Compositor

Status: in progress.

Goals:

- Start from terminal.
- Create Wayland display and socket.
- Advertise required protocol globals.
- Track xdg-shell windows.
- Track map/unmap/destroy.
- Support `wl_shm`.
- Advertise a virtual `wl_output`.
- Read SHM client buffers.
- Render real client pixels in the Winit test backend.
- Send frame callbacks.

Current gaps:

- Subsurface rendering.
- Popup rendering and positioning.
- Damage tracking.
- Proper xdg configure lifecycle.
- Cleaner module split for protocol state and buffer handling.
- Native TTY backend still initializes DRM/GBM/EGL only; it does not present frames yet.
- The temporary `dev-winit` backend renders through Softbuffer only.

Next tasks:

- Add TTY/DRM output mode selection and pageflip rendering.
- A2: real client buffer rendering polish.
- Add damage-aware redraw.
- Avoid cloning full client images every frame.
- Move SHM buffer reading out of `compositor.rs`.

## Phase 2: Interactive Windows

Status: prototype started.

Goals:

- Cursor model.
- Pointer motion.
- Pointer enter/leave/motion/button delivery.
- Window hit testing.
- Active window tracking.
- Z-order.
- Focus transitions.
- Drag windows.
- Resize windows.
- Keyboard focus.
- Keyboard delivery.

Current gaps:

- Visual resize does not send xdg configure resize.
- Keyboard mapping is minimal.
- Pointer focus and active focus are basic.
- No real focus protocol polish for activation/deactivation states.
- No pointer constraints or relative pointer handling.

Next tasks:

- A3: seat + pointer cleanup.
- A4: keyboard cleanup.
- A5: real focus protocol.
- Implement xdg configure for resize.
- Keep scene graph synchronized with window geometry.

## Phase 3: Desktop Shell

Status: not started.

Goals:

- Desktop background.
- Panel.
- Dock.
- Launcher.
- Workspace model.
- Basic app launching.
- Window switcher.
- Session commands.

Rules:

- Do not start this phase until core compositor and interactive windows are stable.
- Shell UI should use the same design primitives planned for LIME UI.
- Shell surfaces should be treated separately from normal app windows.

Likely modules:

- `shell/desktop.rs`
- `shell/panel.rs`
- `shell/dock.rs`
- `shell/launcher.rs`
- `shell/workspaces.rs`

## Phase 4: LIME UI

Status: not started.

Goals:

- Shared UI kit.
- Theme colors.
- Typography and spacing.
- Settings application.
- Shell component styling.
- Icons and wallpapers.

Rules:

- No blur, animations, or heavy visuals before compositor interaction is stable.
- Keep LIME UI independent from low-level compositor protocol code.

Likely modules:

- `lime_ui/theme`
- `lime_ui/widgets`
- `lime_ui/layout`
- `lime_settings`

## Immediate Stabilization Backlog

1. Done: move SHM buffer code into a dedicated module.
2. Move hit-test, z-order, focus, drag, and resize into a window manager module.
3. Make scene graph authoritative for render traversal.
4. Add damage tracking.
5. Replace interval rendering with stricter redraw scheduling.
6. Send xdg configure events for real resize.
7. Add popup support.
8. Improve keyboard mapping and layout handling.
9. Add a small manual test checklist.
10. Keep Winit backend as test backend until DRM/KMS is intentionally started.
