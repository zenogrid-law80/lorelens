## What's Changed

- Published a free Windows distribution build of LoreLens.
- Added repository and folder bookmarks for faster navigation.
- Improved customizable keyboard shortcuts with conflict validation and shortcut-aware menus.
- Refined file browser selection and navigation behavior.

## Installation

Download `LoreLens-0.1.8.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This free public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
b1260b0b254da070b59dd4f1bc5f911379517b57ddfb39aace16c8cf774b2191  LoreLens-0.1.8.msi
```

## Validation and Known Limitations

- Formatting and Clippy checks passed with warnings denied.
- 43 tests passed; one Lore CLI integration test was excluded because it requires a usable local Lore CLI and test workspace.
- The Windows release build and MSI package were generated. Manual UI testing and MSI installation and upgrade testing were not performed.
