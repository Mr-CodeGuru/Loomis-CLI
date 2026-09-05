# scripts/nightly-tree.ps1 — prints project structure for Nightly branch to NightStructure.txt
# Usage: powershell -ExecutionPolicy Bypass -File scripts/nightly-tree.ps1

$rootDir = Split-Path -Parent $PSScriptRoot
if (-not $rootDir) { $rootDir = "." }
$srcDir = Join-Path $rootDir "src"

# Directories that should be ignored at root or transiently (never applied inside src/)
$excludeDirs = @('.git', 'venv', '.venv', 'env', 'target', 'lancedb', 'node_modules', '__pycache__', '.pytest_cache', '.mypy_cache', '.idea', '.vscode', '.cache')
# Large binary/artifact file extensions (never applied inside src/)
$excludeFileExt = @('.gguf', '.bin', '.safetensors', '.pt', '.pth', '.onnx', '.parquet', '.lance', '.DS_Store')
# Directories shown as a placeholder only — contents not recursed into (e.g. models, dbe)
$collapseDirs = @('models', 'dbe')

function Show-Tree {
    param(
        [string]$Path,
        [string]$Indent = ""
    )

    $isInsideSrc = $Path.StartsWith($srcDir, [System.StringComparison]::OrdinalIgnoreCase)

    $items = Get-ChildItem -LiteralPath $Path -Force | Where-Object {
        # Never exclude anything under src/
        if ($isInsideSrc) {
            return $true
        }
        # Filter out transient/build directories
        if ($_.PSIsContainer -and $excludeDirs -contains $_.Name) {
            return $false
        }
        return $true
    } | Sort-Object @{Expression = { -not $_.PSIsContainer }}, Name

    foreach ($item in $items) {
        if ($item.PSIsContainer) {
            Write-Output "$Indent$($item.Name)/"
            # Recurse unless collapsed — src is never collapsed
            if ($isInsideSrc -or $collapseDirs -notcontains $item.Name) {
                Show-Tree -Path $item.FullName -Indent "$Indent  "
            }
        }
        else {
            # Files in src are always kept; otherwise filter out large binary extensions
            if ($isInsideSrc -or $excludeFileExt -notcontains $item.Extension.ToLower()) {
                Write-Output "$Indent$($item.Name)"
            }
        }
    }
}

$outputFile = Join-Path $rootDir "NightStructure.txt"
Show-Tree -Path $rootDir | Out-File -FilePath $outputFile -Encoding utf8

$nightlyDir = Join-Path $rootDir "Nightly"
if (Test-Path $nightlyDir) {
    $nightlyCopy = Join-Path $nightlyDir "NightStructure.txt"
    Copy-Item -Path $outputFile -Destination $nightlyCopy -Force
}

Get-Content $outputFile
