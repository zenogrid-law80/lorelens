## Changes

- Converted Pending changes to a virtual list and added an always-visible vertical scrollbar.
- Pending change files can now be selected with Ctrl/Cmd+A, Ctrl/Cmd-click, and Shift-click.
- Added the ability to revert selected files at once, along with a step for choosing whether folder operations include subfolders.
- Strengthened Obliterate by requiring explicit activation followed by confirmation.
- Commands now show a progress dialog and animated progress indicator while running, preventing interaction with the background.

## Downloads

Windows x64: Download and install `LoreLens-0.1.4.msi`. The SHA256 checksum for verification is available in `SHA256SUMS.txt`.

## Verification and Limitations

- Release build and MSI generation completed. All 40 tests passed, with one Lore CLI integration test excluded.
- MSI installation and upgrade testing from previous versions was not performed.
- Because commands do not provide detailed progress, the progress indicator shows that a command is running rather than a completion percentage.
