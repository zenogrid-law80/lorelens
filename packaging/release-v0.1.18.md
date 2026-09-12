## What's Changed

- Added a sparse workspace editor with a repository folder tree and folder selection checkboxes, including folders that are not present locally.
- Merge folder selections with the latest view file when saving, preserve custom rules, and compact repeated LoreLens-generated selections into the current configuration.
- Refresh the workspace when leaving the sparse editor, with refresh deferred while another operation is running.
- Validate canonical workspace metadata paths on Windows and reject unsafe view file targets.
- Combined line endings, encoding, and text file extensions into one Text file settings popup menu.
- Made system-following LoreLens Light and LoreLens Dark the default themes.
- Simplified recent repository entries to show local paths, with repository URLs in tooltips.
- Updated English, Korean, and Simplified Chinese UI text and documentation.

## Installation

Download `LoreLens-0.1.18.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This free public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
d11cd851c82aaa5ed699698d31873ed6efb83f18f4628d95eb4db05f1a206aca  LoreLens-0.1.18.msi
```

## Validation and Known Limitations

- Formatting and Clippy checks passed with warnings denied.
- The standard test suite passed: 82 passed and 4 ignored.
- The sparse repository listing integration test also passed separately with a local Lore CLI and an isolated offline workspace.
- The Windows release build and MSI package were generated successfully; the installer ProductVersion is 0.1.18.0.
- Manual UI verification and MSI installation and upgrade testing were not completed.
- Complex custom view rules are preserved; the folder selection display does not fully interpret every custom pattern.
