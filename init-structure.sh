#!/usr/bin/env bash
# init-structure.sh — macOS/Linux equivalent of init-structure.ps1
# Reads STRUCTURE.txt (as produced by tree.sh) and recreates the directories on disk.
# Directories only — files come from `git clone`, this script never touches them.
# Safe to re-run: skips anything that already exists.
#
# Usage:  bash init-structure.sh
#     or: chmod +x init-structure.sh && ./init-structure.sh

structure_file="STRUCTURE.txt"

if [[ ! -f "$structure_file" ]]; then
    echo "Error: could not find $structure_file in current directory." >&2
    exit 1
fi

declare -a stack
stack[0]="."

while IFS= read -r line; do
    # Strip trailing \r in case STRUCTURE.txt was generated on Windows (CRLF)
    line="${line%$'\r'}"
    [[ -z "$line" ]] && continue

    # Count leading spaces to determine indent level (2 spaces per level, matching tree.sh)
    stripped="${line#"${line%%[! ]*}"}"
    leading_spaces=$(( ${#line} - ${#stripped} ))
    level=$(( leading_spaces / 2 ))

    parent_path="${stack[$level]:-.}"

    if [[ "$stripped" == */ ]]; then
        # Directory
        dirname_only="${stripped%/}"
        full_path="${parent_path}/${dirname_only}"
        if [[ ! -d "$full_path" ]]; then
            mkdir -p "$full_path"
            echo "Created dir : $full_path"
        fi
        stack[$((level + 1))]="$full_path"
    fi
    # Files are skipped intentionally — they come from `git clone`, not this script.
done < "$structure_file"

echo ""
echo "Done. Directories recreated from $structure_file."