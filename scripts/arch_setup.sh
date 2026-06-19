#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
TARGET_USER="${SUDO_USER:-$USER}"
ASSUME_YES=false
INSTALL_SESSION=false
ENABLE_SEATD=true

for arg in "$@"; do
  case "$arg" in
    -y | --yes)
      ASSUME_YES=true
      ;;
    --install-session)
      INSTALL_SESSION=true
      ;;
    --no-seatd)
      ENABLE_SEATD=false
      ;;
    -h | --help)
      printf 'Usage: %s [--yes] [--install-session] [--no-seatd]\n' "$0"
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$arg" >&2
      exit 1
      ;;
  esac
done

if [[ ! -f /etc/arch-release ]]; then
  printf 'This setup script is intended for Arch Linux.\n' >&2
  exit 1
fi

confirm() {
  local prompt="$1"
  if [[ "$ASSUME_YES" == true ]]; then
    return 0
  fi
  read -r -p "$prompt [y/N]: " answer
  [[ "$answer" =~ ^[Yy]$ ]]
}

run_as_root() {
  if [[ "$(id -u)" -eq 0 ]]; then
    "$@"
  else
    sudo "$@"
  fi
}

run_as_target_user() {
  if [[ "$(id -u)" -eq 0 && "$TARGET_USER" != "root" ]]; then
    runuser -u "$TARGET_USER" -- "$@"
  else
    "$@"
  fi
}

install_packages() {
  if ! confirm 'Install/update Arch packages required by LIME DE?'; then
    printf 'Skipping package installation.\n'
    return
  fi

  local packages=(
    base-devel
    curl
    foot
    git
    libdrm
    libinput
    libxkbcommon
    mesa
    pkgconf
    rustup
    seatd
    systemd
    wayland
    wayland-protocols
  )

  run_as_root pacman -Syu --needed --noconfirm "${packages[@]}"
}

setup_rust() {
  if ! run_as_target_user bash -lc 'command -v cargo >/dev/null 2>&1'; then
    run_as_target_user rustup default stable
  fi

  printf 'Rust: '
  run_as_target_user bash -lc 'cargo --version'
}

setup_seatd() {
  if [[ "$ENABLE_SEATD" != true ]]; then
    printf 'seatd setup skipped; logind will be used when available.\n'
    return
  fi

  run_as_root systemctl enable --now seatd.service

  if getent group seat >/dev/null 2>&1; then
    run_as_root usermod -aG seat "$TARGET_USER"
    printf 'Added %s to group seat.\n' "$TARGET_USER"
  fi

  printf 'seatd enabled. A logout/login is required after a new group assignment.\n'
}

build_project() {
  cd "$PROJECT_ROOT"

  run_as_target_user cargo check --all-targets
  run_as_target_user cargo check --features dev_winit --all-targets
  run_as_target_user cargo check --features native_tty --all-targets
  run_as_target_user cargo build --release --features native_tty --bins
}

install_session() {
  if [[ "$INSTALL_SESSION" != true ]]; then
    return
  fi
  if ! confirm 'Install LIME DE in the system Wayland session list?'; then
    return
  fi

  local bindir=/usr/local/bin
  local sysconfdir=/etc/lime-de
  local sessiondir=/usr/share/wayland-sessions

  run_as_root install -Dm755 "$PROJECT_ROOT/target/release/lime-de" "$bindir/lime-de"
  run_as_root install -Dm755 "$PROJECT_ROOT/target/release/lime-files" "$bindir/lime-files"
  run_as_root install -Dm755 "$PROJECT_ROOT/packaging/lime-de-session" "$bindir/lime-de-session"
  run_as_root install -Dm644 "$PROJECT_ROOT/config/lime-native.toml" "$sysconfdir/lime.toml"
  run_as_root install -Dm644 "$PROJECT_ROOT/packaging/lime.desktop" "$sessiondir/lime.desktop"

  printf 'Installed LIME DE session in %s.\n' "$sessiondir"
}

printf 'LIME DE Arch Linux setup\n'
printf 'Project: %s\n' "$PROJECT_ROOT"
printf 'User: %s\n\n' "$TARGET_USER"

install_packages
setup_rust
setup_seatd
build_project
install_session

printf '\nArch setup complete.\n'
printf 'Manual TTY run:\n'
printf '  Ctrl+Alt+F3\n'
printf '  cd %q\n' "$PROJECT_ROOT"
printf '  ./scripts/arch_run_tty.sh\n'
printf '\nOptional systemd service on tty5:\n'
printf '  sudo %q --tty tty5 --user %q\n' "$PROJECT_ROOT/scripts/arch_install_tty_service.sh" "$TARGET_USER"
