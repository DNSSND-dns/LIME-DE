#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

REMOTE="${REMOTE:-origin}"
BRANCH="${BRANCH:-main}"
RUN_CHECKS=true
DO_PULL=true
DO_PUSH=true
COMMIT_MESSAGE=""

usage() {
  cat <<EOF
Usage: $0 [options]

Sync LIME DE with the configured git remote.

Options:
  -m, --message TEXT  Commit staged project changes with TEXT.
  --no-pull           Do not pull before syncing.
  --no-push           Do not push after commit.
  --no-check          Skip cargo fmt/check.
  -h, --help          Show this help.

Examples:
  ./scripts/sync.sh
  ./scripts/sync.sh -m "Fix dock minimize behavior"
  ./scripts/sync.sh --no-push -m "WIP architecture cleanup"
EOF
}

while (($# > 0)); do
  case "$1" in
    -m | --message)
      shift
      COMMIT_MESSAGE="${1:-}"
      if [[ -z "$COMMIT_MESSAGE" ]]; then
        printf 'Missing commit message.\n' >&2
        exit 1
      fi
      ;;
    --no-pull)
      DO_PULL=false
      ;;
    --no-push)
      DO_PUSH=false
      ;;
    --no-check)
      RUN_CHECKS=false
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      printf 'Unknown option: %s\n\n' "$1" >&2
      usage >&2
      exit 1
      ;;
  esac
  shift
done

cd "$PROJECT_ROOT"

if [[ ! -d .git ]]; then
  printf 'Not a git repository: %s\n' "$PROJECT_ROOT" >&2
  exit 1
fi

printf 'LIME DE git sync\n'
printf 'Project: %s\n' "$PROJECT_ROOT"
printf 'Remote:  %s\n' "$REMOTE"
printf 'Branch:  %s\n\n' "$BRANCH"

if [[ "$DO_PULL" == true ]]; then
  printf 'Pulling latest changes...\n'
  git pull --ff-only "$REMOTE" "$BRANCH"
  printf '\n'
fi

if [[ "$RUN_CHECKS" == true ]]; then
  printf 'Running cargo fmt --all --check...\n'
  cargo fmt --all --check
  printf 'Running cargo check...\n'
  cargo check
  printf '\n'
fi

printf 'Current status:\n'
git status --short
printf '\n'

if [[ -n "$COMMIT_MESSAGE" ]]; then
  if git diff --quiet && git diff --cached --quiet; then
    printf 'No changes to commit.\n\n'
  else
    printf 'Creating commit: %s\n' "$COMMIT_MESSAGE"
    git add .
    git commit -m "$COMMIT_MESSAGE"
    printf '\n'
  fi
fi

if [[ "$DO_PUSH" == true ]]; then
  printf 'Pushing to %s/%s...\n' "$REMOTE" "$BRANCH"
  git push "$REMOTE" "$BRANCH"
fi

printf '\nDone.\n'
