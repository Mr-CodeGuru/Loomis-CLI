#!/usr/bin/env bash
# setup.sh — one-shot onboarding for a fresh clone of LoomisCLI.
# Idempotent: every step checks whether it's already done and skips if so.
# Usage: bash setup.sh

RESULTS=()

report() {
    local step="$1" status="$2" detail="$3"
    local color
    case "$status" in
        OK)      color="\033[32m" ;;  # green
        SKIPPED) color="\033[33m" ;;  # yellow
        FAILED)  color="\033[31m" ;;  # red
        WAITING) color="\033[36m" ;;  # cyan
        *)       color="\033[0m"  ;;
    esac
    echo -e "${color}[$status]\033[0m $step"
    if [[ -n "$detail" ]]; then
        echo -e "\033[90m        $detail\033[0m"
    fi
    RESULTS+=("$status|$step")
}

echo -e "\n\033[35m=== LoomisCLI Setup ===\033[0m\n"

# --- Step 0: safety net -- strip any stray CRLF from shell scripts this script calls,
# in case they were touched/edited on Windows since the repo was cloned.
for f in init-structure.sh tree.sh; do
    if [[ -f "$f" ]]; then
        sed -i 's/\r$//' "$f"
    fi
done

# --- Step 1: directory structure ---
if [[ -d "db" && -d "models" ]]; then
    report "Directory structure" "SKIPPED" "db/ and models/ already exist."
else
    if [[ -f "STRUCTURE.txt" ]]; then
        bash init-structure.sh > /dev/null 2>&1
        report "Directory structure" "OK" "Recreated from STRUCTURE.txt."
    else
        mkdir -p db models
        report "Directory structure" "OK" "STRUCTURE.txt not found — created db/ and models/ directly."
    fi
fi

# --- Step 2: Python venv + dependencies ---
if [[ -d ".venv" ]]; then
    report "Python venv" "SKIPPED" ".venv already exists."
else
    if uv venv --python 3.12 > /dev/null 2>&1; then
        report "Python venv" "OK" "Created via uv (Python 3.12)."
    else
        report "Python venv" "FAILED" "uv venv failed — check uv is installed."
    fi
fi

source .venv/bin/activate
if uv pip install -r requirements.txt > /dev/null 2>&1; then
    report "Python dependencies" "OK" "Installed/verified via uv."
else
    report "Python dependencies" "FAILED" "uv pip install failed."
fi

# --- Step 3: embedding model ---
JINA_CACHE="models/hub/models--jinaai--jina-embeddings-v2-base-code"
if [[ -d "$JINA_CACHE" ]]; then
    report "Embedding model (jina-embeddings-v2-base-code)" "SKIPPED" "Already downloaded."
else
    if python3 loadModels/loadJina.py; then
        report "Embedding model (jina-embeddings-v2-base-code)" "OK" "Downloaded and load-tested."
    else
        report "Embedding model (jina-embeddings-v2-base-code)" "FAILED" "loadJina.py failed."
    fi
fi

# --- Step 4: LLM model ---
GGUF_PATH="models/Llama-3.2-1B-Instruct-Q8_0.gguf"
if [[ -f "$GGUF_PATH" ]]; then
    report "LLM model (Llama-3.2-1B-Instruct-Q8_0.gguf)" "SKIPPED" "Already downloaded."
else
    if python3 loadModels/loadLlamaQ8.py; then
        report "LLM model (Llama-3.2-1B-Instruct-Q8_0.gguf)" "OK" "Downloaded."
    else
        report "LLM model (Llama-3.2-1B-Instruct-Q8_0.gguf)" "FAILED" "loadLlamaQ8.py failed."
    fi
fi

# --- Step 5: Rust build ---
if cargo build > /dev/null 2>&1; then
    report "Rust build" "OK" "cargo build succeeded."
else
    report "Rust build" "FAILED" "cargo build failed."
fi

# --- Step 6: parquet + LanceDB sanity checks ---
if cargo run --example testParquet > /dev/null 2>&1; then
    report "Parquet schema read (Rust)" "OK"
else
    report "Parquet schema read (Rust)" "FAILED"
fi

if [[ -d "db/lancedb/chunks.lance" ]]; then
    report "LanceDB table" "SKIPPED" "db/lancedb/chunks.lance already exists."
else
    if cargo run --example convertLanceDB > /dev/null 2>&1; then
        report "LanceDB table" "OK" "Created and verified with a sample search."
    else
        report "LanceDB table" "FAILED"
    fi
fi

# --- Step 7: llama-server (waits for the user) ---
echo -e "\n\033[36m[WAITING]\033[0m llama-server"
echo -e "\033[90m        This step needs YOU to start llama-server manually, in a separate terminal:\033[0m"
echo -e "        llama-server -m models/Llama-3.2-1B-Instruct-Q8_0.gguf -c 4096 --port 8080"
echo -e "\033[90m        Once it's running and ready, come back here.\033[0m"
read -r -p "        Press Enter once llama-server is running..."

if cargo run --example testLlamaServer > /dev/null 2>&1; then
    report "llama-server connectivity" "OK" "Received a real completion."
else
    report "llama-server connectivity" "FAILED" "Could not reach llama-server — confirm it's actually running on port 8080."
fi

# --- Summary ---
echo -e "\n\033[35m=== Setup Summary ===\033[0m\n"
FAILED_ANY=0
for entry in "${RESULTS[@]}"; do
    status="${entry%%|*}"
    step="${entry#*|}"
    case "$status" in
        OK)      color="\033[32m" ;;
        SKIPPED) color="\033[33m" ;;
        FAILED)  color="\033[31m"; FAILED_ANY=1 ;;
        *)       color="\033[0m"  ;;
    esac
    echo -e "  ${color}[$status]\033[0m $step"
done

if [[ $FAILED_ANY -eq 1 ]]; then
    echo -e "\n\033[31mSome steps failed — check the details above before continuing.\033[0m"
else
    echo -e "\n\033[32mAll steps complete. LoomisCLI environment is ready.\033[0m"
fi