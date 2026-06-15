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
