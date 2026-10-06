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
$WixVersion = '6.0.2'
$UtilExtension = Join-Path $ProjectRoot ".tools\.wix\extensions\WixToolset.Util.wixext\$WixVersion\wixext6\WixToolset.Util.wixext.dll"

if (-not $SkipBuild) {
    cargo build --release --manifest-path $Manifest
    if ($LASTEXITCODE -ne 0) { throw "Rust build failed" }
}
if (-not (Test-Path $ExePath)) {
    throw "Release executable was not found: $ExePath"
}

if (-not (Test-Path $WixTool)) {
    New-Item -ItemType Directory -Force (Split-Path -Parent $WixTool) | Out-Null
    dotnet tool install wix --version $WixVersion --tool-path (Split-Path -Parent $WixTool)
    if ($LASTEXITCODE -ne 0) { throw "WiX installation failed with exit code $LASTEXITCODE" }
}

if (-not (Test-Path -LiteralPath $UtilExtension)) {
    # Keep the versioned extension cache with the project's other build tools.
    Push-Location (Split-Path -Parent $WixTool)
    try {
        & $WixTool extension add "WixToolset.Util.wixext/$WixVersion"
        if ($LASTEXITCODE -ne 0) { throw "WiX Util extension installation failed with exit code $LASTEXITCODE" }
    } finally {
        Pop-Location
    }
}

New-Item -ItemType Directory -Force $OutputDir | Out-Null
$WixSource = Join-Path $PSScriptRoot 'Package.wxs'
$MsiPath = Join-Path $OutputDir "LoreLens-$Version.msi"
& $WixTool build $WixSource -ext $UtilExtension -arch x64 -d "ExePath=$ExePath" -d "Version=$Version" -o $MsiPath
if ($LASTEXITCODE -ne 0) { throw "WiX failed with exit code $LASTEXITCODE" }

Write-Host "Created $MsiPath"
