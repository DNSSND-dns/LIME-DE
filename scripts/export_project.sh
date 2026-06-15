#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
OUTPUT_FILE="${1:-$PROJECT_ROOT/project-code.txt}"

cd "$PROJECT_ROOT"

FILES=(
  "Cargo.toml"
  "Cargo.lock"
)

while IFS= read -r file; do
  FILES+=("$file")
done < <(find src -type f -name '*.rs' | sort)

if [[ -d config ]]; then
  while IFS= read -r file; do
    FILES+=("$file")
  done < <(find config -type f -name '*.toml' | sort)
fi

if [[ -d scripts ]]; then
  while IFS= read -r file; do
    FILES+=("$file")
  done < <(find scripts -type f -name '*.sh' | sort)
fi

: > "$OUTPUT_FILE"

for file in "${FILES[@]}"; do
  if [[ -f "$file" ]]; then
    {
      printf '===== %s =====\n' "$file"
      sed -n '1,$p' "$file"
      printf '\n'
    } >> "$OUTPUT_FILE"
  fi
done

printf 'Exported %s files to %s\n' "${#FILES[@]}" "$OUTPUT_FILE"
