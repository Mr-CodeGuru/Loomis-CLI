# tree.ps1 — prints project structure: all directories, but skips large/binary files by pattern.
# Usage:  powershell -ExecutionPolicy Bypass -File tree.ps1

$excludeDirs = @('.git', 'venv', '.venv', 'env', 'target', 'node_modules', '__pycache__', '.pytest_cache', '.mypy_cache', '.idea', '.vscode', '.cache')
$excludeFileExt = @('.gguf', '.bin', '.safetensors', '.pt', '.pth', '.onnx', '.parquet', '.lance')
# Directories shown as a placeholder only — contents not recursed into (e.g. HF cache internals)
$collapseDirs = @('models')

function Show-Tree {
    param(
        [string]$Path,
        [string]$Indent = ""
    )

    $items = Get-ChildItem -LiteralPath $Path -Force | Where-Object {
        -not ($_.PSIsContainer -and $excludeDirs -contains $_.Name)
    } | Sort-Object @{Expression = { -not $_.PSIsContainer }}, Name

    foreach ($item in $items) {
        if ($item.PSIsContainer) {
            Write-Output "$Indent$($item.Name)/"
            if ($collapseDirs -notcontains $item.Name) {
                Show-Tree -Path $item.FullName -Indent "$Indent  "
            }
        }
        else {
            if ($excludeFileExt -notcontains $item.Extension.ToLower()) {
                Write-Output "$Indent$($item.Name)"
            }
        }
    }
}

Show-Tree -Path "." | Out-File -FilePath "STRUCTURE.txt" -Encoding utf8
Get-Content "STRUCTURE.txt"