# LoreLens v0.1.38

LoreLens v0.1.38 adds a dedicated locked-file view and updates the GPUI dependencies.

## Highlights

- Add a **Locked** filter to Changes, including locked files without local modifications. Show lock status alongside staged, unstaged, and conflict states.
- Keep folder selection, keyboard navigation, select-all, search, file counts, and empty-state messages consistent with the active filter.
- Deduplicate locked paths across slash styles and, on Windows, ASCII letter case. Preserve repository status indices so display-only locked entries are not mistaken for staged, unstaged, or conflicted changes.
- Add English, Korean, and Simplified Chinese text for the locked-file view.
- Update GPUI and GPUI Platform to 0.3.10, and GPUI Component and Assets to 0.8.1. Migrate scroll handlers to the updated GPUI API.

## Download

| Platform | Installer |
| --- | --- |
| Windows x64 | `LoreLens-0.1.38.msi` |
| macOS Apple Silicon | `LoreLens-0.1.38-macos-arm64.dmg` |
| macOS Intel | `LoreLens-0.1.38-macos-x86_64.dmg` |

Verify downloaded installers against `SHA256SUMS-v0.1.38.txt` below. Close LoreLens before installing or upgrading on Windows. The MSI is not code-signed; the macOS apps use ad-hoc signing and are not Developer ID signed or notarized.

Install the Lore CLI separately and connect it using **Locate CLI**, `LORELENS_LORE_BIN`, or `PATH`. These installers do not bundle the CLI.

## Validation

- Rust formatting and Clippy with warnings denied passed.
- Unit tests: 142 passed, 5 ignored. This includes regression coverage for locked-file display, unchanged locks, path normalization, and preserved change indices.
- English, Korean, and Simplified Chinese catalogs have matching keys, and the new interpolation placeholders were verified.
- Windows and macOS workflow validation with actionlint passed.
- [Windows x64 release build and MSI packaging](https://github.com/zenogrid-law80/lorelens/actions/runs/40433085818) passed in GitHub Actions. MSI product version `0.1.38.0`, x64 architecture, and the existing upgrade identity were verified.
- [Apple Silicon and Intel macOS builds](https://github.com/zenogrid-law80/lorelens/actions/runs/40433085848) passed, including DMG integrity, executable architecture, and strict app signature checks.
- All downloaded installers matched their CI checksum manifests and were built from committed app sources matching the `v0.1.38` tag.
- Installer execution and GUI smoke tests were not performed.
