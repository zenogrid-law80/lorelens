use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommandKind {
  Login,
  Logout,
  Account,
  ListBranches,
  ArchiveBranch,
  Sync,
  SwitchBranch,
  Merge,
  Commit,
  Obliterate,
  Other,
}

impl CommandKind {
  fn from_args(args: &[String]) -> Self {
    match (args.first().map(String::as_str), args.get(1).map(String::as_str)) {
      (Some("login"), _) => Self::Login,
      (Some("auth"), Some("clear")) => Self::Logout,
      (Some("auth"), Some("info")) => Self::Account,
      (Some("branch"), Some("list")) => Self::ListBranches,
      (Some("branch"), Some("archive")) => Self::ArchiveBranch,
      (Some("sync"), _) => Self::Sync,
      (Some("branch"), Some("switch")) => Self::SwitchBranch,
      (Some("branch"), Some("merge")) => Self::Merge,
      (Some("commit"), _) => Self::Commit,
      (Some("file"), Some("obliterate")) => Self::Obliterate,
      _ => Self::Other,
    }
  }
  fn is_authentication(self) -> bool {
    matches!(self, Self::Login | Self::Account)
  }
  fn changes_worktree(self) -> bool {
    matches!(self, Self::Sync | Self::SwitchBranch | Self::Merge)
  }
}

pub(crate) fn switch_branch_args(branch: String) -> Vec<String> {
  vec!["branch".into(), "switch".into(), "--".into(), branch]
}

struct CommandResult {
  result: Result<String, String>,
  entries: Option<Result<Vec<Entry>, String>>,
  locks: Option<Result<std::collections::HashSet<String>, String>>,
  identity_update: Option<Result<(), String>>,
  pending_push: Option<Result<Vec<backend::LocalCommit>, String>>,
  pending_pull: Option<Result<Vec<backend::LocalCommit>, String>>,
}

#[cfg(test)]
mod tests {
  use super::{CommandKind, switch_branch_args};
  #[test]
  fn command_behavior_is_derived_from_arguments() {
    let cases = [
      (vec!["login"], CommandKind::Login),
      (vec!["auth", "clear"], CommandKind::Logout),
      (vec!["auth", "info"], CommandKind::Account),
      (vec!["branch", "list"], CommandKind::ListBranches),
      (vec!["branch", "archive", "topic"], CommandKind::ArchiveBranch),
      (vec!["branch", "switch", "--", "topic"], CommandKind::SwitchBranch),
      (vec!["branch", "merge"], CommandKind::Merge),
      (vec!["sync"], CommandKind::Sync),
      (vec!["commit"], CommandKind::Commit),
      (vec!["file", "obliterate", "--path=file"], CommandKind::Obliterate),
      (vec!["history"], CommandKind::Other),
      (vec![], CommandKind::Other),
    ];
    for (args, expected) in cases {
      let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
      assert_eq!(CommandKind::from_args(&args), expected);
    }
    assert!(CommandKind::Account.is_authentication());
    assert!(CommandKind::SwitchBranch.changes_worktree());
    assert!(!CommandKind::ListBranches.changes_worktree());
  }

  #[test]
  fn branch_switch_can_fetch_a_missing_local_latest_from_remote() {
    let args = switch_branch_args("feat-account-login".into());
    assert_eq!(args, ["branch", "switch", "--", "feat-account-login"]);
    assert!(!args.iter().any(|arg| arg == "--local"));
  }
}

