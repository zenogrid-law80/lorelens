## What's Changed

- Added automatic text merging for compatible two-way and three-way conflict inputs, with safe fallback when content cannot be merged.
- Improved conflict resolution with file-name and extension filtering, sorting, keyboard selection, and multi-file side selection.
- Preserved conflict backups until resolution is confirmed and refreshed the conflict queue after repository status updates.
- Increased the Windows main-thread stack to prevent deep GPUI conflict-dialog layouts from crashing the application.
- Improved merge input validation and prevented partial batch overwrites when one selected input is invalid.

## Installation

Download `LoreLens-0.1.23.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
9938524f1fddca1c7c443ab944bd4dc2cef0d1f1a622ee73bee5ae452b14a748  LoreLens-0.1.23.msi
```

## Validation and Known Limitations

- Formatting and Clippy checks passed with warnings denied.
- The standard test suite passed: 113 passed and 5 ignored.
- The Windows release build and MSI package were generated successfully; the installer ProductVersion is 0.1.23.0.
- Manual MSI installation and upgrade testing were not completed.
