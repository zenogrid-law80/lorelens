use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommandKind {
    Login,
    Logout,
    Account,
    ListBranches,
    Sync,
    SwitchBranch,
    Merge,
    Commit,
    Obliterate,
    Other,
}

impl CommandKind {
    fn from_args(args: &[String]) -> Self {
        match (
            args.first().map(String::as_str),
            args.get(1).map(String::as_str),
        ) {
            (Some("login"), _) => Self::Login,
            (Some("auth"), Some("clear")) => Self::Logout,
            (Some("auth"), Some("info")) => Self::Account,
            (Some("branch"), Some("list")) => Self::ListBranches,
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

struct CommandResult {
    result: Result<String, String>,
    entries: Option<Result<Vec<Entry>, String>>,
    locks: Option<Result<std::collections::HashSet<String>, String>>,
    identity_update: Option<Result<(), String>>,
    pending_push: Option<Result<Vec<String>, String>>,
}

#[cfg(test)]
mod tests {
    use super::CommandKind;
    #[test]
    fn command_behavior_is_derived_from_arguments() {
        let cases = [
            (vec!["login"], CommandKind::Login),
            (vec!["auth", "clear"], CommandKind::Logout),
            (vec!["auth", "info"], CommandKind::Account),
            (vec!["branch", "list"], CommandKind::ListBranches),
            (
                vec!["branch", "switch", "--local"],
                CommandKind::SwitchBranch,
            ),
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
}

impl Lens {
    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            self.refresh_pending = true;
            return;
        }
        self.refresh_pending = false;
        if !backend::is_repository(&self.root) {
            self.connected = false;
            self.load_directory(cx);
            return;
        }
        self.command(
            vec!["status".into(), "--scan".into()],
            "Repository status",
            true,
            false,
            cx,
        );
    }

    pub(super) fn command(
        &mut self,
        args: Vec<String>,
        title: &str,
        status: bool,
        mutation: bool,
        cx: &mut Context<Self>,
    ) {
        self.command_batch(vec![args], title, status, mutation, cx);
    }

    pub(super) fn command_batch(
        &mut self,
        commands: Vec<Vec<String>>,
        title: &str,
        status: bool,
        mutation: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(args) = commands.first().cloned() else { return; };
        if self.busy { return; }
        #[cfg(windows)]
        if cli_install::cli_missing(&self.cli) {
            self.cli_install_pending = true;
            cx.notify();
            return;
        }
        self.preview.invalidate();
        self.busy = true;
        self.error = false;
        self.notice = tf("Running {command}…", &[("command", args.join(" "))]);
        self.log(format!("lore {}", args.join(" ")));
        let root = self.root.clone();
        let cli = self.cli.clone();
        let expanded = self.expanded_folders.clone();
        let title = title.to_string();
        let identity = self.settings.identity.clone();
        let kind = CommandKind::from_args(&args);
        let authentication = kind.is_authentication();
        let login = kind == CommandKind::Login;
        let login_remote = if login { args.get(1).cloned() } else { None };
        let branches = kind == CommandKind::ListBranches;
        let task = cx.background_executor().spawn(async move {
            let result = (|| {
                let mut outputs = Vec::new();
                for args in commands {
                    match backend::run_as(&cli, &root, &args, status || authentication || branches,
                        if login { None } else { identity.as_deref() }) {
                        Ok(output) => outputs.push(output),
                        Err(error) => return Err(format!("{}\n{}: {error}", outputs.join("\n"), args.join(" "))),
                    }
                }
                Ok(outputs.join("\n"))
            })();
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
                result.as_ref().map_err(|e| e.clone()).and_then(|output| {
                    backend::pending_push(&cli, &root, output, identity.as_deref())
                })
            });
            let locks = status.then(|| {
                let state = backend::parse_status(result.as_ref().map_err(|e| e.clone())?)?;
                backend::run_as(
                    &cli,
                    &root,
                    &[
                        "lock".into(),
                        "query".into(),
                        "--branch".into(),
                        state.branch,
                    ],
                    true,
                    identity.as_deref(),
                )
                .and_then(|output| backend::parse_locked_paths(&output))
            });
            CommandResult {
                result,
                entries,
                locks,
                identity_update,
                pending_push,
            }
        });
        cx.spawn(async move |this, cx| {
            let CommandResult { result, entries, locks, identity_update, pending_push } = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if let Some(pending_push) = pending_push { this.pending_push = pending_push; }
                if let Some(locks) = locks {
                    this.locked_paths.clear();
                    match locks {
                        Ok(paths) => this.locked_paths = paths,
                        Err(error) => this.log(format!("File lock indicators unavailable: {error}")),
                    }
                }
                if let Some(entries) = entries {
                    match entries {
                        Ok(entries) => this.entries = entries,
                        Err(error) => this.log(format!("Folder refresh failed: {error}")),
                    }
                }
                match result {
                    Ok(output) => {
                        if status {
                            match backend::parse_status(&output) {
                                Ok(state) => {
                                    this.status = state;
                                    this.connected = true;
                                    this.notice = tf("Status refreshed · {count} pending files",
                                        &[("count", this.status.changes.len().to_string())]);
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
                                if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
                                    if event["tagName"] == "branchListEntry" {
                                        let data = &event["data"];
                                        if let Some(name) = data["name"].as_str() {
                                            let location = data["location"].as_str().unwrap_or("unknown");
                                            let archived = data["archived"].as_bool().unwrap_or(false);
                                            if location == "local" && !archived { this.local_branches.push(name.into()); }
                                            if location == "remote" && !archived { this.remote_branches.push(name.into()); }
                                            lines.push(format!("{} {} ({location})", if data["isCurrent"] == true { "*" } else { " " }, name));
                                        }
                                    }
                                }
                            }
                            this.local_branches.sort();
                            this.local_branches.dedup();
                            this.remote_branches.sort();
                            this.remote_branches.dedup();
                            this.branch_output = lines.join("\n");
                            this.notice = "Branches loaded".into();
                            this.command(vec!["auth".into(), "info".into()], "Account", false, false, cx);
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
                            this.show_log = false;
                            this.save_settings();
                        } else if kind == CommandKind::Login {
                            this.settings.login_remote = login_remote;
                            this.save_settings();
                            this.notice = "Login completed. Open or clone a repository.".into();
                            this.output = t(&this.notice);
                            this.output_title = "Login".into();
                            this.show_log = false;
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
                            this.output = if output.trim().is_empty() {
                                "Command completed successfully.".into()
                            } else {
                                output
                            };
                            this.output_title = title.clone();
                            this.show_log = false;
                            this.notice = tf("{title} completed", &[("title", t(&title))]);
                        }
                        this.log(this.notice.clone());
                        if kind == CommandKind::Login {
                            if backend::is_repository(&this.root) {
                                this.refresh(cx);
                            }
                        }
                        if status && this.connected {
                            this.command(
                                vec!["branch".into(), "list".into()],
                                "Branches",
                                false,
                                false,
                                cx,
                            );
                        }
                        if mutation {
                            if kind.changes_worktree() {
                                this.selection.clear();
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
                        if status {
                            this.connected = false;
                            this.status = Status::default();
                        }
                        if kind.is_authentication() {
                            this.logged_in_account = "Account unavailable".into();
                        }
                        this.error = true;
                        this.notice = "Command failed · see details".into();
                        this.output_title = tf("{title} failed", &[("title", t(&title))]);
                        this.output = if e.contains("Invalid or expired authentication") {
                            format!(
                                "{e}\n\nAuthentication expired. Run `lore login` for this repository, then refresh."
                            )
                        } else {
                            e.clone()
                        };
                        this.show_log = false;
                        this.log(e);
                        if matches!(kind, CommandKind::Merge | CommandKind::Obliterate) {
                            this.refresh_pending = true;
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
