# LoreLens v0.1.37

LoreLens v0.1.37 adds automated Windows and macOS packaging in GitHub Actions.

## Highlights

- Build native release apps and DMG installation images on macOS 15 for Apple Silicon (`arm64`) and Intel (`x86_64`).
- Build Windows x64 release apps and MSI installers using MSVC and WiX 6.0.2.
- Run the platform workflows on relevant `main` pushes and pull requests, `v*` tags, or manual dispatch from GitHub Actions.
- Verify the app architecture and ad-hoc signature, and upload each DMG with its SHA-256 checksum as an Actions artifact retained for 14 days.
- Allow macOS packaging without a bundled Lore CLI. Local browsing and previews remain available; connect an installed CLI using **Locate CLI**, `LORELENS_LORE_BIN`, or `PATH` for VCS operations.

## Download

| Platform | Installer |
| --- | --- |
| Windows x64 | `LoreLens-0.1.37.msi` |
| macOS Apple Silicon | `LoreLens-0.1.37-macos-arm64.dmg` |
| macOS Intel | `LoreLens-0.1.37-macos-x86_64.dmg` |

Verify downloaded installers against `SHA256SUMS-v0.1.37.txt` below. Close LoreLens before installing or upgrading on Windows. The MSI is not code-signed; the macOS apps use ad-hoc signing and are not Developer ID signed or notarized.

Install the Lore CLI separately and connect it using **Locate CLI**, `LORELENS_LORE_BIN`, or `PATH`. These installers do not bundle the CLI.

## Validation

- Workflow validation with actionlint and YAML configuration checks passed.
- Bash syntax checks passed for both macOS packaging scripts and all workflow commands.
- PowerShell syntax checks passed for the Windows workflow and MSI packaging script.
- [Windows x64 release build and MSI packaging](https://github.com/zenogrid-law80/lorelens/actions/runs/39748234942) passed in GitHub Actions. MSI product version `0.1.37.0`, x64 architecture, upgrade identity, and the artifact SHA-256 checksum were verified.
- [Apple Silicon and Intel macOS builds](https://github.com/zenogrid-law80/lorelens/actions/runs/39761967965) passed, including DMG integrity, executable architecture, and strict app signature checks.
- All installers were built from committed app sources matching the `v0.1.37` tag. Downloaded artifacts were checked against their CI checksum manifests.
- Installer execution and GUI smoke tests were not performed.
