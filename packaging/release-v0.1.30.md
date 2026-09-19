# LoreLens v0.1.30

LoreLens v0.1.30 is a Windows x64 release with improved repository workflows, file previews, history browsing, and a refreshed documentation website.

## Download

Download `LoreLens-0.1.30.msi` below and close LoreLens before running the installer. The installer is not code-signed, so Windows may display an unknown publisher or SmartScreen warning. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

Verify the downloaded installer with the SHA-256 checksum in `SHA256SUMS-v0.1.30.txt`:

```text
6a12ed4ff11af623f51160416fe27aa0265432a7c1591bcf06472de63782136d  LoreLens-0.1.30.msi
```

## Highlights

- Improved file preview and repository browsing workflows, including FBX previews and broader source-language detection.
- More robust synchronization, branching, history, merge, reset, and duplicate-file handling.
- Expanded keyboard shortcut, text-file validation, settings persistence, and localization support.
- Updated the project documentation website.

## Validation

- `cargo fmt --check` passed.
- 124 unit tests passed; 5 integration tests requiring a usable Lore CLI remained ignored.
- Windows release build and MSI packaging completed successfully.
- MSI installation and upgrade testing were not performed.
