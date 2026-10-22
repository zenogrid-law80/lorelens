# LoreLens v0.1.37

LoreLens v0.1.37 adds automated macOS builds for Apple Silicon and Intel.

## Highlights

- Build native release apps and DMG installation images on macOS 15 for Apple Silicon (`arm64`) and Intel (`x86_64`).
- Run the macOS workflow on relevant `main` pushes and pull requests, `v*` tags, or manual dispatch from GitHub Actions.
- Verify the app architecture and ad-hoc signature, and upload each DMG with its SHA-256 checksum as an Actions artifact retained for 14 days.
- Allow macOS packaging without a bundled Lore CLI. Local browsing and previews remain available; connect an installed CLI using **Locate CLI**, `LORELENS_LORE_BIN`, or `PATH` for VCS operations.

## Build artifacts

Download the `LoreLens-macos-arm64` or `LoreLens-macos-x86_64` artifact from a successful **Build macOS** workflow run. The workflow does not publish GitHub Release assets automatically. Developer ID signing and notarization are separate from the ad-hoc signing used by these builds.

## Validation

- Workflow validation with actionlint and YAML configuration checks passed.
- Bash syntax checks passed for both macOS packaging scripts and all workflow commands.
