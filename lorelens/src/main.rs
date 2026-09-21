#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod assets;
#[cfg(target_os = "macos")]
mod macos;
mod state;
use state::{PendingTreeRow, PreviewState, SelectionState};
mod theme;
use theme::{ColorRole::*, apply_theme, defer_theme, palette, selected_row_palette};
mod backend;
mod branches;
#[cfg(windows)]
mod cli_install;
mod commands;
mod dialogs;
mod external_tools;
mod file_browser;
mod history;
mod i18n;
mod input;
mod menus;
mod remote_history;
mod settings;
mod shortcuts;
mod sparse_editor;
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
use input::{CommitMessage, TextInput};
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
      footer.child(Button::new("dialog-cancel").border_0().label(t("Cancel")).on_click(|_, window, cx| {
        window.dispatch_action(Box::new(Cancel), cx);
      }))
    })
    .child(Button::new(id).border_0().primary().label(ok_text).on_click(|_, window, cx| {
      window.dispatch_action(Box::new(Confirm { secondary: false }), cx);
    }))
}

fn is_staged_modification(change: &Change) -> bool {
  let action = change.action.to_ascii_lowercase();
  change.staged && !matches!(action.as_str(), "add" | "create" | "remove" | "delete")
}

fn change_action_label(action: &str) -> &str {
  if action.eq_ignore_ascii_case("keep") { "modify" } else { action }
}

fn is_modified_change(change: &Change) -> bool {
  !change.conflict && matches!(change.action.to_ascii_lowercase().as_str(), "keep" | "modify" | "edit")
}

fn same_change_path(left: &str, right: &str) -> bool {
  let left = left.replace('\\', "/");
  let right = right.replace('\\', "/");
  if cfg!(windows) { left.eq_ignore_ascii_case(&right) } else { left == right }
}

