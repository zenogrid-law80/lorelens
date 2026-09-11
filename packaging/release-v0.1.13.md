## What's Changed

- Added Reset actions for individual files, folders, and multi-file selections.
- Improved reset handling for staged deletions and missing paths, including Windows path normalization.
- Added a compatibility fallback for obliterating deleted files when Lore requires an address instead of a path.
- Improved command logging and repository refresh behavior after reset operations.
- Updated the user guide and localized Reset text in English, Korean, and Simplified Chinese.

## Installation

Download `LoreLens-0.1.13.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This free public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
ee78a76af3f1752d4125c4cdb1d590861b053bef26a811907685c264d17e0b56  LoreLens-0.1.13.msi
```

## Validation

- Formatting, Clippy, and tests passed with warnings denied where applicable.
- The Windows release build and MSI package were generated successfully.
- Manual UI testing and MSI installation and upgrade testing were not performed.
