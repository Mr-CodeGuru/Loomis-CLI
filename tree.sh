#!/usr/bin/env bash
# tree.sh — macOS/Linux equivalent of tree.ps1
# Prints project structure: all directories, filters out large binary files by extension,
# and collapses noisy cache directories (shown but not recursed into).
#
# Usage:  bash tree.sh
#     or: chmod +x tree.sh && ./tree.sh

exclude_dirs=(".git" "venv" ".venv" "env" "lancedb" "target" "node_modules" "__pycache__" ".pytest_cache" ".mypy_cache" ".idea" ".vscode" ".cache")
exclude_ext=("gguf" "bin" "safetensors" "pt" "pth" "onnx" "parquet" "lance")
# Directories shown as a placeholder only — contents not recursed into (e.g. HF cache internals)
collapse_dirs=("models")

contains() {
    local needle="$1"; shift
    for item in "$@"; do
        [[ "$item" == "$needle" ]] && return 0
    done
    return 1
}

show_tree() {
    local path="$1"
    local indent="$2"

    # Directories first, sorted
    while IFS= read -r -d '' dir; do
        local name
        name=$(basename "$dir")
        contains "$name" "${exclude_dirs[@]}" && continue
        echo "${indent}${name}/"
        if ! contains "$name" "${collapse_dirs[@]}"; then
            show_tree "$dir" "${indent}  "
        fi
    done < <(find "$path" -mindepth 1 -maxdepth 1 -type d -print0 | sort -z)

    # Then files, sorted
    while IFS= read -r -d '' file; do
        local name ext
        name=$(basename "$file")
        ext="${name##*.}"
        contains "$ext" "${exclude_ext[@]}" && continue
        echo "${indent}${name}"
    done < <(find "$path" -mindepth 1 -maxdepth 1 -type f -print0 | sort -z)
}

show_tree "." "" > STRUCTURE.txt
cat STRUCTURE.txt