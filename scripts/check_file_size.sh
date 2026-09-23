#!/usr/bin/env bash
# Fails when a hand-written .rs file exceeds the line limit.
# Skips gitignored paths (target/), tests/ dirs, and files marked as generated.
set -euo pipefail
limit=500
fail=0
while IFS= read -r f; do
  if head -n 5 "$f" | grep -qE '@generated|automatically generated'; then
    continue
  fi
  lines=$(wc -l < "$f")
  if [ "$lines" -gt "$limit" ]; then
    echo "$f has $lines lines (limit $limit)"
    fail=1
  fi
done < <(git ls-files --cached --others --exclude-standard -- '*.rs' ':!:**/tests/**' ':!:tests/**')
exit "$fail"
