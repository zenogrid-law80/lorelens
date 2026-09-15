## What's Changed

- Added keyboard navigation to the Changes tree, including selection movement and folder expand/collapse with the arrow keys.
- Replaced the commit message field with a multiline editor that starts at one line and grows with its content.
- Added force folder reset support with `lore reset --force`, including folders that have no current Changes entries.
- Simplified file and folder context menus, removed the duplicate Revert file action, and renamed the View menu entry to Command Log and History.
- Reworked the workspace splitters so Command Log and History span the full window width, and saved all splitter positions plus maximized and fullscreen window state across restarts.
- Added automatic scrolling to the latest Command Log message and tightened the vertical spacing in Changes rows.
- Streamlined the Files panel and added a recursive file filter that includes every parent folder for matching files.
- Updated the website screenshot asset names and references for the light and dark previews.

## Installation

Download `LoreLens-0.1.25.msi` for Windows x64. Close LoreLens before running the installer. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

The installer is not code-signed. Its SHA-256 checksum is also provided in `SHA256SUMS-v0.1.25.txt`:

```text
359c49aab0f4263a19869fa073c0bfbac7de142fbbdcb1e9cfb70dd7dbaba30b  LoreLens-0.1.25.msi
```

## Validation and Known Limitations

- Formatting and Clippy checks passed with warnings denied.
- The standard test suite passed: 118 passed and 5 ignored.
- The website production build passed.
- The Windows release build and MSI package were generated successfully; the executable version is 0.1.25 and the installer ProductVersion is 0.1.25.0.
- Manual MSI installation and upgrade testing were not completed.
