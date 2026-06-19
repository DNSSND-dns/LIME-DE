#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

if [[ ! -f /etc/arch-release ]]; then
  printf 'Warning: Arch Linux was not detected.\n' >&2
fi

if systemctl is-active --quiet seatd.service 2>/dev/null; then
  export LIBSEAT_BACKEND="${LIBSEAT_BACKEND:-seatd}"
else
  export LIBSEAT_BACKEND="${LIBSEAT_BACKEND:-logind}"
  printf 'seatd is inactive; using %s backend.\n' "$LIBSEAT_BACKEND"
fi

export LIME_CONFIG="${LIME_CONFIG:-$PROJECT_ROOT/config/lime-native.toml}"

exec "$PROJECT_ROOT/scripts/run_tty.sh" "$@"
