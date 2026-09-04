# setup.ps1 - one-shot onboarding for a fresh clone of LoomisCLI.
# Idempotent: every step checks whether it's already done and skips if so.
# Usage:  powershell -ExecutionPolicy Bypass -File setup.ps1

$ErrorActionPreference = "Continue"
$results = @()

function Report {
    param([string]$Step, [string]$Status, [string]$Detail = "")
    $color = switch ($Status) {
        "OK"      { "Green" }
        "SKIPPED" { "Yellow" }
        "FAILED"  { "Red" }
        "WAITING" { "Cyan" }
        default   { "White" }
    }
    Write-Host "[$Status] $Step" -ForegroundColor $color
    if ($Detail) { Write-Host "        $Detail" -ForegroundColor DarkGray }
    $script:results += [PSCustomObject]@{ Step = $Step; Status = $Status }
}

Write-Host "`n=== LoomisCLI Setup ===`n" -ForegroundColor Magenta

# --- Step 1: directory structure ---
if ((Test-Path "db") -and (Test-Path "models")) {
    Report "Directory structure" "SKIPPED" "db/ and models/ already exist."
} else {
    if (Test-Path "STRUCTURE.txt") {
        powershell -ExecutionPolicy Bypass -File init-structure.ps1 | Out-Null
        Report "Directory structure" "OK" "Recreated from STRUCTURE.txt."
    } else {
        New-Item -ItemType Directory -Path "db" -Force | Out-Null
        New-Item -ItemType Directory -Path "models" -Force | Out-Null
        Report "Directory structure" "OK" "STRUCTURE.txt not found - created db/ and models/ directly."
    }
}

# --- Step 2: Python venv + dependencies ---
if (Test-Path ".venv") {
    Report "Python venv" "SKIPPED" ".venv already exists."
} else {
    try {
        uv venv --python 3.12 2>&1 | Out-Null
        Report "Python venv" "OK" "Created via uv (Python 3.12)."
    } catch {
        Report "Python venv" "FAILED" $_.Exception.Message
    }
}

try {
    & ".\.venv\Scripts\Activate.ps1"
    uv pip install -r requirements.txt 2>&1 | Out-Null
    Report "Python dependencies" "OK" "Installed/verified via uv."
} catch {
    Report "Python dependencies" "FAILED" $_.Exception.Message
}

# --- Step 3: embedding model ---
$jinaCachePattern = "models\hub\models--jinaai--jina-embeddings-v2-base-code"
if (Test-Path $jinaCachePattern) {
    Report "Embedding model (jina-embeddings-v2-base-code)" "SKIPPED" "Already downloaded."
} else {
    try {
        python loadModels\loadJina.py
        Report "Embedding model (jina-embeddings-v2-base-code)" "OK" "Downloaded and load-tested."
    } catch {
        Report "Embedding model (jina-embeddings-v2-base-code)" "FAILED" $_.Exception.Message
    }
}

# --- Step 4: LLM model ---
$ggufPath = "models\Llama-3.2-1B-Instruct-Q8_0.gguf"
if (Test-Path $ggufPath) {
    Report "LLM model (Llama-3.2-1B-Instruct-Q8_0.gguf)" "SKIPPED" "Already downloaded."
} else {
    try {
        python loadModels\loadLlamaQ8.py
        Report "LLM model (Llama-3.2-1B-Instruct-Q8_0.gguf)" "OK" "Downloaded."
    } catch {
        Report "LLM model (Llama-3.2-1B-Instruct-Q8_0.gguf)" "FAILED" $_.Exception.Message
    }
}

# --- Step 5: Rust build ---
try {
    cargo build 2>&1 | Out-Null
    Report "Rust build" "OK" "cargo build succeeded."
} catch {
    Report "Rust build" "FAILED" $_.Exception.Message
}

# --- Step 6: parquet + LanceDB sanity checks ---
try {
    cargo run --example testParquet 2>&1 | Out-Null
    Report "Parquet schema read (Rust)" "OK"
} catch {
    Report "Parquet schema read (Rust)" "FAILED" $_.Exception.Message
}

if (Test-Path "db\lancedb\chunks.lance") {
    Report "LanceDB table" "SKIPPED" "db\lancedb\chunks.lance already exists."
} else {
    try {
        cargo run --example convertLanceDB 2>&1 | Out-Null
        Report "LanceDB table" "OK" "Created and verified with a sample search."
    } catch {
        Report "LanceDB table" "FAILED" $_.Exception.Message
    }
}

# --- Step 7: llama-server (waits for the user) ---
Write-Host "`n[WAITING] llama-server" -ForegroundColor Cyan
Write-Host "        This step needs YOU to start llama-server manually, in a separate terminal:" -ForegroundColor DarkGray
Write-Host "        llama-server -m models\Llama-3.2-1B-Instruct-Q8_0.gguf -c 4096 --port 8080" -ForegroundColor White
Write-Host "        Once it's running and ready, come back here." -ForegroundColor DarkGray
Read-Host "        Press Enter once llama-server is running"

try {
    cargo run --example testLlamaServer 2>&1 | Out-Null
    Report "llama-server connectivity" "OK" "Received a real completion."
} catch {
    Report "llama-server connectivity" "FAILED" "Could not reach llama-server - confirm it's actually running on port 8080."
}

# --- Summary ---
Write-Host "`n=== Setup Summary ===`n" -ForegroundColor Magenta
foreach ($r in $results) {
    $color = switch ($r.Status) {
        "OK"      { "Green" }
        "SKIPPED" { "Yellow" }
        "FAILED"  { "Red" }
        default   { "White" }
    }
    Write-Host ("  [{0}] {1}" -f $r.Status, $r.Step) -ForegroundColor $color
}

$failed = $results | Where-Object { $_.Status -eq "FAILED" }
if ($failed) {
    Write-Host "`nSome steps failed - check the details above before continuing." -ForegroundColor Red
} else {
    Write-Host "`nAll steps complete. LoomisCLI environment is ready." -ForegroundColor Green
}