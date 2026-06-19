#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
TARGET_USER="${SUDO_USER:-}"
TARGET_TTY=tty5

while [[ $# -gt 0 ]]; do
  case "$1" in
    --user)
      TARGET_USER="${2:-}"
      shift 2
      ;;
    --tty)
      TARGET_TTY="${2:-}"
      shift 2
      ;;
    -h | --help)
      printf 'Usage: sudo %s --user USER [--tty tty5]\n' "$0"
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$1" >&2
      exit 1
      ;;
  esac
done

if [[ "$(id -u)" -ne 0 ]]; then
  printf 'Run with sudo.\n' >&2
  exit 1
fi
if [[ ! -f /etc/arch-release ]]; then
  printf 'This service installer is intended for Arch Linux.\n' >&2
  exit 1
fi
if [[ -z "$TARGET_USER" ]] || ! id "$TARGET_USER" >/dev/null 2>&1; then
  printf 'A valid target user is required: --user USER\n' >&2
  exit 1
fi
if [[ ! "$TARGET_TTY" =~ ^tty[0-9]+$ ]]; then
  printf 'Invalid TTY name: %s\n' "$TARGET_TTY" >&2
  exit 1
fi

TARGET_HOME="$(getent passwd "$TARGET_USER" | cut -d: -f6)"
TARGET_UID="$(id -u "$TARGET_USER")"
TARGET_GID="$(id -g "$TARGET_USER")"
LOG_DIR="$TARGET_HOME/.local/state/lime-de"

runuser -u "$TARGET_USER" -- cargo build \
  --manifest-path "$PROJECT_ROOT/Cargo.toml" \
  --release \
  --features native_tty \
  --bins

install -Dm755 "$PROJECT_ROOT/target/release/lime-de" /usr/local/bin/lime-de
install -Dm755 "$PROJECT_ROOT/target/release/lime-files" /usr/local/bin/lime-files
install -Dm755 "$PROJECT_ROOT/packaging/lime-de-session" /usr/local/bin/lime-de-session
install -Dm644 "$PROJECT_ROOT/config/lime-native.toml" /etc/lime-de/lime.toml
install -Dm644 "$PROJECT_ROOT/packaging/lime.desktop" /usr/share/wayland-sessions/lime.desktop
install -d -o "$TARGET_UID" -g "$TARGET_GID" "$LOG_DIR"

if getent group seat >/dev/null 2>&1; then
  usermod -aG seat "$TARGET_USER"
fi

cat >/etc/systemd/system/lime-de-tty@.service <<EOF
[Unit]
Description=LIME DE native session on /dev/%i
After=seatd.service systemd-user-sessions.service
Requires=seatd.service
Conflicts=getty@%i.service

[Service]
Type=simple
User=$TARGET_USER
Environment=HOME=$TARGET_HOME
RuntimeDirectory=lime-de-$TARGET_UID
RuntimeDirectoryMode=0700
Environment=XDG_RUNTIME_DIR=/run/lime-de-$TARGET_UID
Environment=LIME_CONFIG=/etc/lime-de/lime.toml
Environment=LIBSEAT_BACKEND=seatd
Environment=XDG_CURRENT_DESKTOP=LIME
Environment=XDG_SESSION_DESKTOP=lime
Environment=XDG_SESSION_TYPE=wayland
ExecStart=/usr/local/bin/lime-de --backend native
StandardInput=tty
StandardOutput=append:$LOG_DIR/systemd-tty.log
StandardError=append:$LOG_DIR/systemd-tty.log
TTYPath=/dev/%i
TTYReset=yes
TTYVHangup=yes
TTYVTDisallocate=yes
Restart=no

[Install]
WantedBy=multi-user.target
EOF

systemctl enable --now seatd.service
systemctl daemon-reload

printf 'Installed LIME DE Arch TTY service.\n'
printf 'Start: sudo systemctl start lime-de-tty@%s.service\n' "$TARGET_TTY"
printf 'Stop:  sudo systemctl stop lime-de-tty@%s.service\n' "$TARGET_TTY"
printf 'Logs:  tail -f %s/systemd-tty.log\n' "$LOG_DIR"
