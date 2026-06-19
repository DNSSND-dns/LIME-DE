#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
REAL_HOME="${SUDO_USER:+$(getent passwd "$SUDO_USER" | cut -d: -f6)}"
REAL_HOME="${REAL_HOME:-$HOME}"
LOG_DIR="${XDG_STATE_HOME:-$REAL_HOME/.local/state}/lime-de"
LOG_FILE="$LOG_DIR/tty.log"
BUILD_MODE="${LIME_BUILD_MODE:-release}"
LIME_TTY_BACKEND="${LIME_TTY_BACKEND:-native}"

mkdir -p "$LOG_DIR"
printf 'LIME DE TTY starting. Log: %s\n' "$LOG_FILE"
exec >>"$LOG_FILE" 2>&1

printf '\n=== LIME DE TTY run: %s ===\n' "$(date -Is)"
printf 'Project: %s\n' "$PROJECT_ROOT"
printf 'Log: %s\n' "$LOG_FILE"
printf 'TTY: %s\n' "$(tty || true)"
printf 'User: %s\n' "$(id)"
printf 'SUDO_USER=%s\n' "${SUDO_USER:-}"
printf 'SUDO_UID=%s\n' "${SUDO_UID:-}"
printf 'Shell PID: %s PPID: %s\n' "$$" "$PPID"
printf 'XDG_SESSION_TYPE=%s\n' "${XDG_SESSION_TYPE:-}"
printf 'XDG_SESSION_ID=%s\n' "${XDG_SESSION_ID:-}"
printf 'WAYLAND_DISPLAY=%s\n' "${WAYLAND_DISPLAY:-}"
printf 'DISPLAY=%s\n' "${DISPLAY:-}"
printf 'LIME_DRM_OUTPUT=%s\n' "${LIME_DRM_OUTPUT:-auto}"
printf 'LIME_TTY_BACKEND=%s\n' "$LIME_TTY_BACKEND"
CURRENT_TTY="$(tty || true)"

if command -v fgconsole >/dev/null 2>&1; then
  printf 'fgconsole=%s\n' "$(fgconsole 2>/dev/null || true)"
fi
if command -v who >/dev/null 2>&1; then
  printf 'who am i: %s\n' "$(who am i 2>/dev/null || true)"
fi
if command -v ps >/dev/null 2>&1; then
  ps -p "$$" -o pid,ppid,tty,stat,comm,args || true
  ps -p "$PPID" -o pid,ppid,tty,stat,comm,args || true
fi

if command -v loginctl >/dev/null 2>&1; then
  if [[ -n "${XDG_SESSION_ID:-}" ]]; then
    loginctl show-session "$XDG_SESSION_ID" -p Type -p Active -p State -p Remote -p Class -p Seat -p VTNr || true
  fi
  loginctl seat-status seat0 || true
fi

if command -v systemctl >/dev/null 2>&1; then
  systemctl is-active seatd 2>/dev/null || true
fi

if [[ "$CURRENT_TTY" == "not a tty" ]]; then
  printf 'Warning: this shell is not a real TTY. For DRM master, switch with Ctrl+Alt+F3 and run this script there.\n'
fi
if [[ "$CURRENT_TTY" == /dev/pts/* && "${LIME_ALLOW_PTS:-0}" != "1" ]]; then
  printf 'Error: %s is a pseudo-terminal, not a Linux VT. Switch to Ctrl+Alt+F3 and run this there.\n' "$CURRENT_TTY"
  exit 1
fi

if [[ -d /dev/dri ]]; then
  ls -l /dev/dri || true
else
  printf 'Warning: /dev/dri does not exist.\n'
fi

cd "$PROJECT_ROOT"

run_cargo() {
  if [[ "$(id -u)" == "0" && -n "${SUDO_USER:-}" ]]; then
    runuser -u "$SUDO_USER" -- env HOME="$REAL_HOME" cargo "$@"
  else
    cargo "$@"
  fi
}

if [[ "${LIME_SKIP_BUILD:-0}" == "1" ]]; then
  if [[ "$BUILD_MODE" == "debug" ]]; then
    BINARY="$PROJECT_ROOT/target/debug/lime-de"
  else
    BINARY="$PROJECT_ROOT/target/release/lime-de"
  fi
elif [[ "$BUILD_MODE" == "debug" ]]; then
  run_cargo build --features native_tty
  BINARY="$PROJECT_ROOT/target/debug/lime-de"
else
  run_cargo build --release --features native_tty
  BINARY="$PROJECT_ROOT/target/release/lime-de"
fi

export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"
export LIME_CONFIG="${LIME_CONFIG:-$PROJECT_ROOT/config/lime-native.toml}"
if [[ -z "${XDG_RUNTIME_DIR:-}" && "$(id -u)" == "0" && -n "${SUDO_UID:-}" ]]; then
  USER_RUNTIME_DIR="/run/user/$SUDO_UID"
  if [[ -d "$USER_RUNTIME_DIR" ]]; then
    export XDG_RUNTIME_DIR="$USER_RUNTIME_DIR"
  fi
fi
if [[ -z "${XDG_RUNTIME_DIR:-}" && "$(id -u)" == "0" ]]; then
  export XDG_RUNTIME_DIR=/run/lime-de
  mkdir -p "$XDG_RUNTIME_DIR"
  chmod 700 "$XDG_RUNTIME_DIR"
fi
if [[ -z "${LIBSEAT_BACKEND:-}" ]]; then
  if [[ "$(id -u)" == "0" ]]; then
    export LIBSEAT_BACKEND=builtin
  else
    export LIBSEAT_BACKEND=logind
  fi
else
  export LIBSEAT_BACKEND
fi
export XDG_CURRENT_DESKTOP=LIME
export XDG_SESSION_DESKTOP=lime

printf 'Binary: %s\n' "$BINARY"
printf 'Config: %s\n' "$LIME_CONFIG"
printf 'LIBSEAT_BACKEND=%s\n' "$LIBSEAT_BACKEND"
printf 'LIME_TTY_BACKEND=%s\n' "$LIME_TTY_BACKEND"
printf 'Runtime XDG_SESSION_TYPE=%s\n' "${XDG_SESSION_TYPE:-}"
printf 'Runtime XDG_RUNTIME_DIR=%s\n' "${XDG_RUNTIME_DIR:-}"
printf 'Starting native TTY backend...\n'

exec "$BINARY" --backend "$LIME_TTY_BACKEND" --no-test-client "$@"
