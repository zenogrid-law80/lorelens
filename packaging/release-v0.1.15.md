## What's Changed

- Renamed the duplicate cleanup action to `Reset Files`, with updated English, Korean, and Simplified Chinese labels.
- Moved the action from beside Push to the context menu of duplicate file entries in Changes. Opening it from any duplicate entry still processes all duplicate paths after confirmation.
- Added an always-visible vertical scrollbar on the right side of the Command Log.

## Installation

Download `LoreLens-0.1.15.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This free public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
51a327ad5226be11a4190ba7cd0b338312714c30cd1dc91df4741476d903f78b  LoreLens-0.1.15.msi
```

## Validation

- Formatting and Clippy checks passed; all 62 tests passed, including the two local Lore CLI integration tests.
- The Windows release build and MSI package were generated successfully.
- Manual UI testing, live remote reset workflow testing, and MSI installation and upgrade testing were not performed.
