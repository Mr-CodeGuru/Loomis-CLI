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

# --- Step 0: safety net -- strip any stray CRLF from shell scripts this script calls
for f in scripts/init-structure.sh scripts/tree.sh init-structure.sh tree.sh; do
    if [[ -f "$f" ]]; then
        sed -i 's/\r$//' "$f" 2>/dev/null || sed -i '' 's/\r$//' "$f" 2>/dev/null || true
    fi
done

# --- Step 1: directory structure ---
if [[ -d "dbe" && -d "models" ]]; then
    report "Directory structure" "SKIPPED" "dbe/ and models/ already exist."
else
    if [[ -f "STRUCTURE.txt" ]]; then
        if [[ -f "scripts/init-structure.sh" ]]; then
            bash scripts/init-structure.sh > /dev/null 2>&1
        else
            bash init-structure.sh > /dev/null 2>&1
        fi
        report "Directory structure" "OK" "Recreated from STRUCTURE.txt."
    else
        mkdir -p dbe models
        report "Directory structure" "OK" "STRUCTURE.txt not found — created dbe/ and models/ directly."
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

# --- Step 6: parquet embeddings dataset ---
mkdir -p dbe
PARQUET_PATH="dbe/embeddings.parquet"
PARQUET_URL="https://huggingface.co/datasets/MrDevCoder01/LoomisDB/resolve/main/embeddings.parquet"

if [[ -f "$PARQUET_PATH" ]]; then
    report "Parquet dataset (embeddings.parquet)" "SKIPPED" "Already downloaded."
else
    echo -e "\033[90m        Downloading embeddings.parquet (~240MB)...\033[0m"
    TOKEN="${HF_TOKEN:-${HUGGING_FACE_HUB_TOKEN:-}}"
    if [[ -z "$TOKEN" && -f "$HOME/.cache/huggingface/token" ]]; then
        TOKEN=$(head -n 1 "$HOME/.cache/huggingface/token" 2>/dev/null | tr -d '[:space:]')
    fi
    if [[ -z "$TOKEN" && -n "${HF_HOME:-}" && -f "$HF_HOME/token" ]]; then
        TOKEN=$(head -n 1 "$HF_HOME/token" 2>/dev/null | tr -d '[:space:]')
    fi

    AUTH_HEADER=()
    if [[ -n "$TOKEN" ]]; then
        AUTH_HEADER=(-H "Authorization: Bearer $TOKEN")
    fi

    if command -v curl > /dev/null 2>&1; then
        if curl -# -fL --retry 3 --retry-delay 2 "${AUTH_HEADER[@]}" "$PARQUET_URL" -o "$PARQUET_PATH"; then
            report "Parquet dataset (embeddings.parquet)" "OK" "Downloaded."
        else
            rm -f "$PARQUET_PATH"
            report "Parquet dataset (embeddings.parquet)" "FAILED" "curl download failed — verify network or set HF_TOKEN if private."
        fi
    elif command -v wget > /dev/null 2>&1; then
        WGET_HEADER=()
        if [[ -n "$TOKEN" ]]; then
            WGET_HEADER=(--header="Authorization: Bearer $TOKEN")
        fi
        if wget -q --show-progress "${WGET_HEADER[@]}" "$PARQUET_URL" -O "$PARQUET_PATH"; then
            report "Parquet dataset (embeddings.parquet)" "OK" "Downloaded."
        else
            rm -f "$PARQUET_PATH"
            report "Parquet dataset (embeddings.parquet)" "FAILED" "wget download failed — verify network or set HF_TOKEN if private."
        fi
    else
        report "Parquet dataset (embeddings.parquet)" "FAILED" "Neither curl nor wget found."
    fi
fi

# --- Step 7: parquet + LanceDB sanity checks ---
if cargo run --example testParquet > /dev/null 2>&1; then
    report "Parquet schema read (Rust)" "OK"
else
    report "Parquet schema read (Rust)" "FAILED"
fi

if [[ -d "dbe/lancedb/chunks.lance" ]]; then
    report "LanceDB table" "SKIPPED" "dbe/lancedb/chunks.lance already exists."
else
    if cargo run --example convertLanceDB > /dev/null 2>&1; then
        report "LanceDB table" "OK" "Created and verified with a sample search."
    else
        report "LanceDB table" "FAILED"
    fi
fi

# --- Step 8: llama-server (waits for the user) ---
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
