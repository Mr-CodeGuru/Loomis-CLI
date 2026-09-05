#!/usr/bin/env bash
# scripts/tree.sh — macOS/Linux equivalent of tree.ps1
# Usage: bash scripts/tree.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
SRC_DIR="$ROOT_DIR/src"
output_file="$ROOT_DIR/STRUCTURE.txt"

exclude_dirs='^(\.git|venv|\.venv|env|target|lancedb|node_modules|__pycache__|\.pytest_cache|\.mypy_cache|\.idea|\.vscode|\.cache)$'
exclude_file_ext='\.(gguf|bin|safetensors|pt|pth|onnx|parquet|lance|DS_Store)$'
collapse_dirs='^(models|dbe)$'

show_tree() {
    local dir="$1"
    local indent="$2"

    local is_inside_src=0
    if [[ "$dir" =~ ^"$SRC_DIR" ]]; then
        is_inside_src=1
    fi

    local entries=()
    while IFS= read -r entry; do
        [[ -n "$entry" ]] && entries+=("$entry")
    done < <(find "$dir" -mindepth 1 -maxdepth 1 ! -name "." | sort)

    local subdirs=()
    local files=()
    for entry in "${entries[@]}"; do
        local base
        base="$(basename "$entry")"
        if [[ -d "$entry" ]]; then
            if [[ $is_inside_src -eq 0 && "$base" =~ $exclude_dirs ]]; then
                continue
            fi
            subdirs+=("$entry")
        else
            if [[ $is_inside_src -eq 0 && "$base" =~ $exclude_file_ext ]]; then
                continue
            fi
            files+=("$entry")
        fi
    done

    for subdir in "${subdirs[@]}"; do
        local base
        base="$(basename "$subdir")"
        echo "${indent}${base}/"
        if [[ $is_inside_src -eq 1 || ! "$base" =~ $collapse_dirs ]]; then
            show_tree "$subdir" "${indent}  "
        fi
    done

    for file in "${files[@]}"; do
        local base
        base="$(basename "$file")"
        echo "${indent}${base}"
    done
}

show_tree "$ROOT_DIR" "" > "$output_file"
cat "$output_file"
