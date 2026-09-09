$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
& winget install --exact --id EpicGames.Lore --accept-source-agreements --accept-package-agreements --disable-interactivity
if ($LASTEXITCODE -ne 0) { throw "winget installation failed (exit code $LASTEXITCODE)." }

# Reload PATH after installation; the running app still has its old environment.
$env:PATH = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
$loreCommand = Get-Command lore.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
$lorePath = if ($loreCommand) { $loreCommand.Source } else { $null }
if (-not $lorePath) {
    $linkPaths = @(
        "$env:LOCALAPPDATA\Microsoft\WinGet\Links\lore.exe",
        "$env:ProgramFiles\WinGet\Links\lore.exe"
    )
    $lorePath = $linkPaths | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
}
if (-not $lorePath) { throw 'Installation completed, but lore.exe could not be located. Use Locate CLI to select it.' }
[Environment]::SetEnvironmentVariable('LORELENS_LORE_BIN', $lorePath, 'User')
Write-Output "LORELENS_INSTALLED_CLI=$lorePath"
