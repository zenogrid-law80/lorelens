## What's Changed

- Renamed the lower-panel Remote History tab to History.
- Added separate HEAD, Local, and Remote sections to the history branch navigator.
- Changed HEAD history to start from the checked-out local revision, so unpushed local commits are included.
- Added local branch history browsing without checking out the selected branch.
- Kept remote branch browsing explicitly scoped to remote history.
- Updated history loading, empty-state, and error messages for both local and remote sources.

## Installation

Download `LoreLens-0.1.22.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
e33246d6ef75c7dca2478ebba8f273f3da566e7e7becdc4357ea2f76749b1b2a  LoreLens-0.1.22.msi
```

## Validation and Known Limitations

- Formatting and Clippy checks passed with warnings denied.
- The standard test suite passed: 93 passed and 5 ignored.
- The Windows release build and MSI package were generated successfully; the installer ProductVersion is 0.1.22.0.
- Manual MSI installation and upgrade testing were not completed.
