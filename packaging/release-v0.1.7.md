## What's Changed

- Added bookmarks for frequently used repositories and folders, with bookmark actions available from navigation and context menus.
- Improved keyboard shortcut handling across menus, navigation, validation, and settings.
- Refined file browser selection and navigation behavior for a more consistent workflow.
- Updated the Windows installer manufacturer metadata to ZenoGrid.

## Installation

Download `LoreLens-0.1.7.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This free public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
d3bf21291063b44653ef0609ed5b1be9325e0a9746e03c474e83d970476d558d  LoreLens-0.1.7.msi
```

## Validation and Known Limitations

- Formatting and Clippy checks passed with warnings denied.
- 43 tests passed; one Lore CLI integration test was excluded because it requires a usable local Lore CLI and test workspace.
- The Windows release build and MSI package were generated. Manual UI testing and MSI installation and upgrade testing were not performed.
