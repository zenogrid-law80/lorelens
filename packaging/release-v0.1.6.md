## What's Changed

- Added customizable keyboard shortcuts for repository, file, branch, theme, and account actions, including localized shortcut settings and conflict validation.
- Added an unpushed commits view and safer discard handling, with clearer refresh behavior after repository operations.
- Improved merge conflict handling and validation for merge inputs.
- Made repository creation, cloning, and local file operations more reliable with stronger path and destination validation.
- Refined the bundled light and dark theme colors.

## Installation

Download `LoreLens-0.1.6.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

## Validation and Known Limitations

- Formatting and Clippy checks passed with warnings denied.
- 41 tests passed; one Lore CLI integration test was excluded because it requires a usable local Lore CLI and test workspace.
- The Windows release build and MSI package were generated. Manual UI testing and MSI installation and upgrade testing were not performed.
