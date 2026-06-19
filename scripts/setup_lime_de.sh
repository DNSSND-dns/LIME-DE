#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
ASSUME_YES=false
SKIP_TTY_CHECK=false
INSTALL_SESSION=false

for arg in "$@"; do
  case "$arg" in
    -y | --yes)
      ASSUME_YES=true
      ;;
    --skip-tty-check)
      SKIP_TTY_CHECK=true
      ;;
    --install-session)
      INSTALL_SESSION=true
      ;;
    -h | --help)
      printf 'Usage: %s [--yes] [--skip-tty-check] [--install-session]\n' "$0"
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
    libegl1-mesa-dev
    libgbm-dev
    libinput-dev
    libseat-dev
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
    libgbm-devel
    libinput-devel
    libseat-devel
    libwayland-client
    libxkbcommon-devel
    make
    pkgconf-pkg-config
    rsync
    mesa-libEGL-devel
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
    mesa
    libxkbcommon
    pkgconf
    rsync
    seatd
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
    printf 'For TTY backend also install libseat, libudev, libgbm and EGL development packages.\n'
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

  if [[ "$SKIP_TTY_CHECK" == false ]]; then
    cargo check --features native_tty
  else
    printf 'Skipping cargo check --features native_tty.\n'
  fi
}

install_lime_session() {
  if [[ "$INSTALL_SESSION" == false ]]; then
    return
  fi

  cd "$PROJECT_ROOT"

  if ! confirm 'Install LIME DE as a system Wayland session?'; then
    printf 'Skipping session installation.\n'
    return
  fi

  cargo build --release --features native_tty

  local bindir="/usr/local/bin"
  local sysconfdir="/etc/lime-de"
  local sessiondir="/usr/share/wayland-sessions"

  run_as_root install -Dm755 "$PROJECT_ROOT/target/release/lime-de" "$bindir/lime-de"
  run_as_root install -Dm755 "$PROJECT_ROOT/packaging/lime-de-session" "$bindir/lime-de-session"
  run_as_root install -Dm644 "$PROJECT_ROOT/config/lime-native.toml" "$sysconfdir/lime.toml"
  run_as_root install -Dm644 "$PROJECT_ROOT/packaging/lime.desktop" "$sessiondir/lime.desktop"

  printf '\nLIME DE session installed.\n'
  printf 'Binary: %s/lime-de\n' "$bindir"
  printf 'Session wrapper: %s/lime-de-session\n' "$bindir"
  printf 'Config: %s/lime.toml\n' "$sysconfdir"
  printf 'Display manager entry: %s/lime.desktop\n' "$sessiondir"
}

printf 'LIME DE setup\n'
printf 'Project: %s\n\n' "$PROJECT_ROOT"

install_system_packages
install_rust
verify_project
install_lime_session

printf '\nSetup complete.\n'
printf 'Run LIME DE with:\n'
printf '  cd %q\n' "$PROJECT_ROOT"
printf '  cargo run -- --backend dev-winit\n'
printf '\nTTY backend smoke test from a real TTY:\n'
printf '  cd %q\n' "$PROJECT_ROOT"
printf '  cargo run --features native_tty -- --backend native\n'
printf '\nInstall as a selectable native Wayland session:\n'
printf '  %q --yes --install-session\n' "$0"
printf '\nSwitch to a TTY with Ctrl+Alt+F3 and return with Ctrl+Alt+F2/F1.\n'
