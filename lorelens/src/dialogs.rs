use super::*;

impl Lens {
    pub(super) fn obliterate_dialog(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || !self.connected { return; }
        if let Err(error) = backend::obliterate_args(&self.root, &path) {
            self.log(error);
            cx.notify();
            return;
        }
        let root = self.root.clone();
        let branch = self.status.branch.clone();
        let revision = self.status.revision.clone();
        let identity = self.settings.identity.clone();
        let confirmation = cx.new(|cx| TextInput::new("Type OBLITERATE to confirm", cx));
        let validation = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let (path, root, branch, revision, identity) =
                (path.clone(), root.clone(), branch.clone(), revision.clone(), identity.clone());
            let input = confirmation.clone();
            let view = view.clone();
            let validation = validation.clone();
            let error = t(&validation.borrow());
            dialog.title(t("Obliterate file")).confirm()
                .button_props(DialogButtonProps::default().cancel_text(t("Cancel")).ok_text(t("Obliterate")))
                .child(div().flex().flex_col().gap_2()
                    .child(root.display().to_string())
                    .child(path.clone())
                    .child(t("Permanently remove the stored content of this file, delete its local copy, and stage its deletion. This cannot be undone and may affect the remote repository and revisions sharing this content. Other historical versions are not automatically removed."))
                    .child(t("Type OBLITERATE to confirm permanent removal."))
                    .child(confirmation.clone())
                    .child(error))
                .on_ok(move |_, window, cx| {
                    if input.read(cx).content.trim() != "OBLITERATE" {
                        *validation.borrow_mut() = "Enter OBLITERATE to continue.".into();
                        window.refresh();
                        return false;
                    }
                    view.update(cx, |this, cx| {
                        if this.busy || !this.connected || this.root != root || this.status.branch != branch
                            || this.status.revision != revision || this.settings.identity != identity {
                            *validation.borrow_mut() = "Repository state changed. Reopen this dialog.".into();
                            window.refresh();
                            return false;
                        }
                        match backend::obliterate_args(&root, &path) {
                            Ok(args) => {
                                this.selection.clear();
                                this.command(args, "Obliterate file", false, true, cx);
                                true
                            }
                            Err(error) => {
                                *validation.borrow_mut() = error;
                                window.refresh();
                                false
                            }
                        }
                    }).unwrap_or(false)
                })
        });
    }

    pub(super) fn create_repository_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy { return; }
        let url = cx.new(|cx| TextInput::new("lores://server:port/repository", cx));
        let destination = cx.new(|cx| TextInput::new("Absolute destination path", cx));
        url.update(cx, |input, _| input.content = self.settings.create_url.clone().into());
        destination.update(cx, |input, _| input.content = self.settings.create_destination.clone().into());
        let urls = self.settings.create_urls.clone();
        let destinations = self.settings.create_destinations.clone();
        let validation = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let url_input = url.clone();
            let destination_input = destination.clone();
            let validation = validation.clone();
            let validation_text = t(&validation.borrow());
            let view = view.clone();
            let close_view = view.clone();
            let close_url = url.clone();
            let close_destination = destination.clone();
            dialog.title(t("Create repository")).confirm()
                .button_props(DialogButtonProps::default().cancel_text(t("Cancel")).ok_text(t("Create")))
                .child(div().flex().flex_col().gap_2()
                    .child(t("Create a remote repository and its local workspace. Log in to the server first."))
                    .child(t("Repository URL"))
                    .child(Self::history_input("create-url-history", url.clone(), urls.clone()))
                    .child(t("Destination Path"))
                    .child(Self::history_input("create-path-history", destination.clone(), destinations.clone()))
                    .child(validation_text))
                .on_close(move |_, _, cx| {
                    let _ = close_view.update(cx, |this, cx| {
                        this.settings.remember_create(
                            &close_url.read(cx).content,
                            &close_destination.read(cx).content,
                        );
                        this.save_settings();
                    });
                })
                .on_ok(move |_, window, cx| {
                    let remote = url_input.read(cx).content.trim().to_owned();
                    let path = PathBuf::from(destination_input.read(cx).content.trim());
                    let error = if !remote.contains("://") || remote.ends_with("://") || remote.chars().any(char::is_whitespace) {
                        Some("Enter a repository URL including its scheme and repository name.")
                    } else if !path.is_absolute() {
                        Some("Enter an absolute destination path.")
                    } else { None };
                    if let Some(error) = error {
                        *validation.borrow_mut() = error.into();
                        window.refresh();
                        return false;
                    }
                    view.update(cx, |this, cx| {
                        if this.busy { return false; }
                        this.settings.remember_create(&remote, &destination_input.read(cx).content);
                        if !this.save_settings() {
                            *validation.borrow_mut() = "Could not save settings · see command log".into();
                            window.refresh();
                            return false;
                        }
                        this.busy = true;
                        this.error = false;
                        this.notice = "Creating repository…".into();
                        let cli = this.cli.clone();
                        let identity = this.settings.identity.clone();
                        let target = path.clone();
                        let task = cx.background_executor().spawn(async move {
                            backend::create_repository(&cli, &remote, &target, identity.as_deref())
                        });
                        cx.spawn(async move |this, cx| {
                            let result = task.await;
                            let _ = this.update(cx, |this, cx| {
                                this.busy = false;
                                this.show_log = false;
                                match result {
                                    Ok(output) => {
                                        this.log(output);
                                        this.open_repository(path, cx);
                                    }
                                    Err(error) => {
                                        this.error = true;
                                        this.notice = "Create failed — see details".into();
                                        this.output_title = "Create failed".into();
                                        this.output = t(&error);
                                        this.log(error);
                                    }
                                }
                                cx.notify();
                            });
                        }).detach();
                        cx.notify();
                        true
                    }).unwrap_or(false)
                })
        });
    }

    pub(super) fn revert_dialog(&mut self, path: String, unstage_only: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || !self.connected { return; }
        let Some(change) = self.status.changes.iter().find(|change| change.path == path) else { return; };
        if unstage_only && !change.staged { return; }
        let root = self.root.clone();
        let revision = self.status.revision.clone();
        let branch = self.status.branch.clone();
        let view = cx.entity().downgrade();
        let title = if unstage_only { "Unstage" } else { "Revert file" };
        let validation = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        window.open_dialog(cx, move |dialog, _, _| {
            let path = path.clone();
            let root = root.clone();
            let revision = revision.clone();
            let branch = branch.clone();
            let view = view.clone();
            let validation = validation.clone();
            let error = t(&validation.borrow());
            dialog.title(t(title)).confirm()
                .button_props(DialogButtonProps::default().cancel_text(t("Cancel")).ok_text(t(title)))
                .child(div().flex().flex_col().gap_2()
                    .child(root.join(&path).display().to_string())
                    .child(t(if unstage_only {
                        "Remove this file from staging? Local file changes will be kept."
                    } else {
                        "Restore this file to the current committed revision? Deleted files will be restored and staged changes discarded. Local edits will be lost; newly added files will be deleted."
                    }))
                    .child(error))
                .on_ok(move |_, window, cx| {
                    view.update(cx, |this, cx| {
                        if this.busy || !this.connected || this.root != root || this.status.branch != branch
                            || this.status.revision != revision
                            || !this.status.changes.iter().any(|c| c.path == path && (!unstage_only || c.staged)) {
                            *validation.borrow_mut() = "Repository state changed. Reopen this dialog.".into();
                            window.refresh();
                            return false;
                        }
                        let args = if unstage_only {
                            vec!["unstage".into(), "--".into(), path.clone()]
                        } else {
                            vec!["reset".into(), "--purge".into(), "--".into(), path.clone()]
                        };
                        this.selection.clear();
                        this.command(args, title, false, true, cx);
                        true
                    }).unwrap_or(true)
                })
        });
    }
}

