## Changes

- Added an Obliterate menu to the file list and pending changes list. The target and impact must be reviewed, and `OBLITERATE` must be entered before it can run.
- Added the ability to copy the current path to the clipboard.
- Added WinGet installation guidance when the Lore CLI is unavailable on Windows. The CLI is not included in this MSI.
- Explicitly set the Windows MSI architecture to x64 and linked the installer version to Cargo.toml.

## Downloads

Windows x64: Download and install `LoreLens-0.1.3.msi`. The SHA256 checksum for verification is available in `SHA256SUMS.txt`.

If you need the Lore CLI, follow the installation guidance in the app or select a separately installed CLI with `Locate CLI…`.

## Verification and Limitations

- Release build and MSI generation completed. All 32 tests passed, with one integration test against an actual Lore repository excluded.
- MSI installation and upgrade testing was not performed. This release does not include a macOS installer.
- Obliterate permanently removes stored content and may affect remote repositories. It does not remove all historical versions at once; Commit/Push must be performed separately.
- Clone failures caused by `Address not found` and missing content in the Lore CLI are not fixed in this release.
