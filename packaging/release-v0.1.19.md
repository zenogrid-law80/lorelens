## What's Changed

- Fixed the post-login flow so LoreLens selects the authenticated identity matching the login server, updates the repository identity, and only then refreshes repository status.
- Isolated global authentication queries from the current repository so a stale repository identity cannot interfere with account selection.
- Recognized Lore CLI identities with an expiration value of zero as non-expiring sessions.
- Added clear authentication-specific status and error messages when account connection fails.
- Required the secure `lores://` scheme in login, repository creation, and clone dialogs, preventing invalid `lore://` URLs before invoking the CLI.
- Updated English, Korean, and Simplified Chinese translations and authentication documentation.

## Installation

Download `LoreLens-0.1.19.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

This public build is not code-signed, so Windows may display an unknown publisher or Microsoft Defender SmartScreen warning. Download the installer only from this GitHub release and verify its SHA-256 checksum before running it:

```text
134b52466ae4614f2ad33e82c5682da6eaf1f22140afd3136556cc170412dca7  LoreLens-0.1.19.msi
```

## Validation and Known Limitations

- Formatting and Clippy checks passed with warnings denied.
- The standard test suite passed: 85 passed and 4 ignored.
- The Windows release build and MSI package were generated successfully; the installer ProductVersion is 0.1.19.0.
- The authentication flow has unit coverage for server matching, ambiguous identities, non-expiring sessions, and URL scheme validation.
- Manual UI verification and MSI installation and upgrade testing were not completed.
