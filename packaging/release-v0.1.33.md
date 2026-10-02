# LoreLens v0.1.33

LoreLens v0.1.33 is a Windows x64 maintenance release that updates the GPUI interface dependencies and adjusts dialog rendering for the updated component framework.

## Download

Download `LoreLens-0.1.33.msi` below and close LoreLens before running the installer. The installer is not code-signed, so Windows may display an unknown publisher or SmartScreen warning. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

Verify the downloaded installer with the SHA-256 checksum in `SHA256SUMS-v0.1.33.txt`:

```text
e3db81e0c4e821a4e5c2837a90a7e49a3b6dbef00cc9ef9e218a39f5b7479533  LoreLens-0.1.33.msi
```

## Highlights

- Update GPUI, GPUI Platform, and GPUI Component dependencies.
- Adjust dialog-layer rendering for the updated interface dependencies.

## Validation

- `cargo fmt --check` passed.
- Windows release build and MSI packaging completed successfully.
- MSI installation and upgrade testing were not performed.
