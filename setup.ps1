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
if ((Test-Path "dbe") -and (Test-Path "models")) {
    Report "Directory structure" "SKIPPED" "dbe/ and models/ already exist."
} else {
    if (Test-Path "STRUCTURE.txt") {
        if (Test-Path "scripts\init-structure.ps1") {
            powershell -ExecutionPolicy Bypass -File scripts\init-structure.ps1 | Out-Null
        } elseif (Test-Path "init-structure.ps1") {
            powershell -ExecutionPolicy Bypass -File init-structure.ps1 | Out-Null
        }
        Report "Directory structure" "OK" "Recreated from STRUCTURE.txt."
    } else {
        New-Item -ItemType Directory -Path "dbe" -Force | Out-Null
        New-Item -ItemType Directory -Path "models" -Force | Out-Null
        Report "Directory structure" "OK" "STRUCTURE.txt not found - created dbe/ and models/ directly."
    }
}

# --- Step 2: Python venv + dependencies ---
if (Test-Path ".venv") {
    Report "Python venv" "SKIPPED" ".venv already exists."
} else {
    try {
        uv venv --python 3.12 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "uv venv failed" }
        Report "Python venv" "OK" "Created via uv (Python 3.12)."
    } catch {
        Report "Python venv" "FAILED" "uv venv failed - check uv is installed."
    }
}

try {
    & ".\.venv\Scripts\Activate.ps1"
    uv pip install -r requirements.txt 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "uv pip install failed" }
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
        if ($LASTEXITCODE -ne 0) { throw "loadJina.py failed" }
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
        if ($LASTEXITCODE -ne 0) { throw "loadLlamaQ8.py failed" }
        Report "LLM model (Llama-3.2-1B-Instruct-Q8_0.gguf)" "OK" "Downloaded."
    } catch {
        Report "LLM model (Llama-3.2-1B-Instruct-Q8_0.gguf)" "FAILED" $_.Exception.Message
    }
}

# --- Step 5: Rust build ---
try {
    cargo build 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    Report "Rust build" "OK" "cargo build succeeded."
} catch {
    Report "Rust build" "FAILED" $_.Exception.Message
}

# --- Step 6: parquet embeddings dataset ---
if (-not (Test-Path "dbe")) {
    New-Item -ItemType Directory -Path "dbe" -Force | Out-Null
}
$parquetPath = "dbe\embeddings.parquet"
$parquetUrl = "https://huggingface.co/datasets/MrDevCoder01/LoomisDB/resolve/main/embeddings.parquet"

if (Test-Path $parquetPath) {
    Report "Parquet dataset (embeddings.parquet)" "SKIPPED" "Already downloaded."
} else {
    Write-Host "        Downloading embeddings.parquet (~240MB)..." -ForegroundColor DarkGray
    $token = $env:HF_TOKEN
    if (-not $token) { $token = $env:HUGGING_FACE_HUB_TOKEN }
    if (-not $token) {
        $hfTokenFile = Join-Path $env:USERPROFILE ".cache\huggingface\token"
        if (Test-Path $hfTokenFile) {
            $token = (Get-Content $hfTokenFile -Raw).Trim()
        }
    }
    if (-not $token -and $env:HF_HOME) {
        $hfHomeToken = Join-Path $env:HF_HOME "token"
        if (Test-Path $hfHomeToken) {
            $token = (Get-Content $hfHomeToken -Raw).Trim()
        }
    }

    try {
        if (Get-Command curl.exe -ErrorAction SilentlyContinue) {
            $curlArgs = @("-#", "-fL", "--retry", "3", "--retry-delay", "2")
            if ($token) {
                $curlArgs += @("-H", "Authorization: Bearer $token")
            }
            $curlArgs += @($parquetUrl, "-o", $parquetPath)
            & curl.exe @curlArgs
            if ($LASTEXITCODE -ne 0) {
                throw "curl.exe exited with code $LASTEXITCODE"
            }
        } else {
            $headers = @{}
            if ($token) {
                $headers["Authorization"] = "Bearer $token"
            }
            $prevProgress = $ProgressPreference
            $ProgressPreference = 'SilentlyContinue'
            try {
                Invoke-WebRequest -Uri $parquetUrl -OutFile $parquetPath -Headers $headers
            } finally {
                $ProgressPreference = $prevProgress
            }
        }
        Report "Parquet dataset (embeddings.parquet)" "OK" "Downloaded."
    } catch {
        if (Test-Path $parquetPath) {
            Remove-Item -Path $parquetPath -Force -ErrorAction SilentlyContinue
        }
        Report "Parquet dataset (embeddings.parquet)" "FAILED" "Download failed - verify network or set HF_TOKEN if private."
    }
}

# --- Step 7: parquet + LanceDB sanity checks ---
try {
    cargo run --example testParquet 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "testParquet failed" }
    Report "Parquet schema read (Rust)" "OK"
} catch {
    Report "Parquet schema read (Rust)" "FAILED" $_.Exception.Message
}

if (Test-Path "dbe\lancedb\chunks.lance") {
    Report "LanceDB table" "SKIPPED" "dbe\lancedb\chunks.lance already exists."
} else {
    try {
        cargo run --example convertLanceDB 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "convertLanceDB failed" }
        Report "LanceDB table" "OK" "Created and verified with a sample search."
    } catch {
        Report "LanceDB table" "FAILED" $_.Exception.Message
    }
}

# --- Step 8: llama-server (waits for the user) ---
Write-Host "`n[WAITING] llama-server" -ForegroundColor Cyan
Write-Host "        This step needs YOU to start llama-server manually, in a separate terminal:" -ForegroundColor DarkGray
Write-Host "        llama-server -m models\Llama-3.2-1B-Instruct-Q8_0.gguf -c 4096 --port 8080" -ForegroundColor White
Write-Host "        Once it's running and ready, come back here." -ForegroundColor DarkGray
Read-Host "        Press Enter once llama-server is running"

try {
    cargo run --example testLlamaServer 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "testLlamaServer failed" }
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
