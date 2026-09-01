# init-structure.ps1 — reads STRUCTURE.md (as produced by tree.ps1) and recreates
# the directories and empty files on disk. Safe to re-run: skips anything that
# already exists.
#
# Usage:  powershell -ExecutionPolicy Bypass -File init-structure.ps1

$structureFile = "STRUCTURE.md"

if (-not (Test-Path $structureFile)) {
    Write-Error "Could not find $structureFile in current directory."
    exit 1
}

$lines = Get-Content $structureFile | Where-Object { $_ -notmatch '^```' -and $_.Trim() -ne "" }

# Stack tracks the current path at each indent level (2 spaces per level, matching tree.ps1 output)
$stack = @{ 0 = "." }

foreach ($line in $lines) {
    if ($line -notmatch '^\s*') { continue }

    $trimmed = $line.TrimStart(' ')
    $leadingSpaces = $line.Length - $trimmed.Length
    $level = [math]::Floor($leadingSpaces / 2)

    $parentPath = $stack[$level]
    if (-not $parentPath) { $parentPath = "." }

    $fullPath = Join-Path $parentPath $trimmed.TrimEnd('/')

    if ($trimmed.EndsWith('/')) {
        # Directory
        if (-not (Test-Path $fullPath)) {
            New-Item -ItemType Directory -Path $fullPath -Force | Out-Null
            Write-Output "Created dir : $fullPath"
        }
        $stack[$level + 1] = $fullPath
    }
    # Files are skipped intentionally — they come from `git clone`, not this script.
}

Write-Output "`nDone. Structure recreated from $structureFile."