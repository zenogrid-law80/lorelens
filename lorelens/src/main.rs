#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod assets;
#[cfg(target_os = "macos")]
mod macos;
mod state;
use state::{PreviewState, SelectionState};
mod theme;
use theme::{ColorRole::*, apply_theme, defer_theme, palette};
mod backend;
mod branches;
#[cfg(windows)]
mod cli_install;
mod commands;
mod dialogs;
mod external_tools;
mod file_browser;
mod i18n;
mod input;
mod menus;
mod settings;
mod shortcuts;
mod view;
use i18n::{t, tf};

use backend::{Change, Entry, Status};
use gpui::{div, prelude::*, px, *};
use gpui_component::Disableable;
use gpui_component::Icon;
use gpui_component::TitleBar;
use gpui_component::{
  Root, WindowExt,
  button::Button,
  dialog::{Cancel, Confirm, DialogFooter},
  menu::{ContextMenuExt, DropdownMenu, PopupMenuItem},
  resizable::{h_resizable, resizable_panel, v_resizable},
};
use gpui_component::{Sizable, button::ButtonVariants};
use gpui_kit_assets::IconName;
use input::TextInput;
use std::{path::PathBuf, process::Command};

include!(concat!(env!("OUT_DIR"), "/bundled_themes.rs"));

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

fn dialog_footer(id: &'static str, ok_text: String, show_cancel: bool) -> DialogFooter {
  DialogFooter::new()
    .when(show_cancel, |footer| {
      footer.child(Button::new("dialog-cancel").label(t("Cancel")).on_click(|_, window, cx| {
        window.dispatch_action(Box::new(Cancel), cx);
      }))
    })
    .child(Button::new(id).primary().label(ok_text).on_click(|_, window, cx| {
      window.dispatch_action(Box::new(Confirm { secondary: false }), cx);
    }))
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
  Files,
  Pending,
  History,
  Unpushed,
}

