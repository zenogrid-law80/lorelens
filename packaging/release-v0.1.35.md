# LoreLens v0.1.35

LoreLens v0.1.35 is a Windows x64 maintenance release that adds a translucent Acrylic blur effect to the title bar.

## Download

Download `LoreLens-0.1.35.msi` below and close LoreLens before running the installer. The installer is not code-signed, so Windows may display an unknown publisher or SmartScreen warning. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

Verify the downloaded installer with the SHA-256 checksum in `SHA256SUMS-v0.1.35.txt`:

```text
c15e0bab6ab79e6a2861f7e8aa4285dd47f164553872d1a784109864c801260a  LoreLens-0.1.35.msi
```

## Highlights

- Apply the Windows Acrylic blur and translucency effect to the title bar.
- Keep the native title bar and window controls.

## Validation

- `cargo fmt --check` and `cargo check` passed.
- Windows release build and MSI packaging completed successfully.
- MSI installation and upgrade testing were not performed.
