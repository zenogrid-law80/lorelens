use super::*;

impl Lens {
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
        self.settings.remember_clone();
        let urls = self.settings.clone_urls.clone();
        let destinations = self.settings.clone_destinations.clone();
        let url = cx.new(|cx| TextInput::new("lores://server/repository", cx));
        let destination = cx.new(|cx| TextInput::new("Absolute destination path", cx));
        url.update(cx, |input, _| {
            input.content = self.settings.clone_url.clone().into()
        });
        destination.update(cx, |input, _| {
            input.content = self.settings.clone_destination.clone().into()
        });
        cx.observe(&url, |this, input, cx| {
            let value = input.read(cx).content.to_string();
            if this.settings.clone_url != value {
                this.settings.clone_url = value;
                this.save_settings();
            }
        })
        .detach();
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
            let close_url = url.clone();
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
                        .child(t("Repository URL"))
                        .child(Self::history_input(
                            "clone-url-history",
                            url.clone(),
                            urls.clone(),
                        ))
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
                        this.settings.clone_url = close_url.read(cx).content.to_string();
                        this.settings.clone_destination =
                            close_destination.read(cx).content.to_string();
                        this.settings.remember_clone();
                        this.save_settings();
                    });
                })
                .on_ok(move |_, window, cx| {
                    let remote = url_input.read(cx).content.trim().to_string();
                    let path = PathBuf::from(destination_input.read(cx).content.trim());
                    if remote.is_empty() || !path.is_absolute() {
                        *validation.borrow_mut() =
                            "Enter a repository URL and an absolute destination path.".into();
                        window.refresh();
                        return false;
                    }
                    view.update(cx, |this, cx| {
                        if this.busy {
                            return false;
                        }
                        this.busy = true;
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
