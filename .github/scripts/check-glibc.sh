#!/usr/bin/env bash
set -euo pipefail

max="${1:?usage: check-glibc.sh <max-version> <file>...}"
shift
status=0
for file in "$@"; do
  highest="$(objdump -T "$file" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/^GLIBC_//' | sort -V | tail -n 1 || true)"
  if [ -z "$highest" ]; then
    echo "check-glibc: $file links no glibc symbols" >&2
    status=1
    continue
  fi
  echo "$file: GLIBC $highest"
  if [ "$(printf '%s\n%s\n' "$highest" "$max" | sort -V | tail -n 1)" != "$max" ]; then
    echo "check-glibc: $file needs glibc $highest, more than $max" >&2
    status=1
  fi
done
exit "$status"
