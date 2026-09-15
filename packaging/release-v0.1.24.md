## What's Changed

- Improved text input visibility with stronger borders, theme-aware focus indicators, and consistent sizing across the desktop app.
- Integrated search icons into the file and change filters and adjusted history and path layouts to fit the updated inputs.
- Added light and dark themes to the website and documentation, with system appearance support, a saved theme preference, and localized theme controls.
- Updated the website's application screenshots and favicon, including a matching screenshot for each theme.
- Added the MIT license file and linked license information from the README.

## Installation

Download `LoreLens-0.1.24.msi` for Windows x64. Close LoreLens before running the installer. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

The installer is not code-signed. Its SHA-256 checksum is also provided in `SHA256SUMS-v0.1.24.txt`:

```text
df6ff7d3a6664325b68ec598d296a66249bbe927600cb4141ae234a807f2fd36  LoreLens-0.1.24.msi
```

## Validation and Known Limitations

- Formatting and Clippy checks passed with warnings denied.
- The standard test suite passed: 113 passed and 5 ignored.
- The website build, JavaScript syntax checks, and checks of all 10 local website routes and their assets passed.
- The Windows release build and MSI package were generated successfully; the executable version is 0.1.24 and the installer ProductVersion is 0.1.24.0.
- Manual MSI installation and upgrade testing were not completed.
