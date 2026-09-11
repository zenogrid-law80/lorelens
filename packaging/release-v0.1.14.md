## What's Changed

- Added a `Deduplicate Files` action beside Push when Lore reports duplicate entries in Changes.
- Added a confirmation workflow that stages the duplicate paths, commits `Remove unavailable local server binaries`, pushes the commit, refreshes status, and verifies the repository.
- Automatically removes local files that have both `delete`/Staged and `keep`/Unstaged Change states before deduplication.
- Added localized Deduplicate Files labels and workflow guidance in English, Korean, and Simplified Chinese.
- Removed the per-file Changes context action so deduplication is performed from the single repository-level action.

## Installation

Download `LoreLens-0.1.14.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This free public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
1372536244a71054df97ebe25226bdb3fab9a2fc275cc2bbfbff0f217ecd48db  LoreLens-0.1.14.msi
```

## Validation

- Formatting, Clippy, and tests passed with warnings denied where applicable.
- The Windows release build and MSI package were generated successfully.
- Manual UI testing and MSI installation and upgrade testing were not performed.
