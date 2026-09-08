param(
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$Manifest = Join-Path $ProjectRoot 'lorelens\Cargo.toml'
$ReleaseDir = Join-Path $ProjectRoot 'target\release'
$ExePath = Join-Path $ReleaseDir 'lorelens.exe'
$LoreCliPath = Join-Path $ProjectRoot 'lorelens\dist\lore.exe'
$OutputDir = Join-Path $ProjectRoot 'dist'
$WixTool = Join-Path $ProjectRoot '.tools\wix.exe'

if (-not $SkipBuild) {
    cargo build --release --manifest-path $Manifest
    if ($LASTEXITCODE -ne 0) { throw "Rust build failed" }
}
if (-not (Test-Path $ExePath)) {
    throw "Release executable was not found: $ExePath"
}
if (-not (Test-Path -LiteralPath $LoreCliPath -PathType Leaf)) {
    throw "Bundled Lore CLI was not found: $LoreCliPath"
}

if (-not (Test-Path $WixTool)) {
    New-Item -ItemType Directory -Force (Split-Path -Parent $WixTool) | Out-Null
    dotnet tool install wix --version 6.0.2 --tool-path (Split-Path -Parent $WixTool)
}

New-Item -ItemType Directory -Force $OutputDir | Out-Null
$WixSource = Join-Path $PSScriptRoot 'Package.wxs'
$MsiPath = Join-Path $OutputDir 'LoreLens-0.1.2.msi'
& $WixTool build $WixSource -d "ExePath=$ExePath" -d "LoreCliPath=$LoreCliPath" -o $MsiPath
if ($LASTEXITCODE -ne 0) { throw "WiX failed with exit code $LASTEXITCODE" }

Write-Host "Created $MsiPath"
