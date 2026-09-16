## What's Changed

- Fixed a false login failure: LoreLens now uses the account returned by `lore login` instead of performing a second account lookup without an authentication endpoint. This also avoids ambiguity from other stored Lore CLI accounts.
- Run `lore status --scan` when opening a repository and on refresh so changes made outside LoreLens appear in the Changes panel.
- Added All, Staged, and Unstaged filters to the Changes panel.
- Simplified button borders and refreshed the website's light and dark screenshots.

## Installation

Download `LoreLens-0.1.26.msi` for Windows x64. Close LoreLens before running the installer. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

The installer is not code-signed. Its SHA-256 checksum is also provided in `SHA256SUMS-v0.1.26.txt`:

```text
05b2510e09c3e35d1a2eecd86e2b31ed16cbfb53879c051bafb7295fe4ccedbb  LoreLens-0.1.26.msi
```

## Validation and Known Limitations

- Formatting and Clippy checks passed with warnings denied.
- The standard test suite passed: 116 passed and 5 ignored.
- The website production build passed.
- The Windows release build and MSI package were generated successfully; the executable version is 0.1.26 and the installer ProductVersion is 0.1.26.0.
- WiX MSI validation passed with an ICE61 warning because the installer permits same-version upgrades.
- Manual MSI installation and upgrade testing were not completed.
