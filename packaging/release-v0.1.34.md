# LoreLens v0.1.34

LoreLens v0.1.34 is a Windows x64 release that adds in-app update checks and verified MSI downloads.

## Download

Download `LoreLens-0.1.34.msi` below and close LoreLens before running the installer. The installer is not code-signed, so Windows may display an unknown publisher or SmartScreen warning. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

Verify the downloaded installer with the SHA-256 checksum in `SHA256SUMS-v0.1.34.txt`:

```text
e72939d15a42b14ba2e565b9a014fc8b604fc77b5ac0dcf60ad8a7de9ed42794  LoreLens-0.1.34.msi
```

## Highlights

- Check for stable LoreLens releases from the Options menu and view recent release notes.
- Download the Windows installer in app and verify its SHA-256 checksum before launching it.
- Choose whether LoreLens checks for updates automatically at startup.
- Update the GPUI interface dependencies.

## Validation

- `cargo fmt --check`, Clippy, and tests passed.
- Windows release build and MSI packaging completed successfully.
- MSI installation and upgrade testing were not performed.
