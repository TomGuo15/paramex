[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [switch]$BuildOutputs,
    [switch]$VirtualEnv,
    [switch]$AgentState
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$pathComparison = [StringComparison]::OrdinalIgnoreCase

function Resolve-RepoPath {
    param([string]$RelativePath)

    $target = Join-Path $repo $RelativePath
    if (-not (Test-Path -LiteralPath $target)) {
        return $null
    }

    $resolved = (Resolve-Path -LiteralPath $target).Path
    $repoPrefix = $repo + [IO.Path]::DirectorySeparatorChar
    if ($resolved -ne $repo -and -not $resolved.StartsWith($repoPrefix, $pathComparison)) {
        throw "Refusing to remove path outside repository: $resolved"
    }

    return $resolved
}

function Remove-LocalArtifact {
    param([string]$RelativePath)

    $resolved = Resolve-RepoPath $RelativePath
    if ($null -eq $resolved) {
        return
    }

    if ($PSCmdlet.ShouldProcess($resolved, "Remove local generated artifact")) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}

$paths = @(
    "__pycache__",
    ".pytest_cache",
    ".ruff_cache",
    ".pyright",
    ".playwright-mcp",
    ".superpowers",
    "dist",
    "outputs",
    "python_original_empty.png"
)

if ($BuildOutputs) {
    $paths += @(
        "target",
        "crates/paramex-gui/target",
        "build"
    )
}

if ($VirtualEnv) {
    $paths += ".venv"
}

if ($AgentState) {
    $paths += ".claude"
}

foreach ($path in $paths) {
    Remove-LocalArtifact $path
}
