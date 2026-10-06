# LoreLens v0.1.36

LoreLens v0.1.36 is a Windows x64 maintenance release with a gold application icon, improved MSI upgrade handling, and updated dependencies.

## Highlights

- Match the application icon to LoreLens's gold accent color (`#F5C45E`), with a transparent background and seven sizes from 16 to 256 pixels.
- Prompt when LoreLens is running before an MSI installation or upgrade replaces files. Setup can request a normal application shutdown and stops if LoreLens remains open after 15 seconds.
- Upgrade GPUI to 0.3.8, GPUI Component and Assets to 0.7.1, calamine to 0.36.1, reqwest to 0.13.5, and sha2 to 0.11.0.
- Adapt SHA-256 checksum formatting to sha2 0.11 while preserving lowercase hexadecimal output and leading zeroes. Add a known-digest regression test.
- Remove the unused proc-macro-error2 patch and vendored sources.

## Download

Download `LoreLens-0.1.36.msi` below for Windows x64. Finish any active operations and close LoreLens before running the installer. If setup detects a running instance, select **Retry** after closing it, **Ignore** to request a normal shutdown, or **Abort** to cancel.

The installer is not code-signed, so Windows may display an unknown publisher or SmartScreen warning. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with **Locate CLI**.

Verify the downloaded installer with the SHA-256 checksum in `SHA256SUMS-v0.1.36.txt`:

```text
45eff612988734feb73fa465319b27e11b181605e52f7693339030f89c59e6b9  LoreLens-0.1.36.msi
```

## Validation

- Formatting and Clippy checks passed.
- Unit tests: 139 passed, 5 ignored.
- Windows release build and MSI packaging completed successfully.
- MSI product version, packaged executable size, and application-close action sequencing verified.
- MSI installation and upgrade testing were not performed.
