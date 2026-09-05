# scripts/nightly-init-structure.ps1 — reads NightStructure.txt and recreates directories on disk.
# Usage: powershell -ExecutionPolicy Bypass -File scripts/nightly-init-structure.ps1

$rootDir = Split-Path -Parent $PSScriptRoot
if (-not $rootDir) { $rootDir = "." }
$srcDir = Join-Path $rootDir "src"

$structureFile = Join-Path $rootDir "NightStructure.txt"
if (-not (Test-Path $structureFile)) {
    $structureFile = Join-Path (Join-Path $rootDir "Nightly") "NightStructure.txt"
}
if (-not (Test-Path $structureFile)) {
    $structureFile = "NightStructure.txt"
}

if (-not (Test-Path $structureFile)) {
    Write-Error "Could not find NightStructure.txt."
    exit 1
}

# Directories to exclude from recreation if they somehow appear in NightStructure.txt (never applied inside src/)
$excludeDirs = @('.git', 'target', 'node_modules', '.venv', 'venv', '__pycache__', '.cache')

$lines = Get-Content $structureFile | Where-Object { $_ -notmatch '^```' -and $_.Trim() -ne "" }

$stack = @{ 0 = $rootDir }

foreach ($line in $lines) {
    if ($line -notmatch '^\s*') { continue }

    $trimmed = $line.TrimStart(' ')
    $leadingSpaces = $line.Length - $trimmed.Length
    $level = [math]::Floor($leadingSpaces / 2)

    $parentPath = $stack[$level]
    if (-not $parentPath) { $parentPath = $rootDir }

    $dirName = $trimmed.TrimEnd('/')
    $fullPath = Join-Path $parentPath $dirName

    if ($trimmed.EndsWith('/')) {
        # Never exclude anything under src/
        $isInsideSrc = $fullPath.StartsWith($srcDir, [System.StringComparison]::OrdinalIgnoreCase)
        if (-not $isInsideSrc -and $excludeDirs -contains $dirName) {
            continue
        }

        # Directory
        if (-not (Test-Path $fullPath)) {
            New-Item -ItemType Directory -Path $fullPath -Force | Out-Null
            Write-Output "Created dir : $fullPath"
        }
        $stack[$level + 1] = $fullPath
    }
}

Write-Output "`nDone. Structure recreated from $structureFile."
