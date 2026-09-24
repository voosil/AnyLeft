#!/usr/bin/env bash
# AnyLeft release pipeline: bump version → commit → tag → push → rebuild → install.
#
# Usage:
#   release.sh                        infer the semver bump from commits since the last tag
#   release.sh 0.3.0                  use an explicit version
#   release.sh --minor|--patch|--major  force a bump size instead of inferring
#   release.sh --message "..."        commit uncommitted feature work first, with this message
#
# Follows AGENTS.md's release checklist. macOS-only (the install step requires it).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Walk up from the script until the repo root (package.json) is found — robust
# against the skill being invoked from anywhere, symlinked, or moved.
REPO_ROOT="$SCRIPT_DIR"
while [[ ! -f "$REPO_ROOT/package.json" && "$REPO_ROOT" != "/" ]]; do
  REPO_ROOT="$(dirname "$REPO_ROOT")"
done
if [[ ! -f "$REPO_ROOT/package.json" ]]; then
  echo "error: repo root not found (no package.json above the script)." >&2
  exit 1
fi
cd "$REPO_ROOT"

[[ "$(uname -s)" == "Darwin" ]] || { echo "error: release is macOS-only (install step)." >&2; exit 1; }
[[ "$(git branch --show-current)" == "main" ]] || { echo "error: release runs on the main branch only." >&2; exit 1; }
command -v jq >/dev/null || { echo "error: jq is required." >&2; exit 1; }

BUMP=""
VERSION=""
MESSAGE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --minor|--patch|--major) BUMP="${1#--}" ;;
    --message) MESSAGE="${2:?--message needs a value}"; shift ;;
    *) VERSION="$1" ;;
  esac
  shift
done

# Step 1 — nothing uncommitted when the version bump lands, so the tag points at
# a known-good tree. Feature work is committed first: either by the caller via
# --message, or earlier by the agent invoking the skill.
if [[ -n "$(git status --porcelain)" ]]; then
  if [[ -z "$MESSAGE" ]]; then
    echo "error: uncommitted changes. Commit them first, or pass --message \"...\"." >&2
    exit 1
  fi
  echo ">> committing pending work"
  git add -A
  git commit -m "$MESSAGE"
fi

if [[ -z "$VERSION" ]]; then
  if [[ -z "$BUMP" ]]; then
    # Infer from conventional commits since the last tag: feat → minor,
    # BREAKING CHANGE / ! → major, everything else → patch. Highest wins.
    RANGE="$(git describe --tags --abbrev=0 2>/dev/null || echo v0.0.0)..HEAD"
    SUBJECTS="$(git log --format=%s "$RANGE" 2>/dev/null)"
    if [[ -z "$SUBJECTS" ]]; then
      echo "error: no commits since the last tag — nothing to release." >&2
      exit 1
    fi
    if grep -qE "BREAKING CHANGE|!:" <<<"$SUBJECTS"; then BUMP="major";
    elif grep -qE "^feat\b|^feat\(" <<<"$SUBJECTS"; then BUMP="minor";
    else BUMP="patch"; fi
    echo ">> inferred bump: $BUMP (from commits since last tag)"
  fi
else
  echo ">> explicit version: $VERSION"
fi

# Step 2 — bump the version in all three places AGENTS.md requires.
CURRENT="$(jq -r .version package.json)"
[[ "$CURRENT" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "error: unparseable version '$CURRENT'." >&2; exit 1; }
if [[ -z "$VERSION" ]]; then
  IFS=. read -r MAJ MIN PAT <<<"$CURRENT"
  case "$BUMP" in
    major) MAJ=$((MAJ+1)); MIN=0; PAT=0 ;;
    minor) MIN=$((MIN+1)); PAT=0 ;;
    patch) PAT=$((PAT+1)) ;;
  esac
  VERSION="$MAJ.$MIN.$PAT"
fi
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: invalid version '$VERSION'." >&2; exit 1
fi
[[ "$VERSION" != "$CURRENT" ]] || { echo "error: version is already $VERSION." >&2; exit 1; }

echo ">> bumping $CURRENT → $VERSION"
sed -i '' "s/\"version\": \"$CURRENT\"/\"version\": \"$VERSION\"/" package.json
sed -i '' "s/^version = \"$CURRENT\"/version = \"$VERSION\"/" src-tauri/Cargo.toml
sed -i '' "s/\"version\": \"$CURRENT\"/\"version\": \"$VERSION\"/" src-tauri/tauri.conf.json
grep -q "$VERSION" package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json

# Step 3-4 — commit the bump, tag, and push both.
git add -A
git commit -m "chore(release): bump version to $VERSION"
git tag -a "v$VERSION" -m "v$VERSION"
git push origin main "v$VERSION"

# Step 5-6 — rebuild, install to /Applications, and launch.
# CI may be set to an arbitrary value in agent shells; the tauri CLI maps it to
# its boolean --ci flag and rejects anything but true/false.
export CI=true
pnpm app:install --latest
open /Applications/AnyLeft.app

echo
echo "released v$VERSION — verify: pgrep -x anyleft"
