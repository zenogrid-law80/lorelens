# LoreLens v0.1.32

LoreLens v0.1.32 is a Windows x64 maintenance release with improved text-file line-ending workflows and clearer folder operation dialogs.

## Download

Download `LoreLens-0.1.32.msi` below and close LoreLens before running the installer. The installer is not code-signed, so Windows may display an unknown publisher or SmartScreen warning. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

Verify the downloaded installer with the SHA-256 checksum in `SHA256SUMS-v0.1.32.txt`:

```text
fcf345a7b25a42c43fae948b9ff00ef44a036c1a9eb21cb8a15b895578661350  LoreLens-0.1.32.msi
```

## Highlights

- Convert selected text files to the configured line ending from their context menu.
- Improve folder operation dialogs so long selected paths remain readable.
- Improve error reporting for text-file encoding and line-ending validation.

## Validation

- `cargo fmt --check`, Clippy, targeted tests, and the release build passed.
- Windows x64 MSI packaging completed successfully.
- MSI installation and upgrade testing were not performed.
