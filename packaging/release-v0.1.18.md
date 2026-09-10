## What's Changed

- Published a free Windows distribution build of LoreLens.
- Includes repository and folder bookmarks for faster navigation.
- Includes customizable keyboard shortcuts with conflict validation and shortcut-aware menus.
- Includes the latest file browser selection and navigation improvements.

## Installation

Download `LoreLens-0.1.18.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This free public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
f18a87473bee23a34163e9b87b220633d660ca9bd21d16bed03545abe616028d  LoreLens-0.1.18.msi
```

## Validation and Known Limitations

- Formatting and Clippy checks passed with warnings denied.
- 43 tests passed; one Lore CLI integration test was excluded because it requires a usable local Lore CLI and test workspace.
- The Windows release build and MSI package were generated. Manual UI testing and MSI installation and upgrade testing were not performed.
