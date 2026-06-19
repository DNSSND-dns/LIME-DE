# LIME DE

LIME DE is a custom Linux desktop environment built around a minimal Wayland compositor.

The project does not modify the Linux kernel, GPU drivers, Mesa, libdrm, libinput, or systemd.

The first real milestone is intentionally small:

1. Start.
2. Open a Wayland display.
3. Show a black screen.

Window management, dock, panel, launcher, settings, and visual design will be built after the compositor foundation exists.

## Build Dependencies

On Debian/Ubuntu-based systems:

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libxkbcommon-dev
```

Smithay links against `libxkbcommon`, so the runtime package alone is not enough; the development package is required for `cargo check` and `cargo build`.

## Run Modes

Development window inside the current desktop session:

```bash
./scripts/run_winit.sh --no-test-client
```

The `dev-winit` preview backend uses Softbuffer.

Logs are written to `~/.local/state/lime-de/`.

Native TTY/udev backend smoke test from a real Linux TTY:

```bash
./scripts/run_tty.sh
```

The native backend must be started from a seat-managed TTY or display manager session so it can become DRM master.

## Install as a Selectable Session

To make LIME DE appear next to other desktop environments in a display manager:

```bash
./scripts/setup_lime_de.sh --yes --install-session
```

This installs:

- `/usr/local/bin/lime-de`
- `/usr/local/bin/lime-de-session`
- `/etc/lime-de/lime.toml`
- `/usr/share/wayland-sessions/lime.desktop`

After logging out, choose `LIME DE` in the display manager session picker.
