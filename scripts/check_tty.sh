#!/usr/bin/env bash
set -euo pipefail

printf 'tty=%s\n' "$(tty || true)"
printf 'id=%s\n' "$(id)"
printf 'SUDO_USER=%s\n' "${SUDO_USER:-}"
printf 'SUDO_UID=%s\n' "${SUDO_UID:-}"
printf 'XDG_SESSION_TYPE=%s\n' "${XDG_SESSION_TYPE:-}"
printf 'XDG_SESSION_ID=%s\n' "${XDG_SESSION_ID:-}"
printf 'XDG_RUNTIME_DIR=%s\n' "${XDG_RUNTIME_DIR:-}"

if command -v fgconsole >/dev/null 2>&1; then
  printf 'fgconsole=%s\n' "$(fgconsole 2>/dev/null || true)"
fi

if command -v who >/dev/null 2>&1; then
  printf 'who am i=%s\n' "$(who am i 2>/dev/null || true)"
fi

if command -v ps >/dev/null 2>&1; then
  ps -p "$$" -o pid,ppid,tty,stat,comm,args || true
  ps -p "$PPID" -o pid,ppid,tty,stat,comm,args || true
fi
