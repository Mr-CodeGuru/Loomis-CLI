#!/usr/bin/env bash
# scripts/init-structure.sh — macOS/Linux equivalent of init-structure.ps1

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
SRC_DIR="$ROOT_DIR/src"
structure_file="$ROOT_DIR/STRUCTURE.txt"

if [[ ! -f "$structure_file" ]]; then
    structure_file="STRUCTURE.txt"
fi

if [[ ! -f "$structure_file" ]]; then
    echo "Error: could not find STRUCTURE.txt." >&2
    exit 1
fi

exclude_dirs='^(\.git|target|node_modules|\.venv|venv|__pycache__|\.cache)$'

declare -a stack
stack[0]="$ROOT_DIR"

while IFS= read -r line; do
    line="${line%$'\r'}"
    [[ -z "$line" ]] && continue

    stripped="${line#"${line%%[! ]*}"}"
    leading_spaces=$(( ${#line} - ${#stripped} ))
    level=$(( leading_spaces / 2 ))

    parent_path="${stack[$level]:-$ROOT_DIR}"

    if [[ "$stripped" == */ ]]; then
        dirname_only="${stripped%/}"
        full_path="${parent_path}/${dirname_only}"

        # Never exclude anything under src/
        if [[ ! "$full_path" =~ ^"$SRC_DIR" ]]; then
            if [[ "$dirname_only" =~ $exclude_dirs ]]; then
                continue
            fi
        fi

        if [[ ! -d "$full_path" ]]; then
            mkdir -p "$full_path"
            echo "Created dir : $full_path"
        fi
        stack[$((level + 1))]="$full_path"
    fi
done < "$structure_file"

echo ""
echo "Done. Directories recreated from $structure_file."
