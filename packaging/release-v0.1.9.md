## What's Changed

- Added a built-in theme picker with a broad collection of bundled light and dark themes.
- Expanded external diff and merge tool support, including improved executable discovery and configuration.
- Updated dialogs and menus for the current GPUI component APIs and improved confirmation behavior.
- Refined file browser interactions, settings persistence, localization, and website documentation.
- Published a free Windows x64 distribution build of LoreLens.

## Installation

Download `LoreLens-0.1.9.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This free public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
a5f2b3c228839e80661767bbac72e97560353de5b87bd356313cd579a5986e92  LoreLens-0.1.9.msi
```

## Validation and Known Limitations

- Formatting and Clippy checks passed with warnings denied.
- 45 tests passed; one Lore CLI integration test was excluded because it requires a usable local Lore CLI and test workspace.
- The Windows release build and MSI package were generated. Manual UI testing and MSI installation and upgrade testing were not performed.
