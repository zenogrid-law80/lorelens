use super::*;

impl Lens {
    fn copy_dropped_paths(&mut self, paths: &ExternalPaths, destination: PathBuf, cx: &mut Context<Self>) {
        if self.busy || paths.paths().is_empty() { return; }
        self.busy = true;
        self.preview.invalidate();
        self.error = false;
        self.notice = "Copying files…".into();
        let root = self.root.clone();
        let sources = paths.paths().to_vec();
        let task = cx.background_executor().spawn(async move {
            backend::copy_entries(&root, &destination, &sources)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.show_log = false;
                match result {
                    Ok(count) => {
                        this.output_title = "Copy completed".into();
                        this.output = format!("Copied {count} items.");
                        this.selection.clear();
                        this.refresh(cx);
                    }
                    Err(error) => {
                        this.error = true;
                        this.notice = "Copy failed — see details".into();
                        this.output_title = "Copy failed".into();
                        this.output = error.clone();
                        this.log(error);
                    }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    pub(super) fn render_file_browser(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let rgb = palette(cx);
        let ready = !self.busy;
        let query = self.filter.read(cx).content.to_lowercase();
        let title = self
            .root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        // Repository navigation is persistent, independent of the right-hand tab.
        let mut files = div()
            .id("repository-files")
            .track_focus(&self.files_focus)
            .track_scroll(&self.files_scroll)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" {
                    cx.stop_propagation();
                    if !this.busy {
                        this.open_selected_file(cx);
                    }
                    return;
                }
                if !matches!(key, "up" | "down" | "left" | "right") {
                    return;
                }
                cx.stop_propagation();
                if this.busy {
                    return;
                }
                if key == "left" || key == "right" {
                    let target = if key == "left" {
                        if this.directory == this.root {
                            return;
                        }
                        this.directory
                            .parent()
                            .filter(|parent| parent.starts_with(&this.root))
                            .map(|parent| parent.to_path_buf())
                    } else {
                        this.selection.current.as_ref().and_then(|selected| {
                            let path = this.root.join(selected);
                            this.entries
                                .iter()
                                .find(|entry| entry.directory && entry.path == path)
                                .map(|entry| entry.path.clone())
                        })
                    };
                    if let Some(target) = target {
                        this.folder_to_select = (key == "left").then(|| this.directory.clone());
                        this.directory = target;
                        this.selection.clear();
                        this.filter.update(cx, |input, cx| {
                            input.reset();
                            cx.notify();
                        });
                        this.files_scroll.set_offset(point(px(0.), px(0.)));
                        this.load_directory(cx);
                    }
                    return;
                }
                let query = this.filter.read(cx).content.to_lowercase();
                let visible: Vec<_> = this
                    .entries
                    .iter()
                    .filter(|entry| {
                        entry
                            .path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_lowercase()
                            .contains(&query)
                    })
                    .take(2000)
                    .map(|entry| {
                        (
                            entry
                                .path
                                .strip_prefix(&this.root)
                                .unwrap_or(&entry.path)
                                .to_string_lossy()
                                .replace('\\', "/"),
                            entry.directory,
                        )
                    })
                    .collect();
                if visible.is_empty() {
                    return;
                }
                let current = this
                    .selection
                    .current
                    .as_ref()
                    .and_then(|selected| visible.iter().position(|(path, _)| path == selected));
                let index = match (current, key) {
                    (Some(index), "up") => index.saturating_sub(1),
                    (Some(index), _) => (index + 1).min(visible.len() - 1),
                    (None, "up") => visible.len() - 1,
                    (None, _) => 0,
                };
                let (path, directory) = &visible[index];
                if *directory {
                    this.selection.current = Some(path.clone());
                    this.selection.paths.clear();
                    this.selection.paths.insert(path.clone());
                    this.selection.anchor = Some(path.clone());
                    this.notice = "1 item selected".into();
                } else {
                    this.select(path.clone(), cx);
                }
                this.files_scroll.scroll_to_item(index);
                cx.notify();
            }))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        let mut count = 0;
        for (i, entry) in self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&query)
            })
            .take(2000)
        {
            count += 1;
            let relative = entry
                .path
                .strip_prefix(&self.root)
                .unwrap_or(&entry.path)
                .to_string_lossy()
                .replace('\\', "/");
            let label = entry
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let path = entry.path.clone();
            let drop_path = path.clone();
            let context_path = path.clone();
            let context_relative = relative.clone();
            let context_root = self.root.clone();
            let view = cx.entity().downgrade();
            let directory = entry.directory;
            let change = self.status.changes.iter().find(|c| c.path == relative);
            let marker = change.map_or("", Change::file_marker);
            files = files.child(
                div().id(("tree-context", i)).child(
                    div()
                        .id(("tree-file", i))
                        .h(px(30.))
                        .px_3()
                        .flex()
                        .gap_2()
                        .items_center()
                        .bg(rgb(if self.selection.paths.contains(&relative) {
                            Selected
                        } else {
                            PANEL
                        }))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(Hover)))
                        .when(directory && ready, |row| {
                            row.drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(rgb(Selected)))
                                .on_drop(cx.listener(move |this, paths: &ExternalPaths, _, cx| {
                                    cx.stop_propagation();
                                    this.copy_dropped_paths(paths, drop_path.clone(), cx);
                                }))
                        })
                        .child(
                            div()
                                .w(px(16.))
                                .text_color(rgb(if directory { Warning } else { MUTED }))
                                .child(if directory { "▸" } else { "·" }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(label),
                        )
                        .child(div().text_size(px(10.)).text_color(rgb(BLUE)).child(marker))
                        .child(div().text_size(px(10.)).text_color(rgb(Warning)).child(
                            if !directory && self.locked_paths.contains(&relative) {
                                "L"
                            } else {
                                ""
                            },
                        ))
                        .child(div().text_size(px(10.)).text_color(rgb(MUTED)).child(
                            if directory {
                                String::new()
                            } else {
                                format_size(entry.size)
                            },
                        ))
                        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                            window.focus(&this.files_focus);
                            let modifiers = event.modifiers();
                            let additive = modifiers.control || modifiers.platform;
                            // A preview from the first click may still be loading.
                            if !directory
                                && event.click_count() == 2
                                && !additive
                                && !modifiers.shift
                            {
                                this.open_selected_file(cx);
                                return;
                            }
                            if this.busy {
                                return;
                            }
                            if directory
                                && event.click_count() == 2
                                && !additive
                                && !modifiers.shift
                            {
                                this.selection.clear();
                                this.directory = path.clone();
                                this.load_directory(cx);
                            } else if !directory && !additive && !modifiers.shift {
                                this.select(relative.clone(), cx);
                            } else {
                                let query = this.filter.read(cx).content.to_lowercase();
                                let visible: Vec<String> = this
                                    .entries
                                    .iter()
                                    .filter(|entry| {
                                        entry
                                            .path
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .to_lowercase()
                                            .contains(&query)
                                    })
                                    .take(2000)
                                    .map(|entry| {
                                        entry
                                            .path
                                            .strip_prefix(&this.root)
                                            .unwrap_or(&entry.path)
                                            .to_string_lossy()
                                            .replace('\\', "/")
                                    })
                                    .collect();
                                if modifiers.shift {
                                    let anchor =
                                        this.selection.anchor.as_ref().and_then(|anchor| {
                                            visible.iter().position(|p| p == anchor)
                                        });
                                    let end = visible.iter().position(|p| p == &relative);
                                    if !additive {
                                        this.selection.paths.clear();
                                    }
                                    if let (Some(start), Some(end)) = (anchor, end) {
                                        this.selection.paths.extend(
                                            visible[start.min(end)..=start.max(end)]
                                                .iter()
                                                .cloned(),
                                        );
                                    } else {
                                        this.selection.paths.insert(relative.clone());
                                    }
                                } else {
                                    if !additive {
                                        this.selection.paths.clear();
                                    }
                                    if !this.selection.paths.remove(&relative) {
                                        this.selection.paths.insert(relative.clone());
                                    }
                                    this.selection.anchor = Some(relative.clone());
                                }
                                this.selection.current = if this.selection.paths.contains(&relative)
                                {
                                    Some(relative.clone())
                                } else {
                                    this.selection.paths.iter().min().cloned()
                                };
                                this.notice = tf(
                                    "{count} items selected",
                                    &[("count", this.selection.paths.len().to_string())],
                                );
                                cx.notify();
                            }
                        }))
                        .context_menu(move |menu, _window, cx| {
                            let menu = if directory {
                                let folder_view = view.clone();
                                let folder_path = context_relative.clone();
                                let folder_root = context_root.clone();
                                let enabled = view.upgrade().is_some_and(|entity| {
                                    let lens = entity.read(cx);
                                    !lens.busy && lens.connected && lens.root == folder_root
                                        && lens.status.changes.iter().any(|c| std::path::Path::new(&c.path).starts_with(&folder_path))
                                });
                                menu.item(PopupMenuItem::new(t("Revert folder")).disabled(!enabled)
                                    .on_click(move |_, window, cx| {
                                        let _ = folder_view.update(cx, |this, cx| {
                                            if this.root == folder_root {
                                                this.folder_changes_dialog(vec![folder_path.clone()], "reset", window, cx);
                                            }
                                        });
                                    })).separator()
                            } else { menu };
                            let menu = if !directory
                                && view.upgrade().is_some_and(|entity| {
                                    let lens = entity.read(cx);
                                    lens.root == context_root
                                        && lens.status.changes.iter().any(|change| {
                                            change.path == context_relative
                                                && change.file_marker() == "M"
                                        })
                                }) {
                                let diff_view = view.clone();
                                let diff_path = context_relative.clone();
                                let diff_root = context_root.clone();
                                let enabled = view.upgrade().is_some_and(|entity| {
                                    let lens = entity.read(cx);
                                    lens.connected && !lens.busy && lens.root == context_root
                                });
                                menu.item(
                                    PopupMenuItem::new(t("Diff")).disabled(!enabled).on_click(
                                        move |_, _, cx| {
                                            let _ = diff_view.update(cx, |this, cx| {
                                                if this.root == diff_root
                                                    && this.status.changes.iter().any(|change| {
                                                        change.path == diff_path
                                                            && change.file_marker() == "M"
                                                    })
                                                {
                                                    this.external_diff(diff_path.clone(), cx);
                                                }
                                            });
                                        },
                                    ),
                                )
                                .separator()
                            } else {
                                menu
                            };
                            let terminal_path = context_path.clone();
                            let reveal_path = context_path.clone();
                            let copy_path = context_path.clone();
                            let view = view.clone();
                            let staging = view.upgrade().and_then(|entity| {
                                let lens = entity.read(cx);
                                if lens.root != context_root {
                                    return None;
                                }
                                let mut changes = lens.status.changes.iter().filter(|c| {
                                    c.path == context_relative || (directory
                                        && std::path::Path::new(&c.path).starts_with(&context_relative))
                                }).peekable();
                                changes.peek()?;
                                Some((changes.any(|c| c.staged), !lens.busy && lens.connected))
                            });
                            let menu = if let Some((staged, enabled)) = staging {
                                let mut menu = menu;
                                for unstage in [false, true] {
                                    if unstage && !staged {
                                        continue;
                                    }
                                    let stage_view = view.clone();
                                    let stage_path = context_relative.clone();
                                    let stage_root = context_root.clone();
                                    let label = if directory && unstage {
                                        "Unstage folder"
                                    } else if directory {
                                        "Stage folder"
                                    } else if unstage {
                                        "Unstage file"
                                    } else {
                                        "Stage file"
                                    };
                                    menu = menu.item(
                                        PopupMenuItem::new(t(label)).disabled(!enabled).on_click(
                                            move |_, window, cx| {
                                                let _ = stage_view.update(cx, |this, cx| {
                                                    if this.busy
                                                        || !this.connected
                                                        || this.root != stage_root
                                                        || !this.status.changes.iter().any(|c| {
                                                            (c.path == stage_path || (directory
                                                                && std::path::Path::new(&c.path).starts_with(&stage_path)))
                                                                && (!unstage || c.staged)
                                                        })
                                                    {
                                                        return;
                                                    }
                                                    if directory {
                                                        this.folder_changes_dialog(vec![stage_path.clone()], if unstage { "unstage" } else { "stage" }, window, cx);
                                                        return;
                                                    }
                                                    this.command(
                                                        vec![
                                                            if unstage {
                                                                "unstage"
                                                            } else {
                                                                "stage"
                                                            }
                                                            .into(),
                                                            "--".into(),
                                                            stage_path.clone(),
                                                        ],
                                                        label,
                                                        false,
                                                        true,
                                                        cx,
                                                    );
                                                });
                                            },
                                        ),
                                    );
                                }
                                menu.separator()
                            } else {
                                menu
                            };
                            let menu = if !directory
                                && view.upgrade().is_some_and(|entity| {
                                    let lens = entity.read(cx);
                                    lens.root == context_root
                                        && lens
                                            .status
                                            .changes
                                            .iter()
                                            .any(|c| c.path == context_relative)
                                }) {
                                let discard_view = view.clone();
                                let discard_path = context_relative.clone();
                                let discard_root = context_root.clone();
                                menu.item(PopupMenuItem::new(t("Discard")).on_click(
                                    move |_, _, cx| {
                                        let _ = discard_view.update(cx, |this, cx| {
                                            if this.busy
                                                || this.root != discard_root
                                                || !this
                                                    .status
                                                    .changes
                                                    .iter()
                                                    .any(|c| c.path == discard_path)
                                                || this.root.join(&discard_path).is_dir()
                                            {
                                                return;
                                            }
                                            this.command(
                                                vec![
                                                    "reset".into(),
                                                    "--purge".into(),
                                                    "--".into(),
                                                    discard_path.clone(),
                                                ],
                                                "Discard",
                                                false,
                                                true,
                                                cx,
                                            );
                                        });
                                    },
                                ))
                                .separator()
                            } else {
                                menu
                            };
                            let move_view = view.clone();
                            let delete_view = view.clone();
                            let delete_path = context_path.clone();
                            let delete_root = context_root.clone();
                            let delete_enabled = view.upgrade().is_some_and(|entity| {
                                let lens = entity.read(cx);
                                !lens.busy && lens.root == context_root
                            });
                            let move_path = context_path.clone();
                            let move_root = context_root.clone();
                            let menu = menu
                                .item(PopupMenuItem::new(t("Move…")).on_click(
                                    move |_, window, cx| {
                                        let _ = move_view.update(cx, |this, cx| {
                                            if this.root == move_root {
                                                this.move_dialog(move_path.clone(), window, cx);
                                            }
                                        });
                                    },
                                ))
                                .separator();
                            let terminal_view = view.clone();
                            let menu = menu
                                .item(PopupMenuItem::new(t("Copy full path")).on_click(
                                    move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            copy_path.to_string_lossy().into_owned(),
                                        ));
                                    },
                                ))
                                .item(
                                    PopupMenuItem::new(t(if cfg!(target_os = "macos") {
                                        "Show in Finder"
                                    } else {
                                        "Show in Explorer"
                                    }))
                                    .on_click(
                                        move |_, _, cx| {
                                            cx.reveal_path(&reveal_path);
                                        },
                                    ),
                                )
                                .item(PopupMenuItem::new(t("Open Command Window Here")).on_click(
                                    move |_, _, cx| {
                                        let _ = terminal_view.update(cx, |this, cx| {
                                            this.open_command_window(&terminal_path);
                                            cx.notify();
                                        });
                                    },
                                ))
                                .separator();
                            let menu = if !directory {
                                let state = std::rc::Rc::new(std::cell::RefCell::new(
                                    None::<Result<bool, String>>,
                                ));
                                let display_state = state.clone();
                                let action_state = state.clone();
                                let action_view = view.clone();
                                let action_path = context_path.clone();
                                let Some(entity) = view.upgrade() else {
                                    return menu;
                                };
                                let root = entity.read(cx).root.clone();
                                let cli = entity.read(cx).cli.clone();
                                let identity = entity.read(cx).settings.identity.clone();
                                let query_path = context_path.clone();
                                let original_root = root.clone();
                                let task = cx.background_executor().spawn(async move {
                                    backend::run_as(
                                        &cli,
                                        &root,
                                        &[
                                            "lock".into(),
                                            "status".into(),
                                            "--".into(),
                                            query_path.to_string_lossy().into_owned(),
                                        ],
                                        true,
                                        identity.as_deref(),
                                    )
                                    .and_then(|output| backend::parse_lock_status(&output))
                                });
                                let error_view = view.clone();
                                cx.spawn(async move |menu, cx| {
                                    let result = task.await;
                                    if let Err(error) = &result {
                                        let error = error.clone();
                                        let _ = error_view.update(cx, |this, cx| {
                                            this.log(format!("Lock status failed: {error}"));
                                            cx.notify();
                                        });
                                    }
                                    *state.borrow_mut() = Some(result);
                                    let _ = menu.update(cx, |_, cx| cx.notify());
                                })
                                .detach();
                                menu.item(
                                    PopupMenuItem::element(move |_, _| {
                                        div().child(t(match display_state.borrow().as_ref() {
                                            None => "Checking lock status…",
                                            Some(Ok(true)) => "Unlock",
                                            Some(Ok(false)) => "Lock",
                                            Some(Err(_)) => {
                                                "Lock status unavailable — reopen to retry"
                                            }
                                        }))
                                    })
                                    .on_click(
                                        move |_, _, cx| {
                                            let Some(Ok(locked)) =
                                                action_state.borrow().as_ref().cloned()
                                            else {
                                                return;
                                            };
                                            let _ = action_view.update(cx, |this, cx| {
                                                if this.root != original_root {
                                                    return;
                                                }
                                                this.command(
                                                    vec![
                                                        "lock".into(),
                                                        if locked { "release" } else { "acquire" }
                                                            .into(),
                                                        "--".into(),
                                                        action_path.to_string_lossy().into_owned(),
                                                    ],
                                                    if locked { "Unlock" } else { "Lock" },
                                                    false,
                                                    true,
                                                    cx,
                                                );
                                            });
                                        },
                                    ),
                                )
                                .separator()
                            } else {
                                menu
                            };
                            let menu = menu.item(PopupMenuItem::new(t("Delete…")).disabled(!delete_enabled).on_click(
                                move |_, window, cx| {
                                    let _ = delete_view.update(cx, |this, cx| {
                                        if this.root == delete_root { this.delete_dialog(delete_path.clone(), window, cx); }
                                    });
                                },
                            ));
                            if !directory && view.upgrade().is_some_and(|entity| entity.read(cx).obliterate_enabled) {
                                let action_view = view.clone();
                                let path = context_relative.clone();
                                let root = context_root.clone();
                                let enabled = view.upgrade().is_some_and(|entity| {
                                    let lens = entity.read(cx);
                                    !lens.busy && lens.connected && lens.root == root
                                        && backend::obliterate_args(&root, &path).is_ok()
                                });
                                menu.item(PopupMenuItem::new(t("Obliterate…")).disabled(!enabled)
                                    .on_click(move |_, window, cx| {
                                        let _ = action_view.update(cx, |this, cx| {
                                            if this.root == root { this.obliterate_dialog(path.clone(), window, cx); }
                                        });
                                    }))
                            } else { menu }

                        }),
                ),
            );
        }
        if count == 0 {
            files = files.child(
                div()
                    .p_4()
                    .text_color(rgb(MUTED))
                    .child(t("No matching files.")),
            );
        }
        div()
            .id("file-browser-drop-target")
            .when(ready, |panel| {
                panel.drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(rgb(Selected)))
                    .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                        cx.stop_propagation();
                        this.copy_dropped_paths(paths, this.directory.clone(), cx);
                    }))
            })
            .w(px(320.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .min_h_0()
            .bg(rgb(PANEL))
            .border_r_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .text_size(px(11.))
                    .text_color(rgb(BLUE))
                    .child(t("FILES")),
            )
            .child(
                div()
                    .px_3()
                    .pt_3()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.clone()),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .child(self.root.display().to_string()),
            )
            .child(
                div()
                    .mx_3()
                    .mb_2()
                    .overflow_hidden()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .rounded_md()
                    .child(self.filter.clone()),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        self.button("up", "↑", ready && self.directory != self.root)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.busy || this.directory == this.root {
                                    return;
                                }
                                if let Some(parent) = this.directory.parent() {
                                    this.directory = parent.to_path_buf();
                                    this.load_directory(cx);
                                }
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.))
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(format!(
                                "/{}",
                                self.directory
                                    .strip_prefix(&self.root)
                                    .unwrap_or(&self.directory)
                                    .to_string_lossy()
                                    .replace('\\', "/")
                            )),
                    ),
            )
            .child(files)
            .when(self.entries.len() > 2000, |d| {
                d.child(
                    div()
                        .p_2()
                        .text_size(px(10.))
                        .text_color(rgb(MUTED))
                        .child(t("Up to 2,000 matches. Filter to narrow.")),
                )
            })
    }
}
