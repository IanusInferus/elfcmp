#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)

shopt -s nullglob
old_artifacts=("$SCRIPT_DIR"/.elfcmp-test.*)
for directory in "${old_artifacts[@]}"; do
    if [[ ! -d $directory || $directory != "$SCRIPT_DIR"/.elfcmp-test.* ]]; then
        echo "refusing to remove unexpected path: $directory" >&2
        exit 2
    fi
    rm -rf -- "$directory"
done
if (( ${#old_artifacts[@]} > 0 )); then
    echo "Removed ${#old_artifacts[@]} previous test artifact directories."
fi

bash "$SCRIPT_DIR/compat/test.sh"
bash "$SCRIPT_DIR/dlopen/test.sh"

echo "All elfcmp integration tests passed."
