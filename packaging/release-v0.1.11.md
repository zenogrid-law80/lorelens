## What's Changed

- Added incoming-commit counts alongside outgoing commit counts in the Sync and Push controls.
- Split file previews into their own tab while keeping the current Changes or History tab selected when a file is selected.
- Restored the History tab list and added a Clear button for the command log.
- Added English, Korean, and Simplified Chinese text for the new controls.

## Installation

Download `LoreLens-0.1.11.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This free public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
033624469152c28e91a4a8a034bfd63a3b45afd6e47a3f8f0210ebe4ff5ce60e  LoreLens-0.1.11.msi
```

## Validation

- Formatting and Clippy checks passed with warnings denied.
- 52 tests passed; one integration test requiring a local Lore CLI and test workspace was excluded.
- The Windows release build and MSI package were generated successfully.
- Manual UI testing and MSI installation and upgrade testing were not performed.
