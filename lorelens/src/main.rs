#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod state;
#[cfg(target_os = "macos")]
mod macos;
use state::{PreviewState, SelectionState};
mod theme;
use theme::{ColorRole::*, apply_theme, palette};
mod backend;
mod branches;
mod commands;
mod dialogs;
mod external_tools;
mod file_browser;
mod i18n;
mod input;
mod menus;
mod settings;
mod view;
use i18n::{t, tf};

use backend::{Change, Entry, Status};
use gpui::{div, prelude::*, px, *};
use gpui_component::Disableable;
use gpui_component::TitleBar;
use gpui_component::{
    Root, WindowExt,
    button::Button,
    dialog::DialogButtonProps,
    menu::{ContextMenuExt, DropdownMenu, PopupMenuItem},
};
use gpui_component::{Sizable, button::ButtonVariants};
use input::TextInput;
use std::{path::PathBuf, process::Command};

fn canonicalize_path(path: PathBuf) -> PathBuf {
    let canonical = match path.canonicalize() {
        Ok(path) => path,
        Err(_) => return path,
    };

    #[cfg(windows)]
    {
        let text = canonical.to_string_lossy();
        if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
            PathBuf::from(format!(r"\\{unc}"))
        } else {
            PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text))
        }
    }

    #[cfg(not(windows))]
    canonical
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Files,
    Pending,
    History,
}

struct Lens {
    files_focus: FocusHandle,
    files_scroll: ScrollHandle,
    folder_to_select: Option<PathBuf>,
    root: PathBuf,
    directory: PathBuf,
    cli: PathBuf,
    entries: Vec<Entry>,
    status: Status,
    locked_paths: std::collections::HashSet<String>,
    connected: bool,
    busy: bool,
    tab: Tab,
    selection: SelectionState,
    preview: PreviewState,
    output: String,
    output_title: String,
    logs: Vec<String>,
    message: Entity<TextInput>,
    filter: Entity<TextInput>,
    notice: String,
    error: bool,
    show_log: bool,
    settings: settings::Settings,
    settings_error: Option<String>,
    branch_output: String,
    local_branches: Vec<String>,
    remote_branches: Vec<String>,
    show_branches: bool,
    connect_after_load: bool,
    startup_login_pending: bool,
    refresh_pending: bool,
    logged_in_account: String,
    pending_push: Result<Vec<String>, String>,
}

impl Lens {
    fn generate_commit_message(&mut self, cx: &mut Context<Self>) {
        if self.busy || !self.connected {
            return;
        }
        let staged: Vec<_> = self.status.changes.iter().filter(|change| change.staged).collect();
        let Some(first) = staged.first() else {
            self.error = true;
            self.notice = "Stage files before generating a commit message.".into();
            cx.notify();
            return;
        };
        let action = match first.action.as_str() {
            "add" | "create" => "Add",
            "remove" | "delete" => "Remove",
            _ => "Update",
        };
        let scope = first.path.split('/').next().filter(|segment| !segment.is_empty()).unwrap_or("files");
        let message = if staged.len() == 1 {
            format!("{action} {}", first.path)
        } else {
            format!("{action} {scope} ({count} files)", count = staged.len())
        };
        self.message.update(cx, |input, cx| {
            input.reset();
            input.content = message.into();
            cx.notify();
        });
        self.error = false;
        self.notice = "Generated commit message from staged changes.".into();
        cx.notify();
    }

    fn commit_staged(&mut self, cx: &mut Context<Self>) {
        if self.busy || !self.connected || !self.status.changes.iter().any(|c| c.staged) {
            return;
        }
        let message = self.message.read(cx).content.trim().to_string();
        if message.is_empty() {
            self.error = true;
            self.notice = "Enter a commit message first.".into();
            cx.notify();
            return;
        }
        self.command(
            vec!["commit".into(), "--".into(), message],
            "Commit",
            false,
            true,
            cx,
        );
    }

