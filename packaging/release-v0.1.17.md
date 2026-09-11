## What's Changed

- Expanded branch management with remote branch checkout and confirmed local, remote, or combined branch archiving.
- Made branch switching more resilient when the requested branch is available remotely but not yet available locally.
- Grouped pending changes into collapsible folders and added a folder action for selecting all contained files.
- Added English, Korean, and Simplified Chinese text for the new branch and changes workflows.

## Installation

Download `LoreLens-0.1.17.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This free public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
7c8a82d242c6a0eeb5ef0050eb099b39394d4ed908d05a3c600f755f534124ac  LoreLens-0.1.17.msi
```

## Validation

- Formatting and Clippy checks passed with warnings denied.
- 74 tests passed; three integration tests requiring a usable Lore CLI and local test workspace were excluded.
- The Windows release build and MSI package were generated successfully.
- Manual UI testing and MSI installation and upgrade testing were not performed.
