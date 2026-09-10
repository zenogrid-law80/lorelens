## What's Changed

- Added the LoreLens Light and LoreLens Dark themes with a light default for new installations.
- Refreshed the desktop layout with toolbar icons, clearer panels, improved file browsing, and empty states.
- Added change filtering, staged-file guidance, command-log controls, and automatic repository refresh.
- Added English, Korean, and Simplified Chinese text for the new UI.

## Installation

Download `LoreLens-0.1.12.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This free public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
336c06226487a4e716f0a46718fe9c44c1e6ec638d50831be8070f193cfe6c84  LoreLens-0.1.12.msi
```

## Validation

- Formatting, Clippy, and tests passed with warnings denied where applicable.
- The Windows release build and MSI package were generated successfully.
- Manual UI testing and MSI installation and upgrade testing were not performed.
