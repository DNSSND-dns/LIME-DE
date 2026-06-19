#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$PROJECT_ROOT"

git fetch origin main
git merge --ff-only origin/main

printf 'Local project now matches origin/main.\n'
