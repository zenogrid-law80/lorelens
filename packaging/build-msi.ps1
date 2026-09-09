param(
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$Manifest = Join-Path $ProjectRoot 'lorelens\Cargo.toml'
$ReleaseDir = Join-Path $ProjectRoot 'target\release'
$ExePath = Join-Path $ReleaseDir 'lorelens.exe'
$Version = [regex]::Match((Get-Content -LiteralPath $Manifest -Raw), '(?m)^version = "([0-9]+\.[0-9]+\.[0-9]+)"').Groups[1].Value
if (-not $Version) { throw 'Package version was not found in Cargo.toml' }
$OutputDir = Join-Path $ProjectRoot 'dist'
$WixTool = Join-Path $ProjectRoot '.tools\wix.exe'

if (-not $SkipBuild) {
    cargo build --release --manifest-path $Manifest
    if ($LASTEXITCODE -ne 0) { throw "Rust build failed" }
}
if (-not (Test-Path $ExePath)) {
    throw "Release executable was not found: $ExePath"
}

if (-not (Test-Path $WixTool)) {
    New-Item -ItemType Directory -Force (Split-Path -Parent $WixTool) | Out-Null
    dotnet tool install wix --version 6.0.2 --tool-path (Split-Path -Parent $WixTool)
}

New-Item -ItemType Directory -Force $OutputDir | Out-Null
$WixSource = Join-Path $PSScriptRoot 'Package.wxs'
$MsiPath = Join-Path $OutputDir "LoreLens-$Version.msi"
& $WixTool build $WixSource -arch x64 -d "ExePath=$ExePath" -d "Version=$Version" -o $MsiPath
if ($LASTEXITCODE -ne 0) { throw "WiX failed with exit code $LASTEXITCODE" }

Write-Host "Created $MsiPath"
