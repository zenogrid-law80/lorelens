## What's Changed

- Added a dedicated file history view for repository files, available from the History tab and file context menu.
- Added revision selection with checkboxes, Ctrl/Shift selection, refresh, and incremental history loading.
- Added comparison of two selected revisions in the configured external diff tool, including deleted-file revisions.
- Added English, Korean, and Simplified Chinese text for the file history workflow.

## Installation

Download `LoreLens-0.1.16.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This free public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
94dc9abdc32ae2dcc22d4287a33473fee8a33eaf8dae418fd866222084a41934  LoreLens-0.1.16.msi
```

## Validation

- Formatting and Clippy checks passed with warnings denied.
- 64 tests passed; three integration tests requiring a usable Lore CLI and local test workspace were excluded.
- The Windows release build and MSI package were generated successfully.
- Manual UI testing and MSI installation and upgrade testing were not performed.
