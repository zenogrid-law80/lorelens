param(
    [switch]$Silent
)

$ErrorActionPreference = 'Stop'
$Repository = 'zenogrid-law80/lorelens'
$ApiUrl = "https://api.github.com/repos/$Repository/releases/latest"
$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) 'LoreLens-install'
$MsiPath = Join-Path $TempDir 'LoreLens-latest.msi'

New-Item -ItemType Directory -Force $TempDir | Out-Null
$release = Invoke-RestMethod -Uri $ApiUrl -Headers @{ 'User-Agent' = 'LoreLens-installer' }
if ($release.draft -or $release.prerelease) {
    throw "The latest GitHub release is not a stable release: $($release.tag_name)"
}

$asset = @($release.assets) | Where-Object { $_.name -match '^LoreLens-[0-9]+\.[0-9]+\.[0-9]+\.msi$' } | Select-Object -First 1
if (-not $asset) { throw "No LoreLens MSI was found in release $($release.tag_name)." }

Write-Host "Downloading LoreLens $($release.tag_name)..."
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $MsiPath

$checksumAsset = @($release.assets) | Where-Object { $_.name -eq "SHA256SUMS-$($release.tag_name).txt" } | Select-Object -First 1
if ($checksumAsset) {
    $checksumPath = Join-Path $TempDir 'SHA256SUMS.txt'
    Invoke-WebRequest -Uri $checksumAsset.browser_download_url -OutFile $checksumPath
    $expected = ((Get-Content -LiteralPath $checksumPath) | Where-Object { $_ -match [regex]::Escape($asset.name) } | Select-Object -First 1) -replace '\s+.*$', ''
    $actual = (Get-FileHash -LiteralPath $MsiPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($expected -and $actual -ne $expected.ToLowerInvariant()) {
        throw "Checksum verification failed for $($asset.name)."
    }
}

$arguments = @('/i', $MsiPath)
if ($Silent) { $arguments += @('/qn', '/norestart') }
Write-Host "Installing LoreLens $($release.tag_name)..."
$process = Start-Process -FilePath 'msiexec.exe' -ArgumentList $arguments -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "MSI installation failed with exit code $($process.ExitCode)." }
Write-Host "LoreLens $($release.tag_name) installed successfully."
