#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
PROJECT_NAME="$(basename "$PROJECT_ROOT")"

EXCLUDES=(
  "--exclude=target"
  "--exclude=project-code.txt"
  "--exclude=.git"
  "--exclude=.idea"
  "--exclude=.vscode"
  "--exclude=*.log"
  "--exclude=*.tmp"
)

print_mounts() {
  findmnt -rn -o TARGET,SOURCE,FSTYPE,SIZE,AVAIL |
    awk '$1 ~ "^/media/|^/mnt/|^/run/media/" { print }'
}

print_header() {
  clear >&2 2>/dev/null || true
  printf '+------------------------------------------------------------+\n' >&2
  printf '| LIME DE project transfer                                  |\n' >&2
  printf '+------------------------------------------------------------+\n' >&2
  printf 'Project: %s\n\n' "$PROJECT_ROOT" >&2
}

print_mount_table() {
  local -n rows_ref=$1

  if ((${#rows_ref[@]} == 0)); then
    printf 'No mounted external drives found.\n\n' >&2
    return
  fi

  printf 'Mounted destinations:\n' >&2
  printf '  %-3s %-28s %-8s %-8s %-8s %s\n' '#' 'Mount point' 'FS' 'Size' 'Free' 'Device' >&2
  printf '  %-3s %-28s %-8s %-8s %-8s %s\n' '---' '----------------------------' '--------' '--------' '--------' '------' >&2

  for index in "${!rows_ref[@]}"; do
    read -r target source fstype size avail <<<"${rows_ref[$index]}"
    printf '  %-3s %-28s %-8s %-8s %-8s %s\n' \
      "$((index + 1))" "$target" "$fstype" "$size" "$avail" "$source" >&2
  done

  printf '\n' >&2
}

read_manual_path() {
  local manual_path

  printf 'Destination path: ' >&2
  read -r -e manual_path
  printf '%s\n' "$manual_path"
}

choose_destination() {
  local choice target

  while true; do
    mapfile -t mounts < <(print_mounts)

    print_header
    print_mount_table mounts

    printf 'Options:\n' >&2
    printf '  number  Copy to selected drive\n' >&2
    printf '  m       Enter path manually\n' >&2
    printf '  r       Refresh drive list\n' >&2
    printf '  q       Quit\n\n' >&2

    printf 'Choose destination: ' >&2
    read -r choice

    case "$choice" in
      q | Q)
        return 1
        ;;
      r | R)
        continue
        ;;
      m | M | 0)
        read_manual_path
        return 0
        ;;
      *)
        if [[ "$choice" =~ ^[0-9]+$ ]] && ((choice > 0 && choice <= ${#mounts[@]})); then
          read -r target _ <<<"${mounts[$((choice - 1))]}"
          printf '%s\n' "$target"
          return 0
        fi

        printf 'Invalid choice: %s\n' "$choice" >&2
        printf 'Press Enter to continue...' >&2
        read -r _
        ;;
    esac
  done
}

if ! DESTINATION_ROOT="$(choose_destination)"; then
  printf 'Cancelled.\n' >&2
  exit 0
fi

if [[ -z "$DESTINATION_ROOT" ]]; then
  printf 'No destination selected.\n' >&2
  exit 1
fi

mkdir -p "$DESTINATION_ROOT"

if [[ ! -d "$DESTINATION_ROOT" ]]; then
  printf 'Destination is not a directory: %s\n' "$DESTINATION_ROOT" >&2
  exit 1
fi

DESTINATION="$DESTINATION_ROOT/$PROJECT_NAME"

printf '\nCopy summary:\n'
printf '  From: %s\n' "$PROJECT_ROOT"
printf '  To:   %s\n' "$DESTINATION"
printf '  Keep: src, config, scripts, docs, assets, reference/anvil donor files\n'
printf '  Skip: target, .git, IDE folders, logs, tmp files, project-code.txt\n\n'

printf 'Start copy? [y/N]: ' >&2
read -r confirm_copy
if [[ ! "$confirm_copy" =~ ^[Yy]$ ]]; then
  printf 'Cancelled.\n'
  exit 0
fi

printf '\nCopying clean project...\n\n'

if command -v rsync >/dev/null 2>&1; then
  mkdir -p "$DESTINATION"
  rsync -a --delete --info=stats2 "${EXCLUDES[@]}" "$PROJECT_ROOT/" "$DESTINATION/"
else
  mkdir -p "$DESTINATION"
  tar \
    --exclude='./target' \
    --exclude='./project-code.txt' \
    --exclude='./.git' \
    --exclude='./.idea' \
    --exclude='./.vscode' \
    --exclude='*.log' \
    -C "$PROJECT_ROOT" \
    -cf - . | tar -C "$DESTINATION" -xf -
fi

sync

copied_files="$(find "$DESTINATION" -type f | wc -l)"

printf 'Done.\n'
printf 'Copied project: %s\n' "$DESTINATION"
printf 'Copied files: %s\n' "$copied_files"
printf '\nOn the other computer run:\n'
printf '  cd %q\n' "$DESTINATION"
printf '  ./scripts/setup_lime_de.sh --yes\n'
printf '\nDev mode:\n'
printf '  cargo run -- --backend dev-winit\n'
printf '\nTTY backend smoke test:\n'
printf '  cargo run --features native_tty -- --backend native\n'
