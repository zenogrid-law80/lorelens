# LoreLens v0.1.31

LoreLens v0.1.31 is a Windows x64 maintenance release focused on safer reset handling, legacy repository protection, and more reliable asynchronous file loading.

## Download

Download `LoreLens-0.1.31.msi` below and close LoreLens before running the installer. The installer is not code-signed, so Windows may display an unknown publisher or SmartScreen warning. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

Verify the downloaded installer with the SHA-256 checksum in `SHA256SUMS-v0.1.31.txt`:

```text
d93e2bb5d13c06ec02c2a80676827a653056025f1020ccef84b837c4b3956225  LoreLens-0.1.31.msi
```

## Highlights

- Preserve restored files when a reset command fails or times out after writing them.
- Clean up only temporary reset placeholders when preparation fails before reset execution.
- Protect legacy `.urc` repository metadata in move operations alongside `.lore` and `.git` metadata.
- Keep startup connection handling reliable when newer asynchronous file searches finish out of order.

## Validation

- `cargo fmt --check` passed.
- 128 unit tests passed; 5 integration tests requiring a usable Lore CLI remained ignored.
- Windows release build and MSI packaging completed successfully.
- MSI installation and upgrade testing were not performed.
