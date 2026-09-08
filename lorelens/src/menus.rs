use super::*;

impl Lens {
    pub(super) fn app_menu(
        &self,
        kind: &'static str,
        toolbar: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity().downgrade();
        let recent = self.settings.recent.clone();
        let ready = !self.busy;
        let file = ready && self.connected && self.selection.current.is_some();
        let commit = ready && self.connected && self.status.changes.iter().any(|c| c.staged);
        let theme = self.settings.theme.clone();
        let show_log = self.show_log;
        let account = self.logged_in_account.clone();
        let label = if toolbar {
            format!(
                "{} ▾",
                self.root.file_name().unwrap_or_default().to_string_lossy()
            )
        } else if kind == "Account" {
            format!("{}: {} ▾", t("Account"), t(&self.logged_in_account))
        } else {
            format!("{} ▾", t(kind))
        };
        Button::new(if toolbar { "repository-selector" } else { kind })
            .label(label)
            .dropdown_menu(move |mut menu, window, cx| {
                let items: Vec<(&str, &str, bool)> = match kind {
                    "Repository" => vec![
                        ("Open repository…", "open", ready),
                        ("Clone repository…", "clone", ready),
                    ],
                    "Changes" => vec![
                        ("Stage", "stage", file),
                        ("Unstage", "unstage", file),
                        ("Commit staged", "commit", commit),
                        ("File history", "history", file),
                        ("Pending push", "pending", ready),
                    ],
                    "View" => vec![
                        ("System theme", "System", true),
                        ("Light theme", "Light", true),
                        ("Dark theme", "Dark", true),
                        ("Command log", "log", true),
                    ],
                    _ => vec![("Login…", "login", ready)],
                };
                for (label, action, enabled) in items {
                    let view = view.clone();
                    let label = t(label);
                    let checked =
                        kind == "View" && (theme == action || (action == "log" && show_log));
                    menu = menu.item(
                        PopupMenuItem::new(if checked {
                            format!("✓ {label}")
                        } else {
                            label.into()
                        })
                        .disabled(!enabled)
                        .on_click(move |_, window, cx| {
                            let _ = view.update(cx, |this, cx| {
                                match action {
                                    "open" => this.choose(false, cx),
                                    "clone" => this.clone_dialog(window, cx),
                                    "stage" | "unstage" | "history" => {
                                        this.file_command(action, cx)
                                    }
                                    "commit" => this.commit_staged(cx),
                                    "login" => this.login_dialog(window, cx),
                                    "System" | "Light" | "Dark" => {
                                        this.settings.theme = action.into();
                                        apply_theme(action, Some(window), cx);
                                        this.save_settings();
                                    }
                                    "log" => this.show_log = !this.show_log,
                                    "pending" => {
                                        this.output_title = "Pending push commits".into();
                                        this.output = match &this.pending_push {
                                            Ok(items) if items.is_empty() => {
                                                t("No commits pending push.")
                                            }
                                            Ok(items) => tf(
                                                "{count} commits pending push\n\n{commits}",
                                                &[
                                                    ("count", items.len().to_string()),
                                                    ("commits", items.join("\n")),
                                                ],
                                            ),
                                            Err(error) => error.clone(),
                                        };
                                        this.show_log = false;
                                    }
                                    _ => {}
                                }
                                cx.notify();
                            });
                        }),
                    );
                }
                if kind == "View" {
                    let language_view = view.clone();
                    menu = menu.separator().submenu(
                        t("Language"),
                        window,
                        cx,
                        move |mut menu, _, cx| {
                            let current = language_view
                                .upgrade()
                                .map(|entity| entity.read(cx).settings.language.clone())
                                .unwrap_or_default();
                            for (code, name) in i18n::LOCALES {
                                let view = language_view.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(if current == code {
                                        format!("✓ {name}")
                                    } else {
                                        name.into()
                                    })
                                    .on_click(
                                        move |_, window, cx| {
                                            let _ = view.update(cx, |this, cx| {
                                                this.settings.language = code.into();
                                                i18n::set_locale(code);
                                                window.set_window_title(&t(
                                                    "LoreLens — Desktop repository client",
                                                ));
                                                this.save_settings();
                                                this.filter.update(cx, |_, cx| cx.notify());
                                                this.message.update(cx, |_, cx| cx.notify());
                                                window.refresh();
                                                cx.notify();
                                            });
                                        },
                                    ),
                                );
                            }
                            menu
                        },
                    );
                }
                if kind == "Repository" {
                    menu = menu.separator().label(t("Recent repositories"));
                    for path in &recent {
                        let view = view.clone();
                        let path = path.clone();
                        menu = menu.item(
                            PopupMenuItem::new(path.display().to_string())
                                .disabled(!ready)
                                .on_click(move |_, _, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.open_repository(path.clone(), cx)
                                    });
                                }),
                        );
                    }
                    if recent.is_empty() {
                        menu = menu.label(t("No recent repositories"));
                    }
                }
                if kind == "Account" {
                    menu = menu
                        .separator()
                        .label(tf("Signed in: {account}", &[("account", t(&account))]));
                }
                menu
            })
    }

    pub(super) fn tool_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().downgrade();
        let selected = self.settings.external_tool.clone();
        Button::new("tools-menu")
            .label(t("Tools ▾"))
            .dropdown_menu(move |menu, window, cx| {
                let selection_view = view.clone();
                let current = selected.clone();
                let menu = menu.submenu(
                    tf(
                        "Diff / Merge: {selected}",
                        &[("selected", selected.to_string())],
                    ),
                    window,
                    cx,
                    move |mut menu, _, _| {
                        for tool in external_tools::TOOLS {
                            let view = selection_view.clone();
                            menu = menu.item(
                                PopupMenuItem::new(if tool == current {
                                    format!("✓ {tool}")
                                } else {
                                    tool.into()
                                })
                                .on_click(move |_, _, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.settings.external_tool = tool.into();
                                        this.save_settings();
                                        if external_tools::resolve(
                                            tool,
                                            this.settings.tool_paths.get(tool),
                                        )
                                        .is_none()
                                        {
                                            this.choose_tool(tool.into(), None, cx);
                                        }
                                        cx.notify();
                                    });
                                }),
                            );
                        }
                        menu
                    },
                );
                let cli_view = view.clone();
                let menu = menu.item(PopupMenuItem::new(t("Locate Lore CLI…")).on_click(
                    move |_, _, cx| {
                        let _ = cli_view.update(cx, |this, cx| this.choose(true, cx));
                    },
                ));
                let locate_view = view.clone();
                let path_view = view.clone();
                menu.separator()
                    .item(
                        PopupMenuItem::new(t("Locate executable…")).on_click(move |_, _, cx| {
                            let _ = locate_view.update(cx, |this, cx| {
                                let tool = this.settings.external_tool.clone();
                                this.choose_tool(tool, None, cx);
                            });
                        }),
                    )
                    .item(PopupMenuItem::new(t("Use PATH")).on_click(move |_, _, cx| {
                        let _ = path_view.update(cx, |this, cx| {
                            let tool = this.settings.external_tool.clone();
                            this.settings.tool_paths.remove(&tool);
                            this.save_settings();
                            if external_tools::resolve(&tool, None).is_none() {
                                this.choose_tool(tool, None, cx);
                            }
                            cx.notify();
                        });
                    }))
            })
    }
}
