#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
ASSUME_YES=false

for arg in "$@"; do
  case "$arg" in
    -y | --yes)
      ASSUME_YES=true
      ;;
    -h | --help)
      printf 'Usage: %s [--yes]\n' "$0"
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$arg" >&2
      exit 1
      ;;
  esac
done

confirm() {
  local prompt="$1"

  if [[ "$ASSUME_YES" == true ]]; then
    return 0
  fi

  read -r -p "$prompt [y/N]: " answer
  [[ "$answer" =~ ^[Yy]$ ]]
}

run_as_root() {
  if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
    "$@"
  else
    sudo "$@"
  fi
}

install_packages_apt() {
  local base_packages=(
    build-essential
    ca-certificates
    curl
    libinput-dev
    libudev-dev
    libwayland-dev
    libxkbcommon-dev
    pkg-config
    rsync
    wayland-protocols
  )

  run_as_root apt-get update
  run_as_root apt-get install -y "${base_packages[@]}"
  run_as_root apt-get install -y foot || printf 'Optional package not installed: foot\n'
}

install_packages_dnf() {
  local base_packages=(
    ca-certificates
    curl
    gcc
    gcc-c++
    libinput-devel
    libwayland-client
    libxkbcommon-devel
    make
    pkgconf-pkg-config
    rsync
    systemd-devel
    wayland-devel
    wayland-protocols-devel
  )

  run_as_root dnf install -y "${base_packages[@]}"
  run_as_root dnf install -y foot || printf 'Optional package not installed: foot\n'
}

install_packages_pacman() {
  local base_packages=(
    base-devel
    ca-certificates
    curl
    libinput
    libxkbcommon
    pkgconf
    rsync
    systemd
    wayland
    wayland-protocols
  )

  run_as_root pacman -Sy --needed --noconfirm "${base_packages[@]}"
  run_as_root pacman -S --needed --noconfirm foot || printf 'Optional package not installed: foot\n'
}

install_system_packages() {
  if ! confirm 'Install/update system packages for LIME DE?'; then
    printf 'Skipping system package installation.\n'
    return
  fi

  if command -v apt-get >/dev/null 2>&1; then
    install_packages_apt
  elif command -v dnf >/dev/null 2>&1; then
    install_packages_dnf
  elif command -v pacman >/dev/null 2>&1; then
    install_packages_pacman
  else
    printf 'No supported package manager detected. Install build tools, xkbcommon, Wayland and libinput development packages manually.\n'
  fi
}

install_rust() {
  if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
  fi

  if command -v cargo >/dev/null 2>&1; then
    printf 'Rust/Cargo found: %s\n' "$(cargo --version)"
    return
  fi

  if ! confirm 'Rust was not found. Install Rust with rustup?'; then
    printf 'Rust installation skipped. cargo check cannot run.\n' >&2
    exit 1
  fi

  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"

  if ! command -v cargo >/dev/null 2>&1; then
    printf 'Cargo is still unavailable after rustup installation.\n' >&2
    exit 1
  fi
}

verify_project() {
  cd "$PROJECT_ROOT"

  if [[ ! -f Cargo.toml ]]; then
    printf 'Cargo.toml not found in %s\n' "$PROJECT_ROOT" >&2
    exit 1
  fi

  if [[ ! -f config/lime.toml ]]; then
    printf 'config/lime.toml not found in %s\n' "$PROJECT_ROOT" >&2
    exit 1
  fi

  cargo check
}

printf 'LIME DE setup\n'
printf 'Project: %s\n\n' "$PROJECT_ROOT"

install_system_packages
install_rust
verify_project

printf '\nSetup complete.\n'
printf 'Run LIME DE with:\n'
printf '  cd %q\n' "$PROJECT_ROOT"
printf '  cargo run\n'