    fn choose_tool(
        &mut self,
        tool: String,
        retry: Option<(PathBuf, String)>,
        cx: &mut Context<Self>,
    ) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tf("Select {tool} executable", &[("tool", tool.to_string())]).into()),
        });
        cx.spawn(async move |this, cx| {
            let result = prompt.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            let path = canonicalize_path(path);
                            if !path.is_absolute() || !external_tools::is_executable(&path) {
                                this.notice =
                                    "Select a valid executable file (.exe or .com on Windows)."
                                        .into();
                                this.error = true;
                            } else {
                                this.settings.tool_paths.insert(tool.clone(), path.clone());
                                if !this.save_settings() {
                                    cx.notify();
                                    return;
                                }
                                this.notice = tf(
                                    "Saved {tool}: {path}",
                                    &[
                                        ("tool", tool.to_string()),
                                        ("path", path.display().to_string()),
                                    ],
                                );
                                this.error = false;
                                if let Some((root, file)) = retry {
                                    if this.root == root && this.settings.external_tool == tool {
                                        this.external_diff(file, cx);
                                    }
                                }
                            }
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => {
                        this.notice = tf(
                            "Could not open the file picker for {tool}.",
                            &[("tool", tool.to_string())],
                        );
                        this.error = true;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn external_diff(&mut self, path: String, cx: &mut Context<Self>) {
        if self.busy || !self.connected || self.root.join(&path).is_dir() {
            return;
        }
        let root = self.root.clone();
        let cli = self.cli.clone();
        let identity = self.settings.identity.clone();
        let revision = format!("{}@{}", self.status.branch, self.status.revision);
        let tool = self.settings.external_tool.clone();
        let Some(executable) = external_tools::resolve(&tool, self.settings.tool_paths.get(&tool))
        else {
            self.choose_tool(tool, Some((root, path)), cx);
            return;
        };
        self.preview.invalidate();
        self.busy = true;
        self.notice = tf(
            "Opening {tool} diff for {path}…",
            &[("tool", tool.to_string()), ("path", path.to_string())],
        );
        let task = cx.background_executor().spawn(async move {
            external_tools::diff(
                &cli,
                &root,
                &path,
                &revision,
                identity.as_deref(),
                &tool,
                &executable,
            )
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(()) => {
                        this.notice = "External diff opened".into();
                        this.error = false;
                    }
                    Err(error) => {
                        this.notice = error;
                        this.error = true;
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn new(
        root: PathBuf,
        settings: settings::Settings,
        settings_error: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        i18n::set_locale(&settings.language);
        let connect_after_load = backend::is_repository(&root);
        let filter = cx.new(|cx| TextInput::new("Filter files…", cx));
        cx.observe(&filter, |_, _, cx| cx.notify()).detach();
        let mut view = Self {
            files_focus: cx.focus_handle(),
            files_scroll: ScrollHandle::new(),
            folder_to_select: None,
            directory: root.clone(), root, cli: settings.cli.clone().filter(|path| path.is_file()).unwrap_or_else(backend::find_cli), entries: vec![],
            status: Status::default(), connected: false, busy: false, tab: Tab::Pending,
            locked_paths: Default::default(),
            selection: SelectionState::default(), preview: PreviewState::default(), output: "Open a folder to browse local files.\n\nFor version control, open a Lore repository and locate the Lore CLI.\nUse Sync to synchronize the current repository. Commits are not pushed automatically.".into(),
            output_title: "Welcome to LoreLens".into(), logs: vec![],
            message: cx.new(|cx| TextInput::new("Describe your staged changes…", cx)),
            filter, notice: "Opening repository…".into(), error: false, show_log: false,
            settings, settings_error, branch_output: String::new(), show_branches: false,
            connect_after_load,
            startup_login_pending: false,
            refresh_pending: false,
            logged_in_account: "Not signed in".into(),
            local_branches: Vec::new(),
            remote_branches: Vec::new(),
            pending_push: Err("Refresh to check pending push commits.".into()),
        };
        view.load_directory(cx);
        cx.observe(&cx.entity(), |this, _, cx| {
            if this.refresh_pending && !this.busy {
                this.refresh(cx);
            }
        })
        .detach();
        view
    }

    fn save_settings(&mut self) -> bool {
        if let Some(error) = &self.settings_error {
            self.log(format!("Settings not saved: {error}"));
            self.error = true;
            self.notice = "Settings unavailable · see command log".into();
            return false;
        }
        if let Err(error) = self.settings.save(&settings::Settings::path()) {
            self.log(format!("Settings save failed: {error}"));
            self.error = true;
            self.notice = "Could not save settings · see command log".into();
            return false;
        }
        true
    }

    fn open_repository(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if !path.is_dir() {
            self.error = true;
            self.notice = tf(
                "Repository folder unavailable: {path}",
                &[("path", path.display().to_string())],
            );
            cx.notify();
            return;
        }
        self.root = canonicalize_path(path);
        self.directory = self.root.clone();
        self.folder_to_select = None;
        self.refresh_pending = false;
        self.status = Status::default();
        self.locked_paths.clear();
        self.connected = false;
        self.selection.current = None;
        self.pending_push = Err("Refresh to check pending push commits.".into());
        self.selection.paths.clear();
        self.selection.anchor = None;
        self.entries.clear();
        self.branch_output.clear();
        self.local_branches.clear();
        self.remote_branches.clear();
        self.tab = Tab::Pending;
        self.filter.update(cx, |input, cx| {
            input.reset();
            cx.notify();
        });
        self.output.clear();
        self.output_title = "Repository opened".into();
        self.connect_after_load = backend::is_repository(&self.root);
        if self.connect_after_load {
            self.settings.remember(&self.root);
            self.save_settings();
        }
        self.load_directory(cx);
    }

    fn log(&mut self, text: String) {
        self.logs.push(text);
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }

    fn open_command_window(&mut self, path: &std::path::Path) {
        let directory = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        #[cfg(windows)]
        let result = {
            use std::os::windows::process::CommandExt;
            Command::new("pwsh.exe")
                .arg("-NoExit")
                .current_dir(directory)
                .creation_flags(0x00000010)
                .spawn()
        };
        #[cfg(target_os = "macos")]
        let result = Command::new("open")
            .args(["-a", "Terminal"])
            .arg(directory)
            .spawn();
        #[cfg(not(any(windows, target_os = "macos")))]
        let result = Command::new("x-terminal-emulator")
            .current_dir(directory)
            .spawn();
        if let Err(error) = result {
            self.error = true;
            self.notice = tf(
                "Could not open command window: {error}",
                &[("error", error.to_string())],
            );
        }
    }

    fn load_directory(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.preview.invalidate();
        self.busy = true;
        let root = self.root.clone();
        let directory = self.directory.clone();
        let task = cx
            .background_executor()
            .spawn(async move { backend::list_directory(&root, &directory) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(entries) => {
                        this.entries = entries;
                        if let Some(folder) = this.folder_to_select.take() {
                            if let Some(index) =
                                this.entries.iter().position(|entry| entry.path == folder)
                            {
                                let relative = folder
                                    .strip_prefix(&this.root)
                                    .unwrap_or(&folder)
                                    .to_string_lossy()
                                    .replace('\\', "/");
                                this.selection.current = Some(relative.clone());
                                this.selection.paths.clear();
                                this.selection.paths.insert(relative.clone());
                                this.selection.anchor = Some(relative);
                                this.files_scroll.scroll_to_item(index);
                            }
                        }
                        this.error = false;
                        this.notice = tf(
                            "{count} entries · local filesystem",
                            &[("count", this.entries.len().to_string())],
                        );
                        if this.connect_after_load {
                            this.connect_after_load = false;
                            this.command(
                                vec!["status".into()],
                                "Repository status",
                                true,
                                false,
                                cx,
                            );
                        }
                    }
                    Err(e) => {
                        this.error = true;
                        this.notice = e;
                        this.folder_to_select = None;
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn choose(&mut self, executable: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: executable,
            directories: !executable,
            multiple: false,
            prompt: Some(
                t(if executable {
                    "Select Lore executable"
                } else {
                    "Open repository"
                })
                .into(),
            ),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| {
                    if this.busy {
                        return;
                    }
                    if executable {
                        this.cli = path.clone();
                        this.settings.cli = Some(path);
                        this.save_settings();
                        this.refresh(cx);
                    } else {
                        this.open_repository(path, cx);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn file_command(&mut self, command: &str, cx: &mut Context<Self>) {
        if command == "diff" {
            if let Some(path) = self.selection.current.clone() {
                self.external_diff(path, cx);
            }
            return;
        }
        if !self.connected {
            return;
        }
        if let Some(path) = self.selection.current.clone() {
            let args = match command {
                "history" => vec![
                    "file".into(),
                    "history".into(),
                    "--".into(),
                    path,
                    "50".into(),
                ],
                _ => {
                    let mut paths: Vec<_> = self.selection.paths.iter().cloned().collect();
                    paths.sort();
                    if paths.is_empty() {
                        paths.push(path);
                    }
                    let mut args = vec![command.into(), "--".into()];
                    args.extend(paths);
                    args
                }
            };
            self.command(
                args,
                command,
                false,
                matches!(command, "stage" | "unstage"),
                cx,
            );
        }
    }

    fn select(&mut self, path: String, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.selection.select(path.clone());
        self.output_title = path.clone();
        self.show_log = false;
        let root = self.root.clone();
        let file = root.join(&path);
        let request = self.preview.begin();
        let selected = path;
        let preview_root = self.root.clone();
        let task = cx
            .background_executor()
            .spawn(async move { backend::preview(&root, &file) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if !this.preview.accepts(request)
                    || this.root != preview_root
                    || this.selection.current.as_ref() != Some(&selected)
                {
                    return;
                }
                this.preview.finish();
                this.output = result.unwrap_or_else(|e| {
                    tf(
                        "Preview unavailable: {error}\nUse Diff or File history for removed files.",
                        &[("error", e.to_string())],
                    )
                });
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn open_selected_file(&mut self, cx: &mut Context<Self>) {
        let Some(selected) = self.selection.current.clone() else {
            return;
        };
        let path = self.root.join(selected);
        if path.is_file() {
            cx.open_with_system(&path);
        } else {
            self.error = true;
            self.notice = tf(
                "File unavailable: {path}",
                &[("path", path.display().to_string())],
            );
            cx.notify();
        }
    }

    fn button(&self, id: &'static str, label: &'static str, enabled: bool) -> Button {
        Button::new(id)
            .label(t(label))
            .small()
            .disabled(!enabled)
            .when(id == "commit", |button| button.primary())
    }

    fn change_row(&self, index: usize, change: &Change, cx: &mut Context<Self>) -> Stateful<Div> {
        let path = change.path.clone();
        let context_path = path.clone();
        let context_root = self.root.clone();
        let view = cx.entity().downgrade();
        div().id(("pending-context", index)).child(self.row(
            index,
            &change.path,
            &change.action,
            if change.conflict {
                "Conflict"
            } else if change.staged {
                "Staged"
            } else {
                "Unstaged"
            },
            change.size,
            cx,
        )
        .on_click(cx.listener(move |this, _, _, cx| this.select(path.clone(), cx)))
        .context_menu(move |mut menu, _, cx| {
            let state = view.upgrade().and_then(|entity| {
                let lens = entity.read(cx);
                if lens.root != context_root { return None; }
                lens.status.changes.iter().find(|c| c.path == context_path).map(|c| (
                    c.staged,
                    c.staged || c.file_marker() == "M" || matches!(c.action.as_str(), "modify" | "remove" | "delete")
                        || matches!(lens.root.join(&context_path).try_exists(), Ok(false)),
                    !lens.busy && lens.connected,
                ))
            });
            let Some((staged, revert, enabled)) = state else { return menu; };
            if !staged {
                let view = view.clone();
                let path = context_path.clone();
                let root = context_root.clone();
                menu = menu.item(PopupMenuItem::new(t("Stage"))
                    .disabled(!enabled).on_click(move |_, _, cx| {
                        let _ = view.update(cx, |this, cx| {
                            if this.busy || !this.connected || this.root != root
                                || !this.status.changes.iter().any(|c| c.path == path && !c.staged) {
                                return;
                            }
                            this.command(vec!["stage".into(), "--".into(), path.clone()],
                                "Stage", false, true, cx);
                        });
                    }));
            }
            for unstage_only in [true, false] {
                if (unstage_only && !staged) || (!unstage_only && !revert) { continue; }
                let view = view.clone();
                let path = context_path.clone();
                let root = context_root.clone();
                menu = menu.item(PopupMenuItem::new(t(if unstage_only { "Unstage…" } else { "Revert file…" }))
                    .disabled(!enabled).on_click(move |_, window, cx| {
                        let _ = view.update(cx, |this, cx| {
                            if this.root == root { this.revert_dialog(path.clone(), unstage_only, window, cx); }
                        });
                    }));
            }
            menu
        }))
    }

    fn row(
        &self,
        index: usize,
        path: &str,
        action: &str,
        state: &str,
        size: u64,
        cx: &App,
    ) -> Stateful<Div> {
        let rgb = palette(cx);
        div()
            .id(("file-row", index))
            .flex()
            .items_center()
            .h(px(34.))
            .px_4()
            .gap_3()
            .border_b_1()
            .border_color(rgb(Divider))
            .bg(rgb(if self.selection.current.as_deref() == Some(path) {
                Selected
            } else {
                PANEL
            }))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(Hover)))
            .child(
                div()
                    .w(px(18.))
                    .text_color(rgb(BLUE))
                    .child(if state == "Folder" { "▸" } else { "·" }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(path.to_string()),
            )
            .child(div().w(px(90.)).text_color(rgb(MUTED)).child(t(action)))
            .child(
                div()
                    .w(px(125.))
                    .text_color(rgb(if state == "Staged" {
                        Success
                    } else if state == "Conflict" {
                        Danger
                    } else {
                        MUTED
                    }))
                    .child(t(state)),
            )
            .child(div().w(px(75.)).text_right().text_color(rgb(MUTED)).child(
                if state == "Folder" {
                    "—".into()
                } else {
                    format_size(size)
                },
            ))
    }
}

fn format_size(size: u64) -> String {
    if size >= 1_048_576 {
        format!("{:.1} MB", size as f64 / 1_048_576.)
    } else if size >= 1024 {
        format!("{:.1} KB", size as f64 / 1024.)
    } else {
        format!("{size} B")
    }
}

fn main() {
    let (settings, settings_error) = match settings::Settings::load(&settings::Settings::path()) {
        Ok(settings) => (settings, None),
        Err(error) => (settings::Settings::default(), Some(error.to_string())),
    };
    i18n::set_locale(&settings.language);
    let root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| settings.restore())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let root = canonicalize_path(root);
    Application::new()
        .with_assets(gpui_component_assets::Assets)
        .run(move |cx: &mut App| {
            #[cfg(target_os = "macos")]
            macos::set_application_icon();
            gpui_component::init(cx);
            let theme_set: gpui_component::ThemeSet =
                serde_json::from_str(include_str!("../themes/longbridge-pro.json"))
                    .expect("bundled Longbridge-inspired theme must be valid");
            for config in theme_set.themes {
                let theme = gpui_component::Theme::global_mut(cx);
                if config.mode.is_dark() {
                    theme.dark_theme = std::rc::Rc::new(config);
                } else {
                    theme.light_theme = std::rc::Rc::new(config);
                }
            }
            apply_theme(&settings.theme, None, cx);
            input::init(cx);
            let bounds = Bounds::centered(None, size(px(1360.), px(900.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(1000.), px(700.))),
                    titlebar: Some(TitlebarOptions {
                        title: Some(t("LoreLens — Desktop repository client").into()),
                        ..TitleBar::title_bar_options()
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    let view = cx.new(|cx| Lens::new(root, settings, settings_error, cx));
                    view.update(cx, |this, cx| {
                        cx.observe_in(&cx.entity(), window, |this, _, window, cx| {
                            if this.startup_login_pending && !this.busy {
                                this.startup_login_pending = false;
                                this.login_dialog(window, cx);
                            }
                        }).detach();
                        let cli = this.cli.clone();
                        let root = this.root.clone();
                        let identity = this.settings.identity.clone();
                        let check = cx.background_executor().spawn(async move {
                            let output = backend::run_as(&cli, &root,
                                &["auth".into(), "list".into()], true, None)?;
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
                            backend::has_valid_login(&output, identity.as_deref(), now)
                        });
                        cx.spawn(async move |this, cx| {
                            let result = check.await;
                            let _ = this.update(cx, |this, cx| {
                                match result {
                                    Ok(valid) => this.startup_login_pending = !valid,
                                    Err(error) => this.log(format!("Startup login check: {error}")),
                                }
                                cx.notify();
                            });
                        }).detach();
                        let mut was_active = window.is_window_active();
                        cx.observe_window_activation(window, move |this, window, cx| {
                            let active = window.is_window_active();
                            if active && !was_active {
                                this.refresh(cx);
                            }
                            was_active = active;
                        })
                        .detach();
                        cx.observe_window_appearance(window, |this, window, cx| {
                            apply_theme(&this.settings.theme, Some(window), cx);
                            cx.notify();
                        })
                        .detach();
                    });
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("Could not open LoreLens window");
            cx.activate(true);
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
        });
}
