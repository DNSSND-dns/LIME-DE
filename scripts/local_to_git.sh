#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
COMMIT_MESSAGE="${*:-Update LIME DE}"

cd "$PROJECT_ROOT"

git add -A

if ! git diff --cached --quiet; then
  git commit -m "$COMMIT_MESSAGE"
else
  printf 'No local changes to commit.\n'
fi

git pull --rebase origin main
git push origin main

printf 'Local changes are now on origin/main.\n'
