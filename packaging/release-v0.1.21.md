## What's Changed

- Added a Remote History tab to the lower panel with remote branch navigation and 100-commit pages.
- Added a commit grid with aligned Commit message, Author, and Date columns, plus changed-file details for the selected revision.
- Resolved Lore identity IDs through authentication metadata so commit authors display as names or email addresses instead of internal hashes.
- Added Diff and patch export actions for modified files in remote revisions, with revision and path validation.
- Fixed a stack overflow that could crash the app while rendering populated remote history.
- Improved selected-row contrast across the Files, Changes, and history views.

## Installation

Download `LoreLens-0.1.21.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
59dbe17c19578b1b9fd2f7f1a9903034650a8b2412bbd2afe03305f57e42ede2  LoreLens-0.1.21.msi
```

## Validation and Known Limitations

- Formatting and Clippy checks passed with warnings denied.
- The standard test suite passed: 94 passed and 5 ignored.
- The Windows release build and MSI package were generated successfully; the installer ProductVersion is 0.1.21.0.
- Manual MSI installation and upgrade testing were not completed.