impl Lens {
    pub(super) fn delete_dialog(&mut self, source: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy { return; }
        let root = self.root.clone();
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let source = source.clone();
            let root = root.clone();
            let view = view.clone();
            dialog.title(t("Delete")).confirm()
                .button_props(DialogButtonProps::default().cancel_text(t("Cancel")).ok_text(t("Delete")))
                .child(div().flex().flex_col().gap_2()
                    .child(source.display().to_string())
                    .child(t("Permanently delete this item? Folders and all their contents will be deleted. This does not use the Recycle Bin.")))
                .on_ok(move |_, _, cx| {
                    let root = root.clone();
                    let source = source.clone();
                    view.update(cx, |this, cx| {
                        if this.busy || this.root != root { return false; }
                        this.busy = true;
                        this.preview.invalidate();
                        this.notice = "Deleting…".into();
                        let task = cx.background_executor().spawn(async move {
                            backend::delete_entry(&root, &source)
                        });
                        cx.spawn(async move |this, cx| {
                            let result = task.await;
                            let _ = this.update(cx, |this, cx| {
                                this.busy = false;
                                this.show_log = false;
                                this.selection.clear();
                                match result {
                                    Ok(()) => {
                                        this.error = false;
                                        this.output_title = "Delete completed".into();
                                        this.output = t("Item deleted. Refreshing file list and repository status.");
                                        this.notice = "Delete completed".into();
                                    }
                                    Err(error) => {
                                        this.error = true;
                                        this.notice = "Delete failed — see details".into();
                                        this.output_title = "Delete failed".into();
                                        this.output = t(&error);
                                        this.log(error);
                                    }
                                }
                                // A failed recursive delete can still have removed some children.
                                this.refresh(cx);
                                cx.notify();
                            });
                        }).detach();
                        cx.notify();
                        true
                    }).unwrap_or(true)
                })
        });
    }
}

