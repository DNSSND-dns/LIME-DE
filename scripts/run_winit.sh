#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
LOG_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/lime-de"
LOG_FILE="$LOG_DIR/dev-winit.log"

mkdir -p "$LOG_DIR"
exec > >(tee -a "$LOG_FILE") 2>&1

printf '\n=== LIME DE winit run: %s ===\n' "$(date -Is)"
printf 'Renderer: softbuffer\n'
printf 'Log: %s\n' "$LOG_FILE"

cd "$PROJECT_ROOT"
export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"
exec cargo run --features dev_winit -- --backend dev-winit "$@"
