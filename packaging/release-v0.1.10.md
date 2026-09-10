## What's Changed

- Added a binary merge selection dialog when a conflicted file has `~base` and `~theirs` backups but no `~mine` backup.
- Labeled the choices as **Mine (Base)** and **Remote (Theirs)**, with English, Korean, and Simplified Chinese translations.
- Replace the working file with the selected version and remove merge backups after Lore confirms successful resolution. Canceling leaves the files unchanged.
- Added configurable text file extensions, encoding, and line-ending rules, with validation before staging that leaves file contents unchanged.
- Updated text settings tests to include the default `txt` and `ini` extensions.

## Installation

Download `LoreLens-0.1.10.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

SHA-256:

```text
d9da5aa83df9cb3770ed533ee025532c7336af3e8970018fe3baf8b5acd57d49  LoreLens-0.1.10.msi
```

## Validation

- Formatting and Clippy checks passed with warnings denied.
- 50 tests passed; one integration test requiring a local Lore CLI and test workspace was excluded.
- The Windows release build and MSI package were generated successfully.
- Manual UI testing and MSI installation and upgrade testing were not performed.
