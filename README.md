# LoreLens

A local Lore VCS desktop client for Windows, built with Rust and GPUI. Its workspace, pending changes, submitted revisions, and details layout is inspired by P4V.

## Running

Requires stable Rust and the Visual Studio C++ Build Tools / Windows SDK.

```powershell
cd C:\GitHub\lorelens
cargo run -- C:\GitHub\lore
```

The built application is `target\debug\lorelens.exe`. You can pass a folder as a command-line argument or choose one with **Open repository…**. Local file browsing and preview work without the Lore CLI.

## Connecting Lore

The implementation follows the source and CLI event definitions in `C:\GitHub\lore`. This repository is not itself a Lore workspace; existing Lore workspaces and the Lore CLI are required for VCS operations.

```powershell
cd C:\GitHub\lore
cargo build -p lore-client --bin lore
```

The app looks for the Lore CLI in this order: `LORELENS_LORE_BIN`, the bundled platform path (`dist/lore.exe` on Windows and `dis/lore` on macOS), and then `lore` on `PATH`. You can also choose it with **Locate CLI…**. After opening a workspace, **Refresh** displays the result of `lore status --scan --json`.

## Usage

Right-click a file or folder in the left panel and choose `Move…`. Check the source and enter a destination path including the new name. Relative paths are resolved from the repository root. Moving outside the repository and overwriting an existing destination are rejected. Refresh after moving to detect the change.

Use `↑`/`↓` to select an item, `→` to enter a selected folder, and `←` to move to its parent. The repository root has no parent within the workspace.

The top **Push** button sends local commits on the current branch to the remote using the signed-in account (`lore push`). The status is refreshed afterward, and results or errors appear in Details.

Double-clicking a file opens it with the operating system's default application. A single click shows a preview. Double-clicking a folder opens it inside LoreLens.

Use **Clone repository…** to enter a repository URL and absolute destination path. The parent folder must exist, and the destination must be new or empty. On success, the cloned repository opens automatically. The most recent URL and destination values are restored from local settings.

The **Account** button displays the account used by the current workspace (`lore auth info`). Authentication tokens are never displayed. **Sync** runs `lore sync` and refreshes the file list, change status, lock indicators, and branch information. `--reset` and `--force` are not used.

Use `Ctrl` (or `Cmd` on macOS) plus click to add or remove files and folders from the selection. Use `Shift` plus click to select a range. Stage, Unstage, and Diff apply to the selected items; context-menu actions apply to the item that was right-clicked.

Choose `System`, `Light`, or `Dark` from the View menu. `System` follows the operating system appearance, and the selected setting is saved for the next launch.

The file context menu provides Discard for detected changes, **Open Command Window Here**, Explorer/Finder navigation, and Lore Lock/Unlock actions. Failed lock queries are recorded in the command log and can be retried by reopening the menu.

1. Browse folders, search files, and preview text in **Files**.
2. Scan for modified, added, and deleted files with **Refresh**.
3. Use **Diff**, **Stage file**, and **Unstage** in **Pending changes**.
4. Enter a message and use **Commit staged** to create a local commit from all staged changes.
5. Inspect revisions, branches, and file history in **Submitted revisions**, **Branches**, and **File history**.
6. Review results and failures in **Details / Command log**. Use **Copy** to copy the full output.

If an authenticated command fails, log in again with the Lore CLI:

```powershell
cd C:\GitHub\lore
..\lore\target\debug\lore.exe login lores://lore.zenogrid.co.kr:41337
```

Refresh uses `--scan` to update Lore's dirty state. Commits are not pushed automatically. Commands run in the background with direct argument passing, without a shell. Duplicate operations are blocked while a command is running.

## Current scope

- Native GPUI UI with Korean IME support and text selection, copy, and paste.
- Local folder browsing and status, staging, commit, diff, history, and branch operations for existing Lore workspaces.
- Switch local branches, create local or remote branches, and merge a selected source branch into the current target branch.
- Conflict-free merges create a local commit automatically; use **Push** to send it remotely. Resolve conflicts with the Lore CLI.
- Folder-by-folder navigation. `.git`, `.lore`, `target`, and entries matched by `.loreignore` are hidden. Search displays up to 2,000 results.
- Previews are limited to 128 KiB, details to 1,500 lines, and command output collection to 16 MiB.
- Ten recent repositories and the selected Lore CLI path are stored in `%LOCALAPPDATA%\LoreLens\settings.json`. The last valid repository is restored on launch; a command-line path takes priority.

## Verification

```powershell
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo test -- --include-ignored  # Includes the local VCS integration test when Lore CLI is available
cargo build
```

The main modules are `src/main.rs` (application state), `src/view.rs` and related view modules (UI), `src/backend/` (CLI, filesystem, and ignore handling), `src/input.rs` (GPUI text input), and `src/settings.rs` (persistent settings). See the [GPUI documentation](https://gpui.rs/).

## External diff and merge tools

Choose `idea`, `p4merge`, or `TortoiseGitMerge` from Tools → Diff / Merge. The selected tool and executable path are shared by diff and merge operations and persist across launches. Use **Locate executable…** to set a path or **Use PATH** to clear it. Windows executables must be `.exe` or `.com`.

Diff compares the current revision with a copy of the local file. Temporary comparison files remain in the OS temporary directory under `lorelens-diff-*` so existing IDE processes can open them. New files that do not exist in the current revision report an extraction error. Conflict-resolution merge execution is not yet connected.

## Menu layout

- **Repository**: open, clone, and revisit repositories.
- **Changes**: Stage, Unstage, Commit staged, File history, and Pending push.
- **Tools**: select a Diff / Merge tool, locate an executable, use `PATH`, and locate the Lore CLI.
- **View**: choose a theme and open the command log.
- **Account**: log in and display the current account.

The second row contains the repository selector, branch menu, Refresh, Sync, and Push. The file context menu contains Diff, Stage/Unstage, Discard, Move, Explorer/Finder navigation, terminal access, and Lock/Unlock actions.

## Localization

Choose English, 한국어, or 简体中文 from **View → Language**. English is the default. The selection is applied immediately, saved to settings, and restored on the next launch.

- `i18n/en-US.json`: source English catalog.
- `i18n/ko-KR.json`: Korean catalog.
- `i18n/zh-CN.json`: Simplified Chinese catalog.
- `lorelens/src/i18n.rs`: translation lookup, locale switching, and interpolation.

Catalogs are flat JSON files keyed by the English UI text. Placeholder names such as `{count}` and `{path}` must remain unchanged, though their order may change. Missing translations fall back to English, and unknown locale settings fall back to `en-US`. Catalogs are embedded at build time, so changes require a rebuild.
