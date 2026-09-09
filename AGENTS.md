# LoreLens contributor guidance

## Project layout

- `lorelens/` contains the Rust 2024 GPUI desktop application.
- `lorelens/src/backend/` owns Lore CLI invocation, filesystem work, and ignore handling.
- `lorelens/src/` contains application state, UI views, dialogs, commands, settings, and platform integration.
- `i18n/` contains flat JSON catalogs. English (`en-US.json`) is the source catalog.
- `website/` is an independent static Node.js site. Its generated output is `website/dist/`.
- `packaging/` contains release packaging assets and scripts.

## Development

From the repository root, use:

```powershell
cargo run -- <path-to-a-Lore-workspace>
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo build
```

Run `cargo test -- --include-ignored` only when a usable Lore CLI and local test workspace are available.

For the website:

```powershell
cd website
npm run dev
npm run build
```

## Code conventions

- Format Rust with the repository `rustfmt.toml` (two-space indentation and 200-column width).
- Keep UI behavior and application state in the existing module that owns it; keep CLI argument construction and command execution in `backend/`.
- Invoke external commands with direct arguments, never through a shell.
- Preserve the app's local-first behavior: filesystem browsing and previews must continue to work without the Lore CLI.
- Avoid logging credentials, authentication tokens, or other secrets.

## Localization

- Add or update English UI text in `i18n/en-US.json`, then keep Korean and Simplified Chinese catalogs aligned.
- Keep interpolation placeholder names such as `{count}` and `{path}` unchanged. Their order may differ by language.
- Catalogs are embedded at build time, so rebuild after catalog changes.

## Change safety

- Do not overwrite or discard unrelated working-tree changes.
- Treat deletion, discard, revert, and obliterate flows as destructive. Retain their validation and confirmation behavior.
- Run the checks relevant to changed code before handing off work. For Rust changes, start with formatting, Clippy, and targeted tests; use a build when feasible.