impl Lens {
    pub(super) fn logout_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy { return; }
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let view = view.clone();
            dialog.title(t("Logout")).confirm()
                .button_props(DialogButtonProps::default().cancel_text(t("Cancel")).ok_text(t("Logout")))
                .child(t("Log out all Lore CLI accounts on this device? URL history and local repositories will be kept."))
                .on_ok(move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        if this.busy { return false; }
                        this.command(vec!["auth".into(), "clear".into()], "Logout", false, false, cx);
                        true
                    }).unwrap_or(false)
                })
        });
    }

    pub(super) fn login_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy { return; }
        let no_repository = !backend::is_repository(&self.root);
        let history = self.settings.login_urls.clone();
        let url = cx.new(|cx| {
            let mut input = TextInput::new("lores://server:port", cx);
            input.content = history.first().cloned().unwrap_or_default().into();
            input
        });
        let view = cx.entity().downgrade();
        let validation = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        window.open_dialog(cx, move |dialog, _, _| {
            let input = url.clone();
            let view = view.clone();
            let validation = validation.clone();
            let validation_text = t(&validation.borrow());
            dialog.title(t("Login")).confirm()
                .button_props(DialogButtonProps::default().cancel_text(t("Cancel")).ok_text(t("Login")))
                .child(div().flex().flex_col().gap_2()
                    .when(no_repository, |el| el.child(t("No local repository. Enter the server URL to log in, then clone a repository.")))
                    .child(t("Server URL"))
                    .child(Self::history_input("login-remote-url", url.clone(), history.clone()))
                    .child(validation_text))
                .on_ok(move |_, window, cx| {
                    let remote = input.read(cx).content.trim().to_owned();
                    if !remote.contains("://") || remote.ends_with("://") || remote.chars().any(char::is_whitespace) {
                        *validation.borrow_mut() = "Enter a server URL including its scheme.".into();
                        window.refresh();
                        return false;
                    }
                    view.update(cx, |this, cx| {
                        if this.busy { return false; }
                        this.settings.remember_login(&remote);
                        if !this.save_settings() {
                            *validation.borrow_mut() = "Could not save settings · see command log".into();
                            window.refresh();
                            return false;
                        }
                        this.command(vec!["login".into(), remote], "Login", false, false, cx);
                        true
                    }).unwrap_or(false)
                })
        });
    }
    pub(super) fn merge_branch_dialog(
        &mut self,
        source: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || !self.connected {
            return;
        }
        let name = cx.new(|cx| {
            let mut input = TextInput::new("Select source branch", cx);
            input.content = source.into();
            input
        });
        let target = self.status.branch.clone();
        let branches: Vec<String> = self
            .local_branches
            .iter()
            .filter(|branch| **branch != target)
            .cloned()
            .collect();
        let root = self.root.clone();
        let view = cx.entity().downgrade();
        let validation = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        window.open_dialog(cx, move |dialog, _, _| {
            let input = name.clone();
            let view = view.clone();
            let root = root.clone();
            let target = target.clone();
            let error = validation.borrow().clone();
            let validation = validation.clone();
            dialog.title(t("Merge local branch")).confirm()
                .button_props(DialogButtonProps::default().cancel_text(t("Cancel")).ok_text(t("Merge")))
                .child(tf("Target (current branch): {target}", &[("target", target.to_string())]))
                .child(t("Source branch"))
                .child(Self::history_input("merge-source-branch", name.clone(), branches.clone()))
                .child(t("A conflict-free merge creates a local commit automatically. Conflicts are shown in Details."))
                .child(t(&error))
                .on_ok(move |_, window, cx| {
                    let branch = input.read(cx).content.trim().to_string();
                    view.update(cx, |this, cx| {
                        if this.busy || this.root != root || this.status.branch != target {
                            *validation.borrow_mut() = "Repository state changed. Reopen this dialog.".into();
                            window.refresh();
                            return false;
                        }
                        if branch == target || !this.local_branches.contains(&branch) {
                            *validation.borrow_mut() = "Select a different local source branch.".into();
                            window.refresh();
                            return false;
                        }
                        this.command(vec!["branch".into(), "merge".into(), "--".into(), branch],
                            "Merge local branch", false, true, cx);
                        true
                    }).unwrap_or(true)
                })
        });
    }

    pub(super) fn switch_branch_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || !self.connected {
            return;
        }
        let name = cx.new(|cx| TextInput::new("Select a local branch", cx));
        let branches = self.local_branches.clone();
        let root = self.root.clone();
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let input = name.clone();
            let view = view.clone();
            let root = root.clone();
            dialog
                .title(t("Switch local branch"))
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .cancel_text(t("Cancel"))
                        .ok_text(t("Switch")),
                )
                .child(Self::history_input(
                    "switch-local-branch",
                    name.clone(),
                    branches.clone(),
                ))
                .on_ok(move |_, _, cx| {
                    let branch = input.read(cx).content.trim().to_string();
                    view.update(cx, |this, cx| {
                        if this.busy || this.root != root || !this.local_branches.contains(&branch)
                        {
                            return false;
                        }
                        if branch == this.status.branch {
                            return true;
                        }
                        this.command(
                            vec![
                                "branch".into(),
                                "switch".into(),
                                "--local".into(),
                                "--".into(),
                                branch,
                            ],
                            "Switch local branch",
                            false,
                            true,
                            cx,
                        );
                        true
                    })
                    .unwrap_or(true)
                })
        });
    }

    pub(super) fn new_branch_dialog(
        &mut self,
        remote: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || !self.connected {
            return;
        }
        let name = cx.new(|cx| TextInput::new("Branch name", cx));
        let view = cx.entity().downgrade();
        let root = self.root.clone();
        let title = if remote {
            "New remote branch"
        } else {
            "New local branch"
        };
        window.open_dialog(cx, move |dialog, _, _| {
            let input = name.clone();
            let view = view.clone();
            let root = root.clone();
            dialog.title(t(title)).confirm()
                .button_props(DialogButtonProps::default().cancel_text(t("Cancel")).ok_text(t("Create")))
                .child(t("Branch name")).child(name.clone())
                .child(if remote { "Create and switch to the new branch, then push it to the remote." }
                    else { "Create and switch to the new local branch." })
                .on_ok(move |_, _, cx| {
                    let name = input.read(cx).content.trim().to_string();
                    if name.is_empty() { return false; }
                    view.update(cx, |this, cx| {
                        if this.busy || !this.connected || this.root != root { return false; }
                        this.busy = true;
                        this.notice = tf("Creating branch {name}…", &[("name", name.to_string())]);
                        let cli = this.cli.clone();
                        let root = root.clone();
                        let identity = this.settings.identity.clone();
                        let task = cx.background_executor().spawn(async move {
                            let created = backend::run_as(&cli, &root, &["branch".into(), "create".into(), "--".into(), name.clone()], false, identity.as_deref())?;
                            if remote {
                                match backend::run_as(&cli, &root, &["push".into(), "--".into(), name], false, identity.as_deref()) {
                                    Ok(pushed) => Ok(format!("{created}\n{pushed}")),
                                    Err(error) => Err(format!("{created}\nLocal branch created, but remote push failed. Retry with Push.\n{error}")),
                                }
                            } else { Ok(created) }
                        });
                        cx.spawn(async move |this, cx| {
                            let result = task.await;
                            let _ = this.update(cx, |this, cx| {
                                this.busy = false;
                                this.output_title = title.into();
                                this.output = match result {
                                    Ok(output) => output,
                                    Err(error) => tf("Branch operation failed:\n{error}", &[("error", error.to_string())]),
                                };
                                this.show_log = false;
                                this.log(this.output.clone());
                                this.show_branches = true;
                                this.refresh(cx);
                            });
                        }).detach();
                        cx.notify();
                        true
                    }).unwrap_or(true)
                })
        });
    }

    pub(super) fn move_dialog(
        &mut self,
        source: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        let destination =
            cx.new(|cx| TextInput::new("Full destination path including file or folder name", cx));
        destination.update(cx, |input, _| {
            input.content = source.to_string_lossy().into_owned().into()
        });
        let root = self.root.clone();
        let view = cx.entity().downgrade();
        let validation = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        window.open_dialog(cx, move |dialog, _, _| {
            let destination_input = destination.clone();
            let source = source.clone();
            let root = root.clone();
            let view = view.clone();
            let validation = validation.clone();
            let error = validation.borrow().clone();
            dialog.title(t("Move")).confirm()
                .button_props(DialogButtonProps::default().cancel_text(t("Cancel")).ok_text(t("Move")))
                .child(div().flex().flex_col().gap_2()
                    .child(t("Source")).child(source.display().to_string())
                    .child(t("Destination")).child(destination.clone())
                    .child(t("Include the new file or folder name. Parent folder must exist."))
                    .child(t(&error)))
                .on_ok(move |_, window, cx| {
                    let value = destination_input.read(cx).content.trim().to_string();
                    if value.is_empty() {
                        *validation.borrow_mut() = "Enter a destination path.".into();
                        window.refresh();
                        return false;
                    }
                    let destination = PathBuf::from(value);
                    let destination = if destination.is_absolute() { destination } else { root.join(destination) };
                    let source = source.clone();
                    let root = root.clone();
                    view.update(cx, |this, cx| {
                        if this.busy || this.root != root { return false; }
                        this.busy = true;
                        this.notice = "Moving…".into();
                        let task = cx.background_executor().spawn(async move { backend::move_entry(&root, &source, &destination) });
                        cx.spawn(async move |this, cx| {
                            let result = task.await;
                            let _ = this.update(cx, |this, cx| {
                                this.busy = false;
                                this.show_log = false;
                                match result {
                                    Ok(()) => {
                                        this.selection.clear();
                                        this.output_title = "Move completed".into();
                                        this.output = "File system move completed. Refreshing repository status.".into();
                                        this.refresh(cx);
                                    }
                                    Err(error) => {
                                        this.error = true;
                                        this.notice = "Move failed — see details".into();
                                        this.output_title = "Move failed".into();
                                        this.output = error.clone();
                                        this.log(error);
                                    }
                                }
                                cx.notify();
                            });
                        }).detach();
                        cx.notify();
                        true
                    }).unwrap_or(true)
                })
        });
    }

    pub(super) fn history_input(
        id: &'static str,
        input: Entity<TextInput>,
        history: Vec<String>,
    ) -> impl IntoElement {
        let target = input.clone();
        div()
            .flex()
            .gap_2()
            .items_center()
            .child(div().flex_1().min_w_0().child(input))
            .child(
                Button::new(id)
                    .label("▾")
                    .dropdown_menu(move |mut menu, _, _| {
                        if history.is_empty() {
                            return menu.item(PopupMenuItem::new(t("No history")).disabled(true));
                        }
                        for value in &history {
                            let value = value.clone();
                            let target = target.clone();
                            menu = menu.item(PopupMenuItem::new(value.clone()).on_click(
                                move |_, _, cx| {
                                    target.update(cx, |input, cx| {
                                        input.reset();
                                        input.content = value.clone().into();
                                        cx.notify();
                                    });
                                },
                            ));
                        }
                        menu
                    }),
            )
    }

    pub(super) fn clone_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let remote = self.settings.clone_remote().unwrap_or_else(|| self.settings.clone_url.clone());
        self.settings.remember_clone();
        let mut urls = self.settings.clone_urls.clone();
        for url in std::iter::once(&remote).chain(self.settings.login_urls.iter()) {
            if !url.trim().is_empty() && !urls.contains(url) { urls.push(url.clone()); }
        }
        let url = cx.new(|cx| {
            let mut input = TextInput::new("lores://server:port/repository", cx);
            input.content = remote.into();
            input
        });
        let destinations = self.settings.clone_destinations.clone();
        let destination = cx.new(|cx| TextInput::new("Absolute destination path", cx));
        destination.update(cx, |input, _| {
            input.content = self.settings.clone_destination.clone().into()
        });
        cx.observe(&destination, |this, input, cx| {
            let value = input.read(cx).content.to_string();
            if this.settings.clone_destination != value {
                this.settings.clone_destination = value;
                this.save_settings();
            }
        })
        .detach();
        let validation = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let url_input = url.clone();
            let destination_input = destination.clone();
            let validation = validation.clone();
            let validation_text = validation.borrow().clone();
            let view = view.clone();
            let close_view = view.clone();
            let close_destination = destination.clone();
            dialog
                .title(t("Clone repository"))
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .cancel_text(t("Cancel"))
                        .ok_text(t("Clone")),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(t("Server URL"))
                        .child(Self::history_input("clone-server-history", url.clone(), urls.clone()))
                        .child(t("Destination Path"))
                        .child(Self::history_input(
                            "clone-path-history",
                            destination.clone(),
                            destinations.clone(),
                        ))
                        .child(validation_text),
                )
                .on_close(move |_, _, cx| {
                    let _ = close_view.update(cx, |this, cx| {
                        this.settings.clone_destination =
                            close_destination.read(cx).content.to_string();
                        this.settings.remember_clone();
                        this.save_settings();
                    });
                })
                .on_ok(move |_, window, cx| {
                    let remote = url_input.read(cx).content.trim().to_owned();
                    let valid_url = remote.split_once("://").is_some_and(|(_, rest)| {
                        rest.split_once('/').is_some_and(|(host, repository)| !host.is_empty() && !repository.trim_matches('/').is_empty())
                    }) && !remote.chars().any(char::is_whitespace);
                    if !valid_url {
                        *validation.borrow_mut() = t("Enter a server URL including the repository name.");
                        window.refresh();
                        return false;
                    }
                    let path = PathBuf::from(destination_input.read(cx).content.trim());
                    if !path.is_absolute() {
                        *validation.borrow_mut() =
                            t("Enter an absolute destination path.");
                        window.refresh();
                        return false;
                    }
                    view.update(cx, |this, cx| {
                        if this.busy {
                            return false;
                        }
                        this.busy = true;
                        this.settings.clone_url = remote.clone();
                        this.settings.clone_destination = path.to_string_lossy().into_owned();
                        this.settings.remember_clone();
                        this.save_settings();
                        this.notice = "Cloning repository…".into();
                        let cli = this.cli.clone();
                        let identity = this.settings.identity.clone();
                        let target = path.clone();
                        let task = cx.background_executor().spawn(async move {
                            backend::clone_repository(&cli, &remote, &target, identity.as_deref())
                        });
                        cx.spawn(async move |this, cx| {
                            let result = task.await;
                            let _ = this.update(cx, |this, cx| {
                                this.busy = false;
                                this.show_log = false;
                                match result {
                                    Ok(output) => {
                                        this.log(output);
                                        this.open_repository(path, cx);
                                    }
                                    Err(error) => {
                                        this.error = true;
                                        this.notice = "Clone failed — see details".into();
                                        this.output_title = "Clone failed".into();
                                        this.output = error.clone();
                                        this.log(error);
                                    }
                                }
                                cx.notify();
                            });
                        })
                        .detach();
                        cx.notify();
                        true
                    })
                    .unwrap_or(true)
                })
        });
    }
}
