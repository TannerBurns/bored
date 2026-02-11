#!/usr/bin/env bash
#
# draft-release-notes.sh — Generate a draft release notes entry from git history.
#
# Usage:
#   ./scripts/draft-release-notes.sh [version]
#
# If version is omitted, it defaults to "NEXT".
#
# The script finds the most recent tag, collects all commits since that tag,
# categorises them by their prefix (Feat/Feature → New Features, Fix → Bug Fixes,
# everything else → Improvements), and prints a JSON object ready to prepend to
# src-tauri/release-notes.json.
#
# Steps to add release notes:
#   1. Run this script:  ./scripts/draft-release-notes.sh v0.1.0-beta.12
#   2. Review the output — reword items to be user-facing, remove noise.
#   3. Prepend the entry into src-tauri/release-notes.json (newest first).
#   4. Commit, tag, push.

set -euo pipefail

VERSION="${1:-NEXT}"
# Strip leading 'v' for the JSON entry
VERSION="${VERSION#v}"

LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
if [ -z "$LAST_TAG" ]; then
  echo "No tags found — listing all commits." >&2
  RANGE="HEAD"
else
  echo "Commits since $LAST_TAG:" >&2
  RANGE="${LAST_TAG}..HEAD"
fi

TODAY=$(date +%Y-%m-%d)

# Collect commits (subject line only, skip merge commits)
FEATURES=()
FIXES=()
IMPROVEMENTS=()

while IFS= read -r line; do
  [ -z "$line" ] && continue

  # Normalise: strip PR number suffix like " (#42)"
  clean=$(echo "$line" | sed 's/ (#[0-9]*)$//')

  # Categorise by conventional-ish prefix
  lower=$(echo "$clean" | tr '[:upper:]' '[:lower:]')
  case "$lower" in
    feat:*|feat/*|feature:*|feature/*|"add "*)
      # Strip the prefix for cleaner output
      item=$(echo "$clean" | sed -E 's/^(feat|feature)[:/] *//i; s/^add //i')
      FEATURES+=("$item")
      ;;
    fix:*|fix/*|"fix "*)
      item=$(echo "$clean" | sed -E 's/^fix[:/] *//i; s/^fix //i')
      FIXES+=("$item")
      ;;
    *)
      IMPROVEMENTS+=("$clean")
      ;;
  esac
done < <(git log "$RANGE" --no-merges --format='%s')

# Helper: print a JSON array of strings
json_array() {
  local arr=("$@")
  local first=true
  printf '['
  for item in "${arr[@]}"; do
    if $first; then first=false; else printf ','; fi
    # Escape double quotes and backslashes for JSON
    escaped=$(printf '%s' "$item" | sed 's/\\/\\\\/g; s/"/\\"/g')
    printf '\n        "%s"' "$escaped"
  done
  printf '\n      ]'
}

# Build categories array
CATEGORIES=()
if [ ${#FEATURES[@]} -gt 0 ]; then
  CATEGORIES+=("NEW_FEATURES")
fi
if [ ${#IMPROVEMENTS[@]} -gt 0 ]; then
  CATEGORIES+=("IMPROVEMENTS")
fi
if [ ${#FIXES[@]} -gt 0 ]; then
  CATEGORIES+=("FIXES")
fi

if [ ${#CATEGORIES[@]} -eq 0 ]; then
  echo "No commits found since $LAST_TAG." >&2
  exit 0
fi

# Print the JSON entry
echo ""
echo "  {"
echo "    \"version\": \"$VERSION\","
echo "    \"published_at\": \"$TODAY\","
echo "    \"summary\": \"TODO: Write a one-line summary\","
echo "    \"notes\": ["

printed=0
for cat in "${CATEGORIES[@]}"; do
  if [ $printed -gt 0 ]; then echo ","; fi
  case "$cat" in
    NEW_FEATURES)
      printf '      {\n        "category": "New Features",\n        "items": '
      json_array "${FEATURES[@]}"
      printf '\n      }'
      ;;
    IMPROVEMENTS)
      printf '      {\n        "category": "Improvements",\n        "items": '
      json_array "${IMPROVEMENTS[@]}"
      printf '\n      }'
      ;;
    FIXES)
      printf '      {\n        "category": "Bug Fixes",\n        "items": '
      json_array "${FIXES[@]}"
      printf '\n      }'
      ;;
  esac
  printed=$((printed + 1))
done

echo ""
echo "    ]"
echo "  }"
echo ""
echo "# Copy the JSON above and prepend it to src-tauri/release-notes.json" >&2