impl Lens {
  pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
    self.refresh_with_mode(false, cx);
  }

  pub(super) fn refresh_with_mode(&mut self, silent: bool, cx: &mut Context<Self>) {
    if self.root.as_os_str().is_empty() {
      return;
    }
    if self.busy {
      self.pending_refresh_silent = silent && (!self.refresh_pending || self.pending_refresh_silent);
      self.refresh_pending = true;
      return;
    }
    self.refresh_pending = false;
    self.pending_refresh_silent = false;
    self.next_refresh = std::time::Instant::now() + std::time::Duration::from_secs(30);
    if !backend::is_repository(&self.root) {
      self.connected = false;
      self.load_directory(cx);
      self.silent_refresh = silent && self.busy;
      return;
    }
    self.command_with_mode(vec!["status".into(), "--scan".into()], "Repository status", true, false, silent, cx);
  }

  pub(super) fn command(&mut self, args: Vec<String>, title: &str, status: bool, mutation: bool, cx: &mut Context<Self>) {
    self.command_batch_with_mode(vec![args], title, status, mutation, false, cx);
  }

  pub(super) fn command_batch(&mut self, commands: Vec<Vec<String>>, title: &str, status: bool, mutation: bool, cx: &mut Context<Self>) {
    self.command_batch_with_mode(commands, title, status, mutation, false, cx);
  }

  fn command_with_mode(&mut self, args: Vec<String>, title: &str, status: bool, mutation: bool, silent: bool, cx: &mut Context<Self>) {
    self.command_batch_with_mode(vec![args], title, status, mutation, silent, cx);
  }

  fn command_batch_with_mode(&mut self, commands: Vec<Vec<String>>, title: &str, status: bool, mutation: bool, silent: bool, cx: &mut Context<Self>) {
    let Some(args) = commands.first().cloned() else {
      return;
    };
    if self.busy {
      return;
    }
    #[cfg(windows)]
    if cli_install::cli_missing(&self.cli) {
      self.cli_install_pending = true;
      cx.notify();
      return;
    }
    self.preview.invalidate();
    self.busy = true;
    self.silent_refresh = silent;
    self.error = false;
    if !silent {
      let command = if commands.len() == 1 { args.join(" ") } else { title.to_owned() };
      self.notice = tf("Running {command}…", &[("command", command)]);
      for command in &commands {
        self.log(format!("lore {}", command.join(" ")));
      }
    }
    let root = self.root.clone();
    let cli = self.cli.clone();
    let expanded = self.expanded_folders.clone();
    let title = title.to_string();
    let identity = self.settings.identity.clone();
    let text_line_ending = self.settings.text_line_ending.clone();
    let text_encoding = self.settings.text_encoding.clone();
    let text_extensions = self.settings.text_extensions.clone();
    let kind = CommandKind::from_args(&args);
    let resets_files = commands.iter().any(|args| args.first().is_some_and(|arg| arg == "reset"));
    let authentication = kind.is_authentication();
    let login = kind == CommandKind::Login;
    let login_remote = if login { args.get(1).cloned() } else { None };
    let branches = kind == CommandKind::ListBranches;
    let task = cx.background_executor().spawn(async move {
      let (reset_placeholders, prepare_error) = if resets_files {
        match backend::prepare_reset_paths(&root, &commands) {
          Ok(paths) => (paths, None),
          Err(error) => (Vec::new(), Some(error)),
        }
      } else {
        (Vec::new(), None)
      };
      let result = prepare_error.map_or_else(
        || {
          (|| {
            let mut outputs = Vec::new();
            for args in commands {
              if args.first().is_some_and(|arg| arg == "stage") {
                let separator = args.iter().position(|arg| arg == "--").map_or(1, |index| index + 1);
                if let Err(error) = backend::validate_text_files(&root, &args[separator..], &text_extensions, &text_line_ending, &text_encoding) {
                  return Err(format!("[stage-validation] {error}"));
                }
              }
              let branch_switch = args.first().is_some_and(|arg| arg == "branch") && args.get(1).is_some_and(|arg| arg == "switch");
              let result = if branch_switch {
                backend::run_branch_switch_skipping_unavailable(&cli, &root, &args, if login { None } else { identity.as_deref() })
              } else {
                backend::run_as(&cli, &root, &args, status || authentication || branches, if login { None } else { identity.as_deref() })
              };
              match result {
                Ok(output) => {
                  outputs.push(output);
                  if args.len() == 5 && args[0] == "branch" && args[1] == "merge" && args[2] == "resolve" && args[3] == "--" {
                    backend::cleanup_resolved_merge(&cli, &root, &args[4], identity.as_deref())?;
                  }
                }
                Err(error) => return Err(format!("{}\n{}: {error}", outputs.join("\n"), args.join(" "))),
              }
            }
            Ok(outputs.join("\n"))
          })()
        },
        Err,
      );
      if result.is_err() {
        backend::cleanup_reset_paths(&reset_placeholders);
      }
      let identity_update = if authentication && backend::is_repository(&root) {
        result
          .as_ref()
          .ok()
          .and_then(|output| backend::parse_account(output).ok())
          .map(|(id, _)| backend::update_repository_identity(&root, &id))
      } else {
        None
      };
      let entries = status.then(|| backend::list_tree(&root, &expanded));
      let pending_push = status.then(|| {
        result
          .as_ref()
          .map_err(|e| e.clone())
          .and_then(|output| backend::pending_push(&cli, &root, output, identity.as_deref()))
      });
      let pending_pull = status.then(|| {
        result
          .as_ref()
          .map_err(|e| e.clone())
          .and_then(|output| backend::pending_pull(&cli, &root, output, identity.as_deref()))
      });
      let locks = status.then(|| {
        let state = backend::parse_status(result.as_ref().map_err(|e| e.clone())?)?;
        backend::run_as(&cli, &root, &["lock".into(), "query".into(), "--branch".into(), state.branch], true, identity.as_deref()).and_then(|output| backend::parse_locked_paths(&output))
      });
      CommandResult {
        result,
        entries,
        locks,
        identity_update,
        pending_push,
        pending_pull,
      }
    });
    cx.spawn(async move |this, cx| {
      let CommandResult {
        result,
        entries,
        locks,
        identity_update,
        pending_push,
        pending_pull,
      } = task.await;
      let _ = this.update(cx, |this, cx| {
        let silent_refresh = this.silent_refresh;
        this.busy = false;
        this.silent_refresh = false;
        if let Some(pending_pull) = pending_pull {
          this.pending_pull = pending_pull;
        }
        if let Some(pending_push) = pending_push {
          this.pending_push = pending_push;
        }
        if let Some(locks) = locks {
          this.locked_paths.clear();
          match locks {
            Ok(paths) => this.locked_paths = paths,
            Err(error) if !silent_refresh => this.log(format!("File lock indicators unavailable: {error}")),
            Err(_) => {}
          }
        }
        if let Some(entries) = entries {
          match entries {
            Ok(entries) => this.entries = entries,
            Err(error) if !silent_refresh => this.log(format!("Folder refresh failed: {error}")),
            Err(_) => {}
          }
        }
        match result {
          Ok(output) => {
            if status {
              match backend::parse_status(&output) {
                Ok(state) => {
                  this.status = state;
                  this.connected = true;
                  if !silent_refresh {
                    this.notice = tf("Status refreshed · {count} pending files", &[("count", this.status.changes.len().to_string())]);
                  }
                }
                Err(e) => {
                  this.connected = false;
                  this.status = Status::default();
                  this.error = true;
                  this.notice = e;
                }
              }
            } else if kind == CommandKind::ListBranches {
              this.local_branches.clear();
              this.remote_branches.clear();
              let mut lines = Vec::new();
              for line in output.lines() {
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(line)
                  && event["tagName"] == "branchListEntry"
                {
                  let data = &event["data"];
                  if let Some(name) = data["name"].as_str() {
                    let location = data["location"].as_str().unwrap_or("unknown");
                    let archived = data["archived"].as_bool().unwrap_or(false);
                    if location == "local" && !archived {
                      this.local_branches.push(name.into());
                    }
                    if location == "remote" && !archived {
                      this.remote_branches.push(name.into());
                    }
                    lines.push(format!("{} {} ({location})", if data["isCurrent"] == true { "*" } else { " " }, name));
                  }
                }
              }
              this.local_branches.sort();
              this.local_branches.dedup();
              this.remote_branches.sort();
              this.remote_branches.dedup();
              this.branch_output = lines.join("\n");
              if !silent_refresh {
                this.notice = "Branches loaded".into();
              }
              this.command_with_mode(vec!["auth".into(), "info".into()], "Account", false, false, silent_refresh, cx);
            } else if kind == CommandKind::Logout {
              this.settings.identity = None;
              this.settings.login_remote = None;
              this.logged_in_account = "Not signed in".into();
              this.startup_login_pending = false;
              this.refresh_pending = false;
              this.connected = false;
              this.notice = "Logged out".into();
              this.output_title = "Logout".into();
              this.output = t("Logged out");
              this.save_settings();
            } else if kind == CommandKind::Login {
              this.settings.login_remote = login_remote;
              this.save_settings();
              this.notice = "Login completed. Open or clone a repository.".into();
              this.output = t(&this.notice);
              this.output_title = "Login".into();
            } else if kind.is_authentication() {
              match backend::parse_account(&output) {
                Ok((id, name)) => {
                  this.logged_in_account = name;
                  this.settings.identity = Some(id);
                  this.save_settings();
                  this.notice = "Account loaded".into();
                  if let Some(Err(error)) = &identity_update {
                    this.error = true;
                    this.notice = "Account loaded; repository identity update failed".into();
                    this.log(error.clone());
                  }
                }
                Err(error) => {
                  this.logged_in_account = "Account unavailable".into();
                  this.log(error);
                }
              }
            } else {
              let fallback_command = output
                .lines()
                .find_map(|line| line.strip_prefix("Fallback: ").or_else(|| line.strip_prefix("Fallback failed: ")))
                .map(str::to_owned);
              this.output = if output.trim().is_empty() { "Command completed successfully.".into() } else { output };
              this.output_title = title.clone();
              this.notice = tf("{title} completed", &[("title", t(&title))]);
              if let Some(command) = fallback_command {
                this.log(command);
              }
            }
            if !silent_refresh {
              this.log(this.notice.clone());
            }
            if kind == CommandKind::Login && backend::is_repository(&this.root) {
              this.refresh(cx);
            }
            if status && this.connected {
              this.command_with_mode(vec!["branch".into(), "list".into()], "Branches", false, false, silent_refresh, cx);
            }
            if mutation {
              if kind.changes_worktree() {
                this.selection.clear();
                this.file_history = history::FileHistoryState::default();
                if !this.directory.is_dir() {
                  this.directory = this.root.clone();
                }
              }
              if kind == CommandKind::Commit {
                this.message.update(cx, |input, cx| {
                  input.reset();
                  cx.notify();
                });
              }
              this.refresh(cx);
            }
          }
          Err(e) => {
            let validation = e.strip_prefix("[stage-validation] ").map(str::to_owned);
            let e = validation.clone().unwrap_or(e);
            if status {
              this.connected = false;
              this.status = Status::default();
            }
            if kind.is_authentication() {
              this.logged_in_account = "Account unavailable".into();
            }
            this.error = true;
            this.notice = validation.unwrap_or_else(|| "Command failed · see details".into());
            this.output_title = tf("{title} failed", &[("title", t(&title))]);
            this.output = if e.contains("Invalid or expired authentication") {
              format!("{e}\n\nAuthentication expired. Run `lore login` for this repository, then refresh.")
            } else {
              e.clone()
            };
            this.log(e);
            if resets_files || matches!(kind, CommandKind::ArchiveBranch | CommandKind::SwitchBranch | CommandKind::Merge | CommandKind::Obliterate) {
              this.refresh_pending = true;
              this.pending_refresh_silent = false;
            }
          }
        }
        cx.notify();
      });
    })
    .detach();
    cx.notify();
  }
}
