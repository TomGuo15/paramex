Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path
$SourceExe = Join-Path $RepoRoot "target\release\paramex-gui.exe"
$DistDir = Join-Path $RepoRoot "target\dist\ParamEx"
$DestExe = Join-Path $DistDir "ParamEx.exe"

function Assert-InRepo {
    param([Parameter(Mandatory = $true)][string]$Path)

    $full = [IO.Path]::GetFullPath($Path)
    $rootWithSlash = $RepoRoot.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if (-not $full.StartsWith($rootWithSlash, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to touch path outside repository: $full"
    }
}

Assert-InRepo -Path $DistDir

Push-Location $RepoRoot
try {
    cargo build --release -p paramex-gui
}
finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $SourceExe)) {
    throw "Expected Rust release binary was not created: $SourceExe"
}

if (Test-Path -LiteralPath $DistDir) {
    Remove-Item -LiteralPath $DistDir -Recurse -Force
}

New-Item -ItemType Directory -Path $DistDir | Out-Null
Copy-Item -LiteralPath $SourceExe -Destination $DestExe -Force
$packagedAt = Get-Date

$source = Get-Item -LiteralPath $SourceExe
$dest = Get-Item -LiteralPath $DestExe
if ($source.Length -ne $dest.Length) {
    throw "Copied EXE size mismatch: source=$($source.Length), dest=$($dest.Length)"
}
$dest.LastWriteTime = $packagedAt

# Bundle the project license alongside the exe. The release workflow generates
# dependency notices directly into this directory before creating the ZIP.
$license = Join-Path $RepoRoot "LICENSE"
if (-not (Test-Path -LiteralPath $license)) {
    throw "Expected distribution file is missing: $license"
}
Copy-Item -LiteralPath $license -Destination (Join-Path $DistDir "LICENSE") -Force

Write-Host "Wrote $DestExe (+ LICENSE)"