struct Lens {
  files_focus: FocusHandle,
  pending_focus: FocusHandle,
  files_scroll: ScrollHandle,
  pending_scroll: UniformListScrollHandle,
  folder_to_select: Option<PathBuf>,
  root: PathBuf,
  directory: PathBuf,
  cli: PathBuf,
  entries: Vec<Entry>,
  expanded_folders: std::collections::HashSet<PathBuf>,
  status: Status,
  locked_paths: std::collections::HashSet<String>,
  connected: bool,
  busy: bool,
  silent_refresh: bool,
  tab: Tab,
  selection: SelectionState,
  preview: PreviewState,
  output: String,
  output_title: String,
  logs: Vec<String>,
  message: Entity<TextInput>,
  filter: Entity<TextInput>,
  pending_filter: Entity<TextInput>,
  selected_path: Entity<TextInput>,
  pending_visible: Vec<usize>,
  notice: String,
  error: bool,
  show_log: bool,
  obliterate_enabled: bool,
  settings: settings::Settings,
  settings_error: Option<String>,
  branch_output: String,
  local_branches: Vec<String>,
  remote_branches: Vec<String>,
  show_branches: bool,
  connect_after_load: bool,
  startup_login_pending: bool,
  #[cfg(windows)]
  cli_install_pending: bool,
  refresh_pending: bool,
  pending_refresh_silent: bool,
  next_refresh: std::time::Instant,
  logged_in_account: String,
  pending_push: Result<Vec<backend::LocalCommit>, String>,
  pending_pull: Result<Vec<backend::LocalCommit>, String>,
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
    self.command(vec!["commit".into(), "--".into(), message], "Commit", false, true, cx);
  }

  fn choose_tool(&mut self, tool: String, retry: Option<(PathBuf, String)>, cx: &mut Context<Self>) {
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
                this.notice = "Select a valid executable file (.exe or .com on Windows).".into();
                this.error = true;
              } else {
                if tool == "custom" {
                  this.settings.custom_tool_path = path.clone();
                } else {
                  this.settings.tool_paths.insert(tool.clone(), path.clone());
                }
                if !this.save_settings() {
                  cx.notify();
                  return;
                }
                this.notice = tf("Saved {tool}: {path}", &[("tool", tool.to_string()), ("path", path.display().to_string())]);
                this.error = false;
                if let Some((root, file)) = retry
                  && this.root == root
                  && this.settings.external_tool == tool
                {
                  this.external_diff(file, cx);
                }
              }
            }
          }
          Ok(Ok(None)) => {}
          _ => {
            this.notice = tf("Could not open the file picker for {tool}.", &[("tool", tool.to_string())]);
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
    let merge = self.status.changes.iter().any(|change| change.path == path && change.conflict);
    let merge_path = path.clone();
    let merge_root = root.clone();
    let merge_branch = self.status.branch.clone();
    let tool = self.settings.external_tool.clone();
    let custom_arguments = (tool == "custom").then(|| self.settings.custom_tool_arguments.clone());
    let configured = if tool == "custom" {
      Some(&self.settings.custom_tool_path)
    } else {
      self.settings.tool_paths.get(&tool)
    };
    let display_name = if tool == "custom" { self.settings.custom_tool_name.clone() } else { tool.clone() };
    let Some(executable) = external_tools::resolve(&tool, configured) else {
      self.choose_tool(tool, Some((root, path)), cx);
      return;
    };
    self.preview.invalidate();
    self.busy = true;
    self.notice = tf(
      if merge { "Opening {tool} merge for {path}…" } else { "Opening {tool} diff for {path}…" },
      &[("tool", display_name), ("path", path.to_string())],
    );
    let task = cx.background_executor().spawn(async move {
      if merge {
        return external_tools::merge(
          &root,
          &path,
          external_tools::Tool {
            name: &tool,
            custom_arguments: custom_arguments.as_deref(),
            executable: &executable,
          },
        );
      }
      external_tools::diff(
        &cli,
        &root,
        &path,
        &revision,
        identity.as_deref(),
        external_tools::Tool {
          name: &tool,
          custom_arguments: custom_arguments.as_deref(),
          executable: &executable,
        },
      )
    });
    cx.spawn(async move |this, cx| {
      let result = task.await;
      let _ = this.update(cx, |this, cx| {
        this.busy = false;
        this.silent_refresh = false;
        match result {
          Ok(()) => {
            if merge && this.root == merge_root && this.status.branch == merge_branch {
              this.command(
                vec!["branch".into(), "merge".into(), "resolve".into(), "--".into(), merge_path],
                "Resolve merge conflict",
                false,
                true,
                cx,
              );
              return;
            }
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

  fn new(root: PathBuf, settings: settings::Settings, settings_error: Option<String>, cx: &mut Context<Self>) -> Self {
    i18n::set_locale(&settings.language);
    let show_log = settings.show_command_log;
    let connect_after_load = backend::is_repository(&root);
    let filter = cx.new(|cx| TextInput::new("Filter files…", cx));
    cx.observe(&filter, |_, _, cx| cx.notify()).detach();
    let pending_filter = cx.new(|cx| TextInput::new("Filter changes…", cx));
    cx.observe(&pending_filter, |_, _, cx| cx.notify()).detach();
    let selected_path = cx.new(|cx| TextInput::new("", cx).read_only());
    let mut view = Self {
            files_focus: cx.focus_handle(),
            pending_focus: cx.focus_handle(),
            files_scroll: ScrollHandle::new(),
            pending_scroll: UniformListScrollHandle::new(),
            folder_to_select: None,
            directory: root.clone(), root, cli: settings.cli.clone().filter(|path| path.is_file()).unwrap_or_else(backend::find_cli), entries: vec![],
            status: Status::default(), connected: false, busy: false, silent_refresh: false, tab: Tab::Pending,
            locked_paths: Default::default(),
            expanded_folders: Default::default(),
            selection: SelectionState::default(), preview: PreviewState::default(), output: "Open a folder to browse local files.\n\nFor version control, open a Lore repository and locate the Lore CLI.\nUse Sync to synchronize the current repository. Commits are not pushed automatically.".into(),
            output_title: "Welcome to LoreLens".into(), logs: vec![],
            message: cx.new(|cx| TextInput::new("Describe your staged changes…", cx)),
            filter, pending_filter, selected_path, pending_visible: Vec::new(), notice: "Opening repository…".into(), error: false, show_log, obliterate_enabled: false,
            settings, settings_error, branch_output: String::new(), show_branches: false,
            connect_after_load,
            startup_login_pending: false,
            #[cfg(windows)]
            cli_install_pending: false,
            refresh_pending: false,
            pending_refresh_silent: false,
            next_refresh: std::time::Instant::now() + std::time::Duration::from_secs(30),
            logged_in_account: "Not signed in".into(),
            local_branches: Vec::new(),
            remote_branches: Vec::new(),
            pending_push: Err("Refresh to check pending push commits.".into()),
            pending_pull: Err("Refresh to check incoming commits.".into()),
        };
    view.load_directory(cx);
    cx.observe(&cx.entity(), |this, _, cx| {
      let path = this.selection.current.as_ref().map(|path| this.root.join(path).display().to_string()).unwrap_or_default();
      this.selected_path.update(cx, |input, cx| {
        if input.content.as_ref() != path {
          input.reset();
          input.content = path.into();
          cx.notify();
        }
      });
      if this.refresh_pending && !this.busy {
        this.refresh_with_mode(this.pending_refresh_silent, cx);
      }
    })
    .detach();
    cx.spawn(async move |this, cx| {
      loop {
        cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
        if this
          .update(cx, |this, cx| {
            if !this.connected || !this.settings.auto_refresh {
              this.next_refresh = std::time::Instant::now() + std::time::Duration::from_secs(30);
            } else if !this.busy && std::time::Instant::now() >= this.next_refresh {
              this.refresh_with_mode(true, cx);
            }
          })
          .is_err()
        {
          break;
        }
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
      self.notice = tf("Repository folder unavailable: {path}", &[("path", path.display().to_string())]);
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
    self.pending_pull = Err("Refresh to check incoming commits.".into());
    self.selection.paths.clear();
    self.selection.anchor = None;
    self.entries.clear();
    self.expanded_folders.clear();
    self.branch_output.clear();
    self.local_branches.clear();
    self.remote_branches.clear();
    self.tab = Tab::Pending;
    self.filter.update(cx, |input, cx| {
      input.reset();
      cx.notify();
    });
    self.output.clear();
    self.preview = PreviewState::default();
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
    let directory = if path.is_dir() { path } else { path.parent().unwrap_or(path) };
    #[cfg(windows)]
    let result = {
      use std::os::windows::process::CommandExt;
      Command::new("pwsh.exe").arg("-NoExit").current_dir(directory).creation_flags(0x00000010).spawn()
    };
    #[cfg(target_os = "macos")]
    let result = Command::new("open").args(["-a", "Terminal"]).arg(directory).spawn();
    #[cfg(not(any(windows, target_os = "macos")))]
    let result = Command::new("x-terminal-emulator").current_dir(directory).spawn();
    if let Err(error) = result {
      self.error = true;
      self.notice = tf("Could not open command window: {error}", &[("error", error.to_string())]);
    }
  }

  fn load_directory(&mut self, cx: &mut Context<Self>) {
    if self.busy {
      return;
    }
    self.preview.invalidate();
    self.busy = true;
    let root = self.root.clone();
    let expanded = self.expanded_folders.clone();
    let task = cx.background_executor().spawn(async move { backend::list_tree(&root, &expanded) });
    cx.spawn(async move |this, cx| {
      let result = task.await;
      let _ = this.update(cx, |this, cx| {
        this.busy = false;
        match result {
          Ok(entries) => {
            this.entries = entries;
            if let Some(target) = this.folder_to_select.take()
              && let Some(index) = this.entries.iter().position(|entry| entry.path == target)
            {
              let relative = target.strip_prefix(&this.root).unwrap_or(&target).to_string_lossy().replace('\\', "/");
              if this.entries[index].directory {
                this.selection.current = Some(relative.clone());
                this.selection.paths.clear();
                this.selection.paths.insert(relative.clone());
                this.selection.anchor = Some(relative);
              } else {
                this.select(relative, cx);
              }
              this.files_scroll.scroll_to_item(index);
            }
            this.error = false;
            this.notice = tf("{count} entries · local filesystem", &[("count", this.entries.len().to_string())]);
            if this.connect_after_load {
              this.connect_after_load = false;
              this.command(vec!["status".into()], "Repository status", true, false, cx);
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

  fn open_bookmark(&mut self, path: PathBuf, cx: &mut Context<Self>) {
    if self.busy || !path.starts_with(&self.root) || path == self.root {
      return;
    }
    let accessible = path.canonicalize().ok().zip(self.root.canonicalize().ok()).is_some_and(|(path, root)| path.starts_with(root));
    if !accessible {
      self.settings.prune_bookmarks();
      self.save_settings();
      self.error = true;
      self.notice = tf("Bookmark unavailable: {path}", &[("path", path.display().to_string())]);
      cx.notify();
      return;
    }
    let mut parent = path.parent();
    while let Some(folder) = parent.filter(|folder| *folder != self.root) {
      self.expanded_folders.insert(folder.to_path_buf());
      parent = folder.parent();
    }
    self.filter.update(cx, |input, cx| {
      input.reset();
      cx.notify();
    });
    self.folder_to_select = Some(path);
    self.load_directory(cx);
  }

  fn choose(&mut self, executable: bool, cx: &mut Context<Self>) {
    if self.busy {
      return;
    }
    let prompt = cx.prompt_for_paths(PathPromptOptions {
      files: executable,
      directories: !executable,
      multiple: false,
      prompt: Some(t(if executable { "Select Lore executable" } else { "Open repository" }).into()),
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

  fn file_command(&mut self, command: &str, window: &mut Window, cx: &mut Context<Self>) {
    if command == "diff" {
      if let Some(path) = self.selection.current.clone() {
        self.resolve_or_diff(path, window, cx);
      }
      return;
    }
    if !self.connected {
      return;
    }
    if let Some(path) = self.selection.current.clone() {
      let mut targets: Vec<_> = self.selection.paths.iter().cloned().collect();
      if targets.is_empty() {
        targets.push(path.clone());
      }
      targets.sort();
      if matches!(command, "stage" | "unstage") && targets.iter().any(|path| self.root.join(path).is_dir()) {
        self.folder_changes_dialog(targets, if command == "stage" { "stage" } else { "unstage" }, window, cx);
        return;
      }
      let args = match command {
        "history" => vec!["file".into(), "history".into(), "--".into(), path, "50".into()],
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
      self.command(args, command, false, matches!(command, "stage" | "unstage"), cx);
    }
  }

  fn select(&mut self, path: String, cx: &mut Context<Self>) {
    if self.busy {
      return;
    }
    self.selection.select(path.clone());
    self.preview.path = Some(path.clone());
    self.preview.content.clear();
    self.output_title = path.clone();
    let root = self.root.clone();
    let file = root.join(&path);
    let request = self.preview.begin();
    let selected = path;
    let preview_root = self.root.clone();
    let task = cx.background_executor().spawn(async move { backend::preview(&root, &file) });
    cx.spawn(async move |this, cx| {
      let result = task.await;
      let _ = this.update(cx, |this, cx| {
        if !this.preview.accepts(request) || this.root != preview_root || this.selection.current.as_ref() != Some(&selected) {
          return;
        }
        this.preview.finish();
        this.preview.content = result.unwrap_or_else(|e| tf("Preview unavailable: {error}\nUse Diff or File history for removed files.", &[("error", e.to_string())]));
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
      self.notice = tf("File unavailable: {path}", &[("path", path.display().to_string())]);
      cx.notify();
    }
  }

  fn button(&self, id: &'static str, label: &'static str, enabled: bool) -> Button {
    let icon = match id {
      "refresh" | "empty-refresh" => Some(IconName::RefreshCw),
      "sync" => Some(IconName::ArrowDown),
      "push" => Some(IconName::ArrowUp),
      "commit" => Some(IconName::GitBranch),
      "generate-message" => Some(IconName::Sparkles),
      "copy" => Some(IconName::Copy),
      "clear-command-log" => Some(IconName::Trash),
      _ => None,
    };
    Button::new(id)
      .label(t(label))
      .small()
      .h(px(34.))
      .px_3()
      .disabled(!enabled)
      .when_some(icon, |button, icon| button.icon(icon))
      .when(id == "commit", |button| button.primary())
  }

  fn pending_paths_valid(&self, paths: &[String], staged: Option<bool>) -> bool {
    let eligible: std::collections::HashSet<_> = self
      .status
      .changes
      .iter()
      .filter(|change| staged.is_none_or(|staged| change.staged == staged))
      .map(|change| change.path.as_str())
      .collect();
    !paths.is_empty() && paths.iter().all(|path| eligible.contains(path.as_str()))
  }

  fn change_row(&self, index: usize, change: &Change, cx: &mut Context<Self>) -> Stateful<Div> {
    let path = change.path.clone();
    let context_path = path.clone();
    let context_root = self.root.clone();
    let view = cx.entity().downgrade();
    div().id(("pending-context", index)).child(
      self
        .row(
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
          &change.node_type,
          cx,
        )
        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
          window.focus(&this.pending_focus, cx);
          if this.busy {
            return;
          }
          let modifiers = event.modifiers();
          let additive = modifiers.control || modifiers.platform;
          if !additive && !modifiers.shift {
            this.select(path.clone(), cx);
            return;
          }
          let visible: Vec<_> = this.pending_visible.iter().map(|index| this.status.changes[*index].path.clone()).collect();
          // Selection shared with the file browser must stay within this list.
          let visible_set: std::collections::HashSet<_> = visible.iter().collect();
          this.selection.paths.retain(|p| visible_set.contains(p));
          this.selection.click(path.clone(), &visible, additive, modifiers.shift);
          this.preview.invalidate();
          this.notice = tf("{count} items selected", &[("count", this.selection.paths.len().to_string())]);
          cx.notify();
        }))
        .context_menu(move |mut menu, _, cx| {
          let Some((paths, stage_paths, unstage_paths, revert, enabled)) = view.upgrade().and_then(|entity| {
            let lens = entity.read(cx);
            if lens.root != context_root {
              return None;
            }
            let use_selection = lens.selection.paths.contains(&context_path);
            let mut paths = Vec::new();
            let mut stage_paths = Vec::new();
            let mut unstage_paths = Vec::new();
            let mut revert = false;
            for change in &lens.status.changes {
              if !(if use_selection {
                lens.selection.paths.contains(&change.path)
              } else {
                change.path == context_path
              }) {
                continue;
              }
              paths.push(change.path.clone());
              if change.staged {
                unstage_paths.push(change.path.clone());
              } else if !change.conflict {
                stage_paths.push(change.path.clone());
              }
              revert |= change.staged || change.file_marker() == "M" || matches!(change.action.as_str(), "modify" | "remove" | "delete");
            }
            if paths.is_empty() {
              return None;
            }
            let revert = revert || paths.len() > 1 || matches!(lens.root.join(&paths[0]).try_exists(), Ok(false));
            Some((paths, stage_paths, unstage_paths, revert, !lens.busy && lens.connected))
          }) else {
            return menu;
          };
          if !enabled {
            return menu;
          }
          let shortcut_settings = view.upgrade().map(|entity| entity.read(cx).settings.shortcuts.clone()).unwrap_or_default();
          if !stage_paths.is_empty() {
            let view = view.clone();
            let root = context_root.clone();
            menu = menu.item(
              PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, "Stage", "stage"))
                .disabled(!enabled)
                .on_click(move |_, _, cx| {
                  let _ = view.update(cx, |this, cx| {
                    if this.busy || !this.connected || this.root != root || !this.pending_paths_valid(&stage_paths, Some(false)) {
                      return;
                    }
                    let mut args = vec!["stage".into(), "--".into()];
                    args.extend(stage_paths.iter().cloned());
                    this.command(args, "Stage", false, true, cx);
                  });
                }),
            );
          }
          for unstage_only in [true, false] {
            if (unstage_only && unstage_paths.is_empty()) || (!unstage_only && !revert) {
              continue;
            }
            let view = view.clone();
            let paths = if unstage_only { unstage_paths.clone() } else { paths.clone() };
            let root = context_root.clone();
            menu = menu.item(
              PopupMenuItem::new(shortcuts::shortcut_label(
                &shortcut_settings,
                if unstage_only {
                  "Unstage…"
                } else if paths.len() > 1 {
                  "Revert selected files"
                } else {
                  "Revert file…"
                },
                if unstage_only { "unstage" } else { "revert" },
              ))
              .disabled(!enabled)
              .on_click(move |_, window, cx| {
                let _ = view.update(cx, |this, cx| {
                  if this.root == root {
                    this.revert_dialog(paths.clone(), unstage_only, window, cx);
                  }
                });
              }),
            );
          }
          // Obliterate resolves the current/staged node. Untracked additions and
          // staged deletions have no eligible node to remove.
          let obliterate_paths = view
            .upgrade()
            .map(|entity| {
              let lens = entity.read(cx);
              if !lens.obliterate_enabled {
                return Vec::new();
              }
              lens
                .status
                .changes
                .iter()
                .filter(|change| {
                  paths.contains(&change.path)
                    && !change.conflict
                    && !(!change.staged && matches!(change.action.as_str(), "add" | "create"))
                    && !(change.staged && matches!(change.action.as_str(), "remove" | "delete"))
                    && backend::obliterate_args(&lens.root, &change.path).is_ok()
                })
                .map(|change| change.path.clone())
                .collect::<Vec<_>>()
            })
            .unwrap_or_default();
          if !obliterate_paths.is_empty() {
            let obliterate_view = view.clone();
            let obliterate_root = context_root.clone();
            menu = menu.item(PopupMenuItem::new(t("Obliterate…")).disabled(!enabled).on_click(move |_, window, cx| {
              let _ = obliterate_view.update(cx, |this, cx| {
                if this.root == obliterate_root {
                  this.obliterate_files_dialog(obliterate_paths.clone(), window, cx);
                }
              });
            }));
          }
          let reset_view = view.clone();
          let reset_root = context_root.clone();
          let mut reset_paths = paths;
          reset_paths.sort();
          reset_paths.dedup();
          menu.separator().item(
            PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, "Reset…", "reset"))
              .disabled(!enabled)
              .on_click(move |_, window, cx| {
                let _ = reset_view.update(cx, |this, cx| {
                  if this.root == reset_root {
                    this.reset_dialog(reset_paths.clone(), window, cx);
                  }
                });
              }),
          )
        }),
    )
  }

  fn row(&self, index: usize, path: &str, action: &str, state: &str, node_type: &str, cx: &App) -> Stateful<Div> {
    let rgb = palette(cx);
    let kind = match node_type.to_ascii_lowercase().as_str() {
      "directory" | "folder" => "Folder",
      "file" => "File",
      "link" => "Link",
      _ => match std::fs::symlink_metadata(self.root.join(path)) {
        Ok(meta) if meta.file_type().is_symlink() => "Link",
        Ok(meta) if meta.is_dir() => "Folder",
        Ok(meta) if meta.is_file() => "File",
        _ => "Unknown type",
      },
    };
    div()
      .id(("file-row", index))
      .flex()
      .items_center()
      .h(px(29.))
      .px_3()
      .gap_2()
      .border_b_1()
      .border_color(rgb(Divider))
      .bg(rgb(if self.selection.paths.contains(path) { Selected } else { PANEL }))
      .cursor_pointer()
      .hover(|s| s.bg(rgb(Hover)))
      .child(
        gpui_component::checkbox::Checkbox::new(("select-change", index))
          .checked(self.selection.paths.contains(path))
          .disabled(self.busy)
          .tab_stop(false),
      )
      .child(
        div().w(px(16.)).flex_shrink_0().text_color(rgb(if kind == "Folder" { Warning } else { MUTED })).child(
          gpui_component::Icon::new(match kind {
            "Folder" => gpui_component::IconName::Folder,
            "Link" => gpui_component::IconName::ExternalLink,
            _ => gpui_component::IconName::File,
          })
          .size(px(16.)),
        ),
      )
      .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().child(path.to_string()))
      .child(
        div()
          .w(px(90.))
          .flex_shrink_0()
          .text_size(px(11.))
          .text_color(rgb(if matches!(action, "remove" | "delete") {
            Danger
          } else if matches!(action, "add" | "create") {
            Success
          } else {
            MUTED
          }))
          .child(t(action)),
      )
      .child(
        div()
          .w(px(110.))
          .flex_shrink_0()
          .text_size(px(11.))
          .text_color(rgb(if state == "Conflict" {
            Danger
          } else if state == "Staged" {
            Success
          } else {
            MUTED
          }))
          .child(t(state)),
      )
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
  gpui_platform::application().with_assets(assets::Assets).run(move |cx: &mut App| {
    #[cfg(target_os = "macos")]
    macos::set_application_icon();
    gpui_component::init(cx);
    for theme in BUNDLED_THEMES {
      gpui_component::ThemeRegistry::global_mut(cx).load_themes_from_str(theme).expect("bundled theme must be valid");
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
            #[cfg(windows)]
            if this.cli_install_pending && !this.busy {
              this.cli_install_pending = false;
              this.install_cli_dialog(window, cx);
              return;
            }
            if this.startup_login_pending && !this.busy {
              this.startup_login_pending = false;
              this.login_dialog(window, cx);
            }
          })
          .detach();
          #[cfg(windows)]
          {
            this.cli_install_pending = cli_install::cli_missing(&this.cli);
          }
          let cli = this.cli.clone();
          let root = this.root.clone();
          let identity = this.settings.identity.clone();
          let check = cx.background_executor().spawn(async move {
            let output = backend::run_as(&cli, &root, &["auth".into(), "list".into()], true, None)?;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
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
          })
          .detach();
          let mut was_active = window.is_window_active();
          cx.observe_window_activation(window, move |this, window, cx| {
            let active = window.is_window_active();
            if active && !was_active {
              this.refresh_with_mode(true, cx);
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
    cx.on_window_closed(|cx, _| {
      if cx.windows().is_empty() {
        cx.quit();
      }
    })
    .detach();
  });
}