fn change_path_is_within(path: &str, folder: &str) -> bool {
  let mut path = path.replace('\\', "/");
  let mut folder = folder.replace('\\', "/");
  if cfg!(windows) {
    path.make_ascii_lowercase();
    folder.make_ascii_lowercase();
  }
  std::path::Path::new(&path).starts_with(std::path::Path::new(&folder))
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
  Files,
  Pending,
  History,
  Unpushed,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum ChangeStateFilter {
  #[default]
  All,
  Staged,
  Unstaged,
}

enum DiffRetry {
  Working { root: PathBuf, path: String },
  Merge { root: PathBuf, paths: Vec<String> },
  History { root: PathBuf, path: String, older: String, newer: String },
  RemoteHistory { root: PathBuf, comparison: backend::RevisionComparison },
}

#[derive(Clone)]
struct ConflictDialogEntry {
  path: String,
  inputs: Option<backend::MergeInputs>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum ConflictSort {
  FileName,
  Extension,
  #[default]
  FullPath,
}

struct Lens {
  files_focus: FocusHandle,
  pending_focus: FocusHandle,
  files_scroll: ScrollHandle,
  command_log_scroll: ScrollHandle,
  preview_scroll: ScrollHandle,
  pending_scroll: UniformListScrollHandle,
  folder_to_select: Option<PathBuf>,
  root: PathBuf,
  directory: PathBuf,
  cli: PathBuf,
  entries: Vec<Entry>,
  file_filter_generation: u64,
  expanded_folders: std::collections::HashSet<PathBuf>,
  collapsed_change_folders: std::collections::HashSet<String>,
  status: Status,
  locked_paths: std::collections::HashSet<String>,
  connected: bool,
  busy: bool,
  silent_refresh: bool,
  tab: Tab,
  selection: SelectionState,
  preview: PreviewState,
  file_history: history::FileHistoryState,
  remote_history: remote_history::RemoteHistoryState,
  output: String,
  output_title: String,
  logs: Vec<String>,
  message: Entity<CommitMessage>,
  filter: Entity<TextInput>,
  pending_filter: Entity<TextInput>,
  change_state_filter: ChangeStateFilter,
  selected_path: Entity<TextInput>,
  pending_visible: Vec<usize>,
  pending_rows: Vec<PendingTreeRow>,
  pending_folder_focus: Option<String>,
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
  startup_login_checked: bool,
  repository_login_checking: bool,
  startup_login_pending: bool,
  #[cfg(windows)]
  cli_install_pending: bool,
  conflict_dialog_pending: bool,
  conflict_dialog_open: bool,
  conflict_dialog_entries: std::rc::Rc<std::cell::RefCell<Vec<ConflictDialogEntry>>>,
  conflict_dialog_selection: std::rc::Rc<std::cell::RefCell<std::collections::HashSet<String>>>,
  conflict_dialog_working: std::rc::Rc<std::cell::Cell<bool>>,
  progress_popup_dismissed: bool,
  conflict_dialog_sort: std::rc::Rc<std::cell::Cell<ConflictSort>>,
  prompted_conflicts: std::collections::HashSet<String>,
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
      input.set_value(message, cx);
    });
    self.error = false;
    self.notice = "Generated commit message from staged changes.".into();
    cx.notify();
  }

  fn commit_staged(&mut self, cx: &mut Context<Self>) {
    if self.busy || !self.connected || !self.status.changes.iter().any(|c| c.staged) {
      return;
    }
    let message = self.message.read(cx).value(cx).trim().to_string();
    if message.is_empty() {
      self.error = true;
      self.notice = "Enter a commit message first.".into();
      cx.notify();
      return;
    }
    self.command(vec!["commit".into(), "--".into(), message], "Commit", false, true, cx);
  }

  fn choose_tool(&mut self, tool: String, retry: Option<DiffRetry>, cx: &mut Context<Self>) {
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
                if this.settings.external_tool == tool {
                  match retry {
                    Some(DiffRetry::Working { root, path }) if this.root == root => this.external_diff(path, cx),
                    Some(DiffRetry::Merge { root, paths }) if this.root == root => this.external_merge(paths, cx),
                    Some(DiffRetry::History { root, path, older, newer }) if this.root == root && this.file_history.matches_pair(&path, &older, &newer) => this.diff_selected_history(cx),
                    Some(DiffRetry::RemoteHistory { root, comparison }) if this.root == root && this.remote_history.matches_comparison(&comparison) => this.diff_remote_file(cx),
                    _ => {}
                  }
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
    if self.busy || !self.connected || self.root.join(&path).is_dir() || !self.status.changes.iter().any(|change| change.path == path && is_modified_change(change)) {
      return;
    }
    let root = self.root.clone();
    let cli = self.cli.clone();
    let identity = self.settings.identity.clone();
    let revision = format!("{}@{}", self.status.branch, self.status.revision);
    let tool = self.settings.external_tool.clone();
    let custom_arguments = (tool == "custom").then(|| self.settings.custom_tool_arguments.clone());
    let configured = if tool == "custom" {
      Some(&self.settings.custom_tool_path)
    } else {
      self.settings.tool_paths.get(&tool)
    };
    let display_name = if tool == "custom" { self.settings.custom_tool_name.clone() } else { tool.clone() };
    let Some(executable) = external_tools::resolve(&tool, configured) else {
      self.choose_tool(tool, Some(DiffRetry::Working { root, path }), cx);
      return;
    };
    self.preview.invalidate();
    self.busy = true;
    self.notice = tf("Opening {tool} diff for {path}…", &[("tool", display_name), ("path", path.to_string())]);
    let task = cx.background_executor().spawn(async move {
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

  fn external_merge(&mut self, mut paths: Vec<String>, cx: &mut Context<Self>) {
    if self.busy || !self.connected {
      return;
    }
    paths.sort();
    paths.dedup();
    paths.retain(|path| self.status.changes.iter().any(|change| change.path == *path && change.conflict) && !self.root.join(path).is_dir());
    if paths.is_empty() {
      return;
    }
    if paths.iter().any(|path| backend::merge_inputs(&self.root, path) != Some(backend::MergeInputs::ThreeWay)) {
      self.notice = t("Merge is available only for three-way conflicts.");
      self.error = true;
      cx.notify();
      return;
    }

    let root = self.root.clone();
    let branch = self.status.branch.clone();
    let tool = self.settings.external_tool.clone();
    let custom_arguments = (tool == "custom").then(|| self.settings.custom_tool_arguments.clone());
    let configured = if tool == "custom" {
      Some(&self.settings.custom_tool_path)
    } else {
      self.settings.tool_paths.get(&tool)
    };
    let executable = external_tools::resolve(&tool, configured);
    let retry_tool = tool.clone();
    let cli = self.cli.clone();
    let identity = self.settings.identity.clone();

    self.preview.invalidate();
    self.busy = true;
    self.notice = tf("Merging {count} files…", &[("count", paths.len().to_string())]);
    let task_paths = paths.clone();
    let task_root = root.clone();
    let task = cx.background_executor().spawn(async move {
      let mut pending = Vec::new();
      for path in task_paths {
        if !backend::try_auto_merge(&task_root, &path)? {
          let Some(executable) = executable.as_ref() else {
            pending.push(path);
            continue;
          };
          external_tools::merge(
            &task_root,
            &path,
            external_tools::Tool {
              name: &tool,
              custom_arguments: custom_arguments.as_deref(),
              executable,
            },
          )?;
        }
        backend::run_as(
          &cli,
          &task_root,
          &["branch".into(), "merge".into(), "resolve".into(), "--".into(), path.clone()],
          false,
          identity.as_deref(),
        )?;
        backend::cleanup_resolved_merge(&cli, &task_root, &path, identity.as_deref())?;
      }
      Ok::<Vec<String>, String>(pending)
    });
    cx.spawn(async move |this, cx| {
      let result = task.await;
      let _ = this.update(cx, |this, cx| {
        this.busy = false;
        this.silent_refresh = false;
        match result {
          Ok(pending) if this.root == root && this.status.branch == branch => {
            if pending.is_empty() {
              this.refresh(cx);
            } else {
              this.choose_tool(retry_tool, Some(DiffRetry::Merge { root, paths: pending }), cx);
              cx.notify();
            }
            return;
          }
          Ok(_) => {
            this.notice = t("Merge inputs have changed. Try resolving again.");
            this.error = true;
          }
          Err(error) => {
            this.notice = error;
            this.error = true;
            this.refresh_pending = true;
          }
        }
        cx.notify();
      });
    })
    .detach();
    cx.notify();
  }

  fn new(root: PathBuf, settings: settings::Settings, settings_error: Option<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
    i18n::set_locale(&settings.language);
    let show_log = settings.show_command_log;
    let connect_after_load = backend::is_repository(&root);
    let filter = cx.new(|cx| TextInput::new("Filter files…", cx).with_icon(IconName::Search));
    cx.observe(&filter, |this, _, cx| this.refresh_file_filter(cx)).detach();
    let pending_filter = cx.new(|cx| TextInput::new("Filter changes…", cx).with_icon(IconName::Search));
    cx.observe(&pending_filter, |_, _, cx| cx.notify()).detach();
    let selected_path = cx.new(|cx| TextInput::new("", cx).read_only());
    let mut view = Self {
            files_focus: cx.focus_handle(),
            pending_focus: cx.focus_handle(),
            files_scroll: ScrollHandle::new(),
            command_log_scroll: ScrollHandle::new(),
            preview_scroll: ScrollHandle::new(),
            pending_scroll: UniformListScrollHandle::new(),
            folder_to_select: None,
            directory: root.clone(), root, cli: settings.cli.clone().filter(|path| path.is_file()).unwrap_or_else(backend::find_cli), entries: vec![], file_filter_generation: 0,
            status: Status::default(), connected: false, busy: false, silent_refresh: false, tab: Tab::Pending,
            locked_paths: Default::default(),
            expanded_folders: Default::default(),
            collapsed_change_folders: Default::default(),
            selection: SelectionState::default(), preview: PreviewState::default(), output: "Open a folder to browse local files.\n\nFor version control, open a Lore repository and locate the Lore CLI.\nUse Sync to synchronize the current repository. Commits are not pushed automatically.".into(),
            file_history: history::FileHistoryState::default(),
            remote_history: remote_history::RemoteHistoryState::new(cx),
            output_title: "Welcome to LoreLens".into(), logs: vec![],
            message: cx.new(|cx| CommitMessage::new(window, cx)),
            pending_folder_focus: None,
            filter, pending_filter, change_state_filter: ChangeStateFilter::All, selected_path, pending_visible: Vec::new(), pending_rows: Vec::new(), notice: "Opening repository…".into(), error: false, show_log, obliterate_enabled: false,
            settings, settings_error, branch_output: String::new(), show_branches: false,
            connect_after_load,
            startup_login_checked: !connect_after_load,
            repository_login_checking: false,
            startup_login_pending: false,
            #[cfg(windows)]
            cli_install_pending: false,
            conflict_dialog_pending: false,
            conflict_dialog_open: false,
            conflict_dialog_entries: Default::default(),
            conflict_dialog_selection: Default::default(),
            conflict_dialog_working: Default::default(),
            progress_popup_dismissed: false,
            conflict_dialog_sort: Default::default(),
            prompted_conflicts: Default::default(),
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

  fn check_repository_login(&mut self, cx: &mut Context<Self>) {
    if self.repository_login_checking || !backend::is_repository(&self.root) {
      return;
    }
    self.repository_login_checking = true;
    let cli = self.cli.clone();
    let root = self.root.clone();
    let identity = self.settings.identity.clone();
    let checked_root = root.clone();
    let check = cx.background_executor().spawn(async move {
      let repository_identity = backend::repository_identity(&root)?;
      let output = backend::run_global(&cli, &root, &["auth".into(), "list".into()], true, None)?;
      let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
      backend::startup_login_status(&output, repository_identity.as_deref(), identity.as_deref(), now)
    });
    cx.spawn(async move |this, cx| {
      let result = check.await;
      let _ = this.update(cx, |this, cx| {
        if this.root != checked_root {
          return;
        }
        this.repository_login_checking = false;
        this.startup_login_checked = true;
        match result {
          Ok(backend::StartupLoginStatus::Valid(identity)) => {
            if this.settings.identity.is_none() && identity.is_some() {
              this.settings.identity = identity;
              this.save_settings();
            }
            if this.connect_after_load && !this.busy {
              this.connect_after_load = false;
              this.command(vec!["status".into(), "--scan".into()], "Repository status", true, false, cx);
            }
          }
          Ok(backend::StartupLoginStatus::LoginRequired) => {
            this.connect_after_load = false;
            this.startup_login_pending = true;
          }
          Ok(backend::StartupLoginStatus::IdentityMismatch { repository, login }) => {
            this.connect_after_load = false;
            this.clear_mismatched_repository_login(repository, login, cx);
          }
          Err(error) => {
            this.connect_after_load = false;
            this.error = true;
            this.notice = "Could not verify repository authentication · see details".into();
            this.output_title = "Authentication failed".into();
            this.output = error.clone();
            this.log(format!("Repository login check: {error}"));
          }
        }
        cx.notify();
      });
    })
    .detach();
  }

  fn clear_mismatched_repository_login(&mut self, repository: String, login: String, cx: &mut Context<Self>) {
    self.repository_login_checking = true;
    self.settings.identity = None;
    self.settings.login_remote = None;
    self.logged_in_account = "Not signed in".into();
    self.save_settings();

    let cli = self.cli.clone();
    let root = self.root.clone();
    let checked_root = root.clone();
    let clear_login = login.clone();
    let clear = cx
      .background_executor()
      .spawn(async move { backend::run_global(&cli, &root, &["auth".into(), "clear".into()], false, Some(&clear_login)) });
    cx.spawn(async move |this, cx| {
      let result = clear.await;
      let _ = this.update(cx, |this, cx| {
        if this.root != checked_root {
          return;
        }
        this.repository_login_checking = false;
        this.startup_login_pending = true;
        this.error = true;
        this.notice = "Repository login does not match · see details".into();
        this.output_title = "Authentication failed".into();
        let mismatch = tf(
          "Repository identity {repository} does not match the signed-in identity {login}. Sign in to this repository again.",
          &[("repository", repository), ("login", login.clone())],
        );
        this.output = match result {
          Ok(_) => format!("{mismatch}\n\n{}", tf("Stored authentication for {login} was removed.", &[("login", login)])),
          Err(error) => {
            this.log(format!("Authentication clear failed: {error}"));
            format!("{mismatch}\n\n{}", tf("Could not remove stored authentication: {error}", &[("error", error)]))
          }
        };
        this.log(this.output.clone());
        cx.notify();
      });
    })
    .detach();
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
    self.conflict_dialog_pending = false;
    self.conflict_dialog_open = false;
    self.conflict_dialog_entries.borrow_mut().clear();
    self.conflict_dialog_selection.borrow_mut().clear();
    self.conflict_dialog_working.set(false);
    self.prompted_conflicts.clear();
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
    self.collapsed_change_folders.clear();
    self.pending_folder_focus = None;
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
    self.file_history = history::FileHistoryState::default();
    self.remote_history = remote_history::RemoteHistoryState::new(cx);
    self.output_title = "Repository opened".into();
    self.connect_after_load = backend::is_repository(&self.root);
    self.startup_login_checked = !self.connect_after_load;
    self.repository_login_checking = false;
    self.startup_login_pending = false;
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
    self.command_log_scroll.scroll_to_bottom();
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
    self.file_filter_generation = self.file_filter_generation.wrapping_add(1);
    let generation = self.file_filter_generation;
    self.busy = true;
    let root = self.root.clone();
    let expanded = self.expanded_folders.clone();
    let query = self.filter.read(cx).content.to_string();
    let task = cx.background_executor().spawn(async move {
      if query.trim().is_empty() {
        backend::list_tree(&root, &expanded)
      } else {
        backend::search_tree(&root, &query)
      }
    });
    cx.spawn(async move |this, cx| {
      let result = task.await;
      let _ = this.update(cx, |this, cx| {
        // This task owns busy regardless of whether a newer search owns the
        // displayed entries. Always finish loading and the startup connection.
        this.busy = false;
        let loaded = result.is_ok();
        if this.file_filter_generation == generation {
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
            }
            Err(e) => {
              this.error = true;
              this.notice = e;
              this.folder_to_select = None;
            }
          }
        }
        if loaded && this.connect_after_load {
          if this.startup_login_checked {
            this.connect_after_load = false;
            this.command(vec!["status".into(), "--scan".into()], "Repository status", true, false, cx);
          } else if !this.repository_login_checking {
            this.check_repository_login(cx);
          }
        }
        cx.notify();
      });
    })
    .detach();
    cx.notify();
  }

  fn refresh_file_filter(&mut self, cx: &mut Context<Self>) {
    self.file_filter_generation = self.file_filter_generation.wrapping_add(1);
    let generation = self.file_filter_generation;
    let query = self.filter.read(cx).content.to_string();
    let root = self.root.clone();
    let expected_root = root.clone();
    let expanded = self.expanded_folders.clone();
    let task = cx.background_executor().spawn(async move {
      if query.trim().is_empty() {
        backend::list_tree(&root, &expanded)
      } else {
        backend::search_tree(&root, &query)
      }
    });
    cx.spawn(async move |this, cx| {
      let result = task.await;
      let _ = this.update(cx, |this, cx| {
        if this.file_filter_generation != generation || this.root != expected_root {
          return;
        }
        match result {
          Ok(entries) => {
            this.entries = entries;
            this.error = false;
          }
          Err(error) => {
            this.error = true;
            this.notice = error;
          }
        }
        this.files_scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
      });
    })
    .detach();
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
    if command == "history" {
      if let Some(path) = self.selection.current.clone() {
        self.open_file_history(path, 100, cx);
      }
      return;
    }
    if command == "diff" {
      if let Some(path) = self.selection.current.clone() {
        if self.status.changes.iter().any(|change| change.path == path && change.conflict) {
          self.resolve_or_diff(path, window, cx);
        } else {
          self.external_diff(path, cx);
        }
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
      let mut args = vec![command.into(), "--".into()];
      args.extend(targets);
      self.command(args, command, false, matches!(command, "stage" | "unstage"), cx);
    }
  }

  fn select(&mut self, path: String, cx: &mut Context<Self>) {
    if self.busy {
      return;
    }
    self.selection.select(path.clone());
    self.pending_folder_focus = None;
    if self.tab == Tab::History {
      self.open_file_history(path, 100, cx);
      return;
    }
    self.preview.path = Some(path.clone());
    self.preview.image_path = None;
    self.preview.content.clear();
    self.preview.sheets.clear();
    self.preview.selected_sheet = 0;
    self.preview.is_image = backend::is_image_path(std::path::Path::new(&path));
    self.output_title = path.clone();
    if self.preview.is_image {
      self.preview.finish();
      cx.notify();
      return;
    }
    if backend::is_spreadsheet_path(std::path::Path::new(&path)) {
      let root = self.root.clone();
      let file = root.join(&path);
      let request = self.preview.begin();
      let selected = path;
      let preview_root = self.root.clone();
      let task = cx.background_executor().spawn(async move { backend::preview_spreadsheet(&file) });
      cx.spawn(async move |this, cx| {
        let result = task.await;
        let _ = this.update(cx, |this, cx| {
          if !this.preview.accepts(request) || this.root != preview_root || this.selection.current.as_ref() != Some(&selected) {
            return;
          }
          this.preview.finish();
          match result {
            Ok(sheets) => this.preview.sheets = sheets,
            Err(error) => this.preview.content = tf("Preview unavailable: {error}\nUse Diff or File history for removed files.", &[("error", error)]),
          }
          cx.notify();
        });
      })
      .detach();
      cx.notify();
      return;
    }
    if backend::is_unreal_asset_path(std::path::Path::new(&path)) {
      let root = self.root.clone();
      let file = root.clone().join(&path);
      let request = self.preview.begin();
      let selected = path;
      let preview_root = self.root.clone();
      let task = cx.background_executor().spawn(async move { backend::preview_unreal_asset(&root, &file) });
      cx.spawn(async move |this, cx| {
        let result = task.await;
        let _ = this.update(cx, |this, cx| {
          if !this.preview.accepts(request) || this.root != preview_root || this.selection.current.as_ref() != Some(&selected) {
            return;
          }
          this.preview.finish();
          this.preview.content = result.unwrap_or_else(|error| tf("Preview unavailable: {error}\nUse Diff or File history for removed files.", &[("error", error)]));
          cx.notify();
        });
      })
      .detach();
      cx.notify();
      return;
    }
    if backend::is_fbx_path(std::path::Path::new(&path)) {
      let root = self.root.clone();
      let file = root.join(&path);
      let request = self.preview.begin();
      let selected = path;
      let preview_root = self.root.clone();
      let task = cx.background_executor().spawn(async move { backend::render_fbx_preview(&file) });
      cx.spawn(async move |this, cx| {
        let result = task.await;
        let _ = this.update(cx, |this, cx| {
          if !this.preview.accepts(request) || this.root != preview_root || this.selection.current.as_ref() != Some(&selected) {
            return;
          }
          this.preview.finish();
          match result {
            Ok(image_path) => {
              this.preview.image_path = Some(image_path);
              this.preview.is_image = true;
            }
            Err(error) => {
              this.preview.content = tf("Preview unavailable: {error}\nUse Diff or File history for removed files.", &[("error", error)]);
            }
          }
          cx.notify();
        });
      })
      .detach();
      cx.notify();
      return;
    }
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
      .border_0()
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

  fn pending_folder_row(&self, index: usize, row: PendingTreeRow, window_active: bool, cx: &mut Context<Self>) -> Stateful<Div> {
    let PendingTreeRow::Folder { path, name, depth, change_index } = row else {
      unreachable!("pending_folder_row requires a folder row");
    };
    let rgb = palette(cx);
    let filtering = !self.pending_filter.read(cx).content.is_empty();
    let expanded = !self.collapsed_change_folders.contains(&path) || filtering;
    let folder_change = change_index.and_then(|index| self.status.changes.get(index));
    let checkbox = folder_change.map(|change| (change_index.expect("folder change index"), change.path.clone()));
    let selected = self.pending_folder_focus.as_ref() == Some(&path) || folder_change.is_some_and(|change| self.selection.paths.contains(&change.path));
    let (selected_background, selected_foreground) = selected_row_palette(cx, window_active);
    let action = folder_change.map(|change| change.action.as_str()).unwrap_or("");
    let state = folder_change.map_or("", |change| {
      if change.conflict {
        "Conflict"
      } else if change.staged {
        "Staged"
      } else {
        "Unstaged"
      }
    });
    let context_path = path.clone();
    let context_root = self.root.clone();
    let view = cx.entity().downgrade();
    div().id(("pending-folder-context", index)).child(
      div()
        .id(("pending-folder", index))
        .flex()
        .items_center()
        .h(px(25.))
        .px_3()
        .gap_2()
        .border_b_1()
        .border_color(rgb(Divider))
        .bg(if selected { selected_background } else { rgb(PANEL) })
        .when(selected, |row| row.text_color(selected_foreground).font_weight(FontWeight::BOLD))
        .when(!filtering && !selected, |row| row.cursor_pointer().hover(|style| style.bg(rgb(Hover))))
        .when(!filtering && selected, |row| row.cursor_pointer())
        .child(div().w(px(16.)).flex_shrink_0().when_some(checkbox, |slot, (change_index, change_path)| {
          let selected = self.selection.paths.contains(&change_path);
          slot.child(
            gpui_component::checkbox::Checkbox::new(("select-folder-change", change_index))
              .checked(selected)
              .disabled(self.busy)
              .tab_stop(false)
              .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                cx.stop_propagation();
                if this.busy {
                  return;
                }
                this.pending_folder_focus = None;
                if *checked {
                  this.selection.paths.insert(change_path.clone());
                  this.selection.current = Some(change_path.clone());
                } else {
                  this.selection.paths.remove(&change_path);
                  if this.selection.current.as_ref() == Some(&change_path) {
                    this.selection.current = this.selection.paths.iter().next().cloned();
                  }
                }
                this.selection.anchor = this.selection.current.clone();
                this.preview.invalidate();
                this.notice = tf("{count} items selected", &[("count", this.selection.paths.len().to_string())]);
                cx.notify();
              })),
          )
        }))
        .child(div().w(px(depth as f32 * 16.)).flex_shrink_0())
        .child(
          div()
            .w(px(16.))
            .flex_shrink_0()
            .text_color(if selected { selected_foreground } else { rgb(MUTED) })
            .child(Icon::new(if expanded { IconName::ChevronDown } else { IconName::ChevronRight }).size(px(14.))),
        )
        .child(
          div()
            .w(px(16.))
            .flex_shrink_0()
            .text_color(if selected { selected_foreground } else { rgb(Warning) })
            .child(Icon::new(if expanded { IconName::FolderOpen } else { IconName::Folder }).size(px(16.))),
        )
        .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().child(name))
        .child(
          div()
            .w(px(90.))
            .flex_shrink_0()
            .text_size(px(11.))
            .text_color(if selected {
              selected_foreground
            } else {
              rgb(if matches!(action, "remove" | "delete") {
                Danger
              } else if matches!(action, "add" | "create") {
                Success
              } else {
                MUTED
              })
            })
            .child(t(action)),
        )
        .child(
          div()
            .w(px(110.))
            .flex_shrink_0()
            .text_size(px(11.))
            .text_color(if selected {
              selected_foreground
            } else {
              rgb(if state == "Conflict" {
                Danger
              } else if state == "Staged" {
                Success
              } else {
                MUTED
              })
            })
            .child(t(state)),
        )
        .on_click(cx.listener(move |this, _, window, cx| {
          window.focus(&this.pending_focus, cx);
          if this.busy {
            return;
          }
          this.select_pending_row(index, false, cx);
          if !this.pending_filter.read(cx).content.is_empty() {
            return;
          }
          if !this.collapsed_change_folders.remove(&path) {
            this.collapsed_change_folders.insert(path.clone());
          }
          cx.notify();
        }))
        .context_menu(move |mut menu, _, cx| {
          let Some((folder_paths, has_unstaged, has_staged, ready, vcs, exists, bookmarked, delete_allowed, shortcut_settings)) = view.upgrade().map(|entity| {
            let lens = entity.read(cx);
            let folder_paths = if lens.root == context_root {
              state::pending_folder_paths(&lens.status.changes, &context_path)
            } else {
              Vec::new()
            };
            let has_unstaged = lens.status.changes.iter().any(|change| folder_paths.contains(&change.path) && !change.staged && !change.conflict);
            let has_staged = lens.status.changes.iter().any(|change| folder_paths.contains(&change.path) && change.staged);
            let delete_allowed = !lens
              .status
              .changes
              .iter()
              .any(|change| folder_paths.iter().any(|path| same_change_path(&change.path, path)) && is_staged_modification(change));
            let ready = !lens.busy && lens.root == context_root;
            let absolute_path = context_root.join(&context_path);
            (
              folder_paths,
              has_unstaged,
              has_staged,
              ready,
              ready && lens.connected,
              matches!(absolute_path.try_exists(), Ok(true)),
              lens.settings.is_bookmarked(&context_root, &absolute_path),
              delete_allowed,
              lens.settings.shortcuts.clone(),
            )
          }) else {
            return menu;
          };
          let enabled = ready && !folder_paths.is_empty();
          let select_view = view.clone();
          let select_root = context_root.clone();
          let select_path = context_path.clone();
          menu = menu.item(PopupMenuItem::new(t("Select all files in folder")).disabled(!enabled).on_click(move |_, _, cx| {
            let _ = select_view.update(cx, |this, cx| {
              if this.busy || this.root != select_root {
                return;
              }
              let paths = state::pending_folder_paths(&this.status.changes, &select_path);
              if paths.is_empty() {
                return;
              }
              this.selection.select_all(&paths);
              this.preview.invalidate();
              this.notice = tf("{count} items selected", &[("count", paths.len().to_string())]);
              cx.notify();
            });
          }));
          for (action, label, action_enabled) in [
            ("stage", "Stage folder", vcs && has_unstaged),
            ("unstage", "Unstage folder", vcs && has_staged),
            ("reset", "Reset folder", vcs && !folder_paths.is_empty()),
          ] {
            let action_view = view.clone();
            let action_root = context_root.clone();
            let action_path = context_path.clone();
            menu = menu.item(
              PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, label, if action == "reset" { "reset" } else { action }))
                .disabled(!action_enabled)
                .on_click(move |_, window, cx| {
                  let _ = action_view.update(cx, |this, cx| {
                    if this.busy || !this.connected || this.root != action_root {
                      return;
                    }
                    this.folder_changes_dialog(vec![action_path.clone()], action, window, cx);
                  });
                }),
            );
          }
          let absolute_path = context_root.join(&context_path);
          let copy_path = absolute_path.clone();
          let reveal_path = absolute_path.clone();
          let terminal_path = absolute_path.clone();
          let terminal_view = view.clone();
          menu = menu
            .separator()
            .item(PopupMenuItem::new(t("Copy full path")).on_click(move |_, _, cx| {
              cx.write_to_clipboard(ClipboardItem::new_string(copy_path.to_string_lossy().into_owned()));
            }))
            .item(
              PopupMenuItem::new(shortcuts::shortcut_label(
                &shortcut_settings,
                if cfg!(target_os = "macos") { "Show in Finder" } else { "Show in Explorer" },
                "reveal",
              ))
              .disabled(!ready || !exists)
              .on_click(move |_, _, cx| cx.reveal_path(&reveal_path)),
            )
            .item(
              PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, "Open Command Window Here", "terminal")).on_click(move |_, _, cx| {
                let _ = terminal_view.update(cx, |this, cx| {
                  this.open_command_window(&terminal_path);
                  cx.notify();
                });
              }),
            )
            .separator();
          let bookmark_view = view.clone();
          let bookmark_root = context_root.clone();
          let bookmark_path = absolute_path.clone();
          menu = menu.item(
            PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, if bookmarked { "Remove bookmark" } else { "Add bookmark" }, "bookmark"))
              .disabled(!ready || !exists)
              .on_click(move |_, _, cx| {
                let _ = bookmark_view.update(cx, |this, cx| {
                  if this.root != bookmark_root || !matches!(bookmark_path.try_exists(), Ok(true)) {
                    return;
                  }
                  let added = this.settings.toggle_bookmark(&bookmark_root, &bookmark_path);
                  if this.save_settings() {
                    this.error = false;
                    this.notice = tf(
                      if added { "Bookmark added: {path}" } else { "Bookmark removed: {path}" },
                      &[("path", bookmark_path.strip_prefix(&bookmark_root).unwrap_or(&bookmark_path).display().to_string())],
                    );
                  }
                  cx.notify();
                });
              }),
          );
          let delete_view = view.clone();
          let delete_root = context_root.clone();
          let delete_relative = context_path.clone();
          let delete_path = absolute_path;
          if delete_allowed {
            menu.separator().item(
              PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, "Delete…", "delete"))
                .disabled(!exists || !ready)
                .on_click(move |_, window, cx| {
                  let _ = delete_view.update(cx, |this, cx| {
                    if !this.busy
                      && this.root == delete_root
                      && matches!(delete_path.try_exists(), Ok(true))
                      && !this
                        .status
                        .changes
                        .iter()
                        .any(|change| change_path_is_within(&change.path, &delete_relative) && is_staged_modification(change))
                    {
                      this.delete_dialog(delete_path.clone(), window, cx);
                    }
                  });
                }),
            )
          } else {
            menu
          }
        }),
    )
  }

  fn change_row(&self, index: usize, change: &Change, name: &str, depth: usize, window_active: bool, cx: &mut Context<Self>) -> Stateful<Div> {
    let path = change.path.clone();
    let context_path = path.clone();
    let context_root = self.root.clone();
    let view = cx.entity().downgrade();
    div().id(("pending-context", index)).child(
      self
        .row(index, change, name, depth, window_active, cx)
        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
          window.focus(&this.pending_focus, cx);
          if this.busy {
            return;
          }
          let modifiers = event.modifiers();
          this.pending_folder_focus = None;
          let additive = modifiers.control || modifiers.platform;
          if !additive && !modifiers.shift {
            this.select(path.clone(), cx);
            return;
          }
          let visible: Vec<_> = this
            .pending_rows
            .iter()
            .filter_map(|row| match row {
              PendingTreeRow::Change { index, .. } => Some(this.status.changes[*index].path.clone()),
              PendingTreeRow::Folder { .. } => None,
            })
            .collect();
          // Selection shared with the file browser must stay within this list.
          let visible_set: std::collections::HashSet<_> = visible.iter().collect();
          this.selection.paths.retain(|p| visible_set.contains(p));
          this.selection.click(path.clone(), &visible, additive, modifiers.shift);
          this.preview.invalidate();
          this.notice = tf("{count} items selected", &[("count", this.selection.paths.len().to_string())]);
          cx.notify();
        }))
        .context_menu(move |mut menu, _, cx| {
          let Some((paths, stage_paths, diff_path, resolve_path, context_file, context_exists, ready, enabled)) = view.upgrade().and_then(|entity| {
            let lens = entity.read(cx);
            if lens.root != context_root {
              return None;
            }
            let use_selection = lens.selection.paths.contains(&context_path);
            let mut paths = Vec::new();
            let mut stage_paths = Vec::new();
            for change in &lens.status.changes {
              if !(if use_selection {
                lens.selection.paths.contains(&change.path)
              } else {
                change.path == context_path
              }) {
                continue;
              }
              paths.push(change.path.clone());
              if !change.staged && !change.conflict {
                stage_paths.push(change.path.clone());
              }
            }
            if paths.is_empty() {
              return None;
            }
            let diff_path = lens
              .status
              .changes
              .iter()
              .find(|change| change.path == context_path && is_modified_change(change) && !lens.root.join(&change.path).is_dir())
              .map(|change| change.path.clone());
            let resolve_path = lens
              .status
              .changes
              .iter()
              .find(|change| change.path == context_path && change.conflict)
              .map(|change| change.path.clone());
            let context_file = lens
              .status
              .changes
              .iter()
              .filter(|change| change.path == context_path)
              .all(|change| !matches!(change.node_type.to_ascii_lowercase().as_str(), "directory" | "folder"))
              && !lens.root.join(&context_path).is_dir();
            let context_exists = matches!(lens.root.join(&context_path).try_exists(), Ok(true));
            Some((paths, stage_paths, diff_path, resolve_path, context_file, context_exists, !lens.busy, !lens.busy && lens.connected))
          }) else {
            return menu;
          };
          let shortcut_settings = view.upgrade().map(|entity| entity.read(cx).settings.shortcuts.clone()).unwrap_or_default();
          let mut selected_paths = paths.clone();
          selected_paths.sort();
          selected_paths.dedup();
          let single_file = selected_paths.len() == 1 && context_file;
          if view
            .upgrade()
            .is_some_and(|entity| backend::duplicate_change_paths(&entity.read(cx).status.changes).contains(&context_path))
          {
            let reset_view = view.clone();
            let reset_root = context_root.clone();
            let reset_path = context_path.clone();
            menu = menu.item(PopupMenuItem::new(t("Reset Files")).on_click(move |_, window, cx| {
              let _ = reset_view.update(cx, |this, cx| {
                if this.root == reset_root && backend::duplicate_change_paths(&this.status.changes).contains(&reset_path) {
                  // The clicked row opens the repository-wide duplicate workflow.
                  this.deduplicate_files_dialog(window, cx);
                }
              });
            }));
          }
          if single_file {
            let history_view = view.clone();
            let history_root = context_root.clone();
            let history_path = context_path.clone();
            menu = menu.item(PopupMenuItem::new(t("File History")).disabled(!enabled).on_click(move |_, _, cx| {
              let _ = history_view.update(cx, |this, cx| {
                if !this.busy && this.connected && this.root == history_root {
                  this.open_file_history(history_path.clone(), 100, cx);
                }
              });
            }));
          }
          if single_file && let Some(resolve_path) = resolve_path {
            let resolve_view = view.clone();
            let resolve_root = context_root.clone();
            menu = menu
              .item(
                PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, "Resolve", "diff"))
                  .disabled(!enabled)
                  .on_click(move |_, window, cx| {
                    let _ = resolve_view.update(cx, |this, cx| {
                      if !this.busy && this.connected && this.root == resolve_root && this.status.changes.iter().any(|change| change.path == resolve_path && change.conflict) {
                        this.resolve_or_diff(resolve_path.clone(), window, cx);
                      }
                    });
                  }),
              )
              .separator();
          } else if single_file && let Some(diff_path) = diff_path {
            let diff_view = view.clone();
            let diff_root = context_root.clone();
            menu = menu
              .item(
                PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, "Diff with current revision", "diff"))
                  .disabled(!enabled)
                  .on_click(move |_, _, cx| {
                    let _ = diff_view.update(cx, |this, cx| {
                      if !this.busy && this.connected && this.root == diff_root && this.status.changes.iter().any(|change| change.path == diff_path && is_modified_change(change)) {
                        this.external_diff(diff_path.clone(), cx);
                      }
                    });
                  }),
              )
              .separator();
          }
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
          let revert_view = view.clone();
          let revert_root = context_root.clone();
          let revert_paths = paths.clone();
          menu = menu.item(
            PopupMenuItem::new(shortcuts::shortcut_label(
              &shortcut_settings,
              if selected_paths.len() > 1 { "Revert selected files" } else { "Revert file…" },
              "revert",
            ))
            .disabled(!enabled)
            .on_click(move |_, window, cx| {
              let _ = revert_view.update(cx, |this, cx| {
                if this.root == revert_root {
                  this.changes_revert_dialog(revert_paths.clone(), window, cx);
                }
              });
            }),
          );
          if selected_paths.len() > 1 {
            let copy_paths = selected_paths.iter().map(|path| context_root.join(path).to_string_lossy().into_owned()).collect::<Vec<_>>().join("\n");
            menu = menu.separator().item(PopupMenuItem::new(t("Copy selected full paths")).on_click(move |_, _, cx| {
              cx.write_to_clipboard(ClipboardItem::new_string(copy_paths.clone()));
            }));
          } else if single_file {
            let absolute_path = context_root.join(&context_path);
            let copy_path = absolute_path.clone();
            let reveal_path = absolute_path.clone();
            let terminal_path = absolute_path.clone();
            let terminal_view = view.clone();
            menu = menu
              .item(PopupMenuItem::new(t("Copy full path")).on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(copy_path.to_string_lossy().into_owned()));
              }))
              .item(
                PopupMenuItem::new(shortcuts::shortcut_label(
                  &shortcut_settings,
                  if cfg!(target_os = "macos") { "Show in Finder" } else { "Show in Explorer" },
                  "reveal",
                ))
                .disabled(!ready || !context_exists)
                .on_click(move |_, _, cx| {
                  cx.reveal_path(&reveal_path);
                }),
              )
              .item(
                PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, "Open Command Window Here", "terminal")).on_click(move |_, _, cx| {
                  let _ = terminal_view.update(cx, |this, cx| {
                    this.open_command_window(&terminal_path);
                    cx.notify();
                  });
                }),
              )
              .separator();
            let bookmark_view = view.clone();
            let bookmark_root = context_root.clone();
            let bookmark_path = absolute_path.clone();
            let bookmarked = view.upgrade().is_some_and(|entity| entity.read(cx).settings.is_bookmarked(&context_root, &absolute_path));
            menu = menu.item(
              PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, if bookmarked { "Remove bookmark" } else { "Add bookmark" }, "bookmark"))
                .disabled(!ready || !context_exists)
                .on_click(move |_, _, cx| {
                  let _ = bookmark_view.update(cx, |this, cx| {
                    if this.root != bookmark_root || !matches!(bookmark_path.try_exists(), Ok(true)) {
                      return;
                    }
                    let added = this.settings.toggle_bookmark(&bookmark_root, &bookmark_path);
                    if this.save_settings() {
                      this.error = false;
                      this.notice = tf(
                        if added { "Bookmark added: {path}" } else { "Bookmark removed: {path}" },
                        &[("path", bookmark_path.strip_prefix(&bookmark_root).unwrap_or(&bookmark_path).display().to_string())],
                      );
                    }
                    cx.notify();
                  });
                }),
            );
            if context_exists && enabled {
              menu = menu.item(PopupMenuItem::new(t("Lock")).disabled(true)).separator();
            }
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
          menu
        }),
    )
  }

  fn row(&self, index: usize, change: &Change, name: &str, depth: usize, window_active: bool, cx: &App) -> Stateful<Div> {
    let rgb = palette(cx);
    let selected = self.selection.paths.contains(&change.path);
    let (selected_background, selected_foreground) = selected_row_palette(cx, window_active);
    let kind = match change.node_type.to_ascii_lowercase().as_str() {
      "directory" | "folder" => "Folder",
      "file" => "File",
      "link" => "Link",
      _ => match std::fs::symlink_metadata(self.root.join(&change.path)) {
        Ok(meta) if meta.file_type().is_symlink() => "Link",
        Ok(meta) if meta.is_dir() => "Folder",
        Ok(meta) if meta.is_file() => "File",
        _ => "Unknown type",
      },
    };
    let state = if change.conflict {
      "Conflict"
    } else if change.staged {
      "Staged"
    } else {
      "Unstaged"
    };
    div()
      .id(("file-row", index))
      .flex()
      .items_center()
      .h(px(25.))
      .px_3()
      .gap_2()
      .border_b_1()
      .border_color(rgb(Divider))
      .bg(if selected { selected_background } else { rgb(PANEL) })
      .when(selected, |row| row.text_color(selected_foreground).font_weight(FontWeight::BOLD))
      .cursor_pointer()
      .when(!selected, |row| row.hover(|style| style.bg(rgb(Hover))))
      .child(
        gpui_component::checkbox::Checkbox::new(("select-change", index))
          .checked(self.selection.paths.contains(&change.path))
          .disabled(self.busy)
          .tab_stop(false),
      )
      .child(div().w(px(depth as f32 * 16.)).flex_shrink_0())
      .child(div().w(px(16.)).flex_shrink_0())
      .child(
        div()
          .w(px(16.))
          .flex_shrink_0()
          .text_color(if selected { selected_foreground } else { rgb(if kind == "Folder" { Warning } else { MUTED }) })
          .child(
            gpui_component::Icon::new(match kind {
              "Folder" => gpui_component::IconName::Folder,
              "Link" => gpui_component::IconName::ExternalLink,
              _ => gpui_component::IconName::File,
            })
            .size(px(16.)),
          ),
      )
      .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().child(name.to_string()))
      .child(
        div()
          .w(px(90.))
          .flex_shrink_0()
          .text_size(px(11.))
          .text_color(if selected {
            selected_foreground
          } else {
            rgb(if matches!(change.action.as_str(), "remove" | "delete") {
              Danger
            } else if matches!(change.action.as_str(), "add" | "create") {
              Success
            } else {
              MUTED
            })
          })
          .child(t(change_action_label(&change.action))),
      )
      .child(
        div()
          .w(px(110.))
          .flex_shrink_0()
          .text_size(px(11.))
          .text_color(if selected {
            selected_foreground
          } else {
            rgb(if state == "Conflict" {
              Danger
            } else if state == "Staged" {
              Success
            } else {
              MUTED
            })
          })
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
    let window_bounds = if settings.window_fullscreen {
      WindowBounds::Fullscreen(bounds)
    } else if settings.window_maximized {
      WindowBounds::Maximized(bounds)
    } else {
      WindowBounds::Windowed(bounds)
    };
    cx.open_window(
      WindowOptions {
        window_bounds: Some(window_bounds),
        window_min_size: Some(size(px(1000.), px(700.))),
        titlebar: Some(TitlebarOptions {
          title: Some(t("LoreLens — Desktop repository client").into()),
          ..TitleBar::title_bar_options()
        }),
        ..Default::default()
      },
      move |window, cx| {
        let view = cx.new(|cx| Lens::new(root, settings, settings_error, window, cx));
        view.update(cx, |this, cx| {
          cx.observe_window_bounds(window, |this, window, _| {
            let maximized = window.is_maximized();
            let fullscreen = window.is_fullscreen();
            if this.settings.window_maximized != maximized || this.settings.window_fullscreen != fullscreen {
              this.settings.window_maximized = maximized;
              this.settings.window_fullscreen = fullscreen;
              this.save_settings();
            }
          })
          .detach();
          cx.observe_in(&cx.entity(), window, |this, _, window, cx| {
            this.conflict_dialog_working.set(this.busy);
            if !this.busy {
              this.progress_popup_dismissed = false;
            }
            #[cfg(windows)]
            if this.cli_install_pending && !this.busy {
              this.cli_install_pending = false;
              this.install_cli_dialog(window, cx);
              return;
            }
            if this.startup_login_pending && !this.busy && !window.has_active_dialog(cx) {
              this.startup_login_pending = false;
              this.login_dialog(window, cx);
              return;
            }
            let has_conflicts = this.status.changes.iter().any(|change| change.conflict);
            if this.conflict_dialog_open && !has_conflicts {
              this.conflict_dialog_open = false;
              if window.has_active_dialog(cx) {
                // Closing invokes the dialog's on_close callback. Defer it until
                // this Lens update has released its borrow to avoid reentrancy.
                window.defer(cx, |window, cx| {
                  if window.has_active_dialog(cx) {
                    window.close_dialog(cx);
                  }
                });
              }
              return;
            }
            if !this.busy && this.conflict_dialog_pending && !window.has_active_dialog(cx) {
              this.merge_conflicts_dialog(window, cx);
            }
          })
          .detach();
          #[cfg(windows)]
          {
            this.cli_install_pending = cli_install::cli_missing(&this.cli);
          }
          this.check_repository_login(cx);
          let mut was_active = window.is_window_active();
          cx.observe_window_activation(window, move |this, window, cx| {
            let active = window.is_window_active();
            if active && !was_active && this.connected {
              this.refresh_with_mode(true, cx);
            }
            if active != was_active {
              was_active = active;
              cx.notify();
            }
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
