## What's Changed

- Expanded the Changes context menus to include the applicable file and folder actions available from the Files panel, including history, diff or conflict resolution, staging, revert, move, path utilities, bookmarks, locking, deletion, obliteration, and reset.
- Added multi-selection support for copying full paths and deleting eligible files from the Changes panel.
- Added Diff directly to unstaged Changes entries and revalidated the repository state before launching the configured external tool.
- Hid Delete for staged existing-file modifications in both the Files and Changes panels, including folder and multi-selection targets.
- Blocked the Delete keyboard shortcut when its targets contain a staged existing-file modification, preventing staged content from being left behind with a new unstaged deletion.
- Kept filesystem-oriented context actions available when Lore operations are temporarily unavailable, while disabling actions that require a connected repository.

## Installation

Download `LoreLens-0.1.20.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
cc393fc131c8c5e9f39c8c14a593de9a83e3a21c96987c7132d970e8d8ffdf2d  LoreLens-0.1.20.msi
```

## Validation and Known Limitations

- Formatting and Clippy checks passed with warnings denied.
- The standard test suite passed: 85 passed and 4 ignored.
- The Windows release build and MSI package were generated successfully; the installer ProductVersion is 0.1.20.0.
- Manual UI verification and MSI installation and upgrade testing were not completed.
