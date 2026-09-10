use super::*;

impl Lens {
  fn toggle_tree_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
    if self.busy {
      return;
    }
    if !self.expanded_folders.remove(&path) {
      self.expanded_folders.insert(path.clone());
    }
    self.directory = path;
    self.load_directory(cx);
  }
  fn copy_dropped_paths(&mut self, paths: &ExternalPaths, destination: PathBuf, cx: &mut Context<Self>) {
    if self.busy || paths.paths().is_empty() {
      return;
    }
    self.busy = true;
    self.preview.invalidate();
    self.error = false;
    self.notice = "Copying files…".into();
    let root = self.root.clone();
    let sources = paths.paths().to_vec();
    let task = cx.background_executor().spawn(async move { backend::copy_entries(&root, &destination, &sources) });
    cx.spawn(async move |this, cx| {
      let result = task.await;
      let _ = this.update(cx, |this, cx| {
        this.busy = false;
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
    })
    .detach();
    cx.notify();
  }

  pub(super) fn render_file_browser(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
    let rgb = palette(cx);
    let ready = !self.busy;
    let query = self.filter.read(cx).content.to_lowercase();
    let title = self.root.file_name().unwrap_or_default().to_string_lossy().into_owned();

    // Repository navigation is persistent, independent of the right-hand tab.
    let mut files = div()
      .id("repository-files")
      .track_focus(&self.files_focus)
      .track_scroll(&self.files_scroll)
      .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        if key == "a" && (modifiers.control || modifiers.platform) && !modifiers.alt && !modifiers.shift {
          cx.stop_propagation();
          if this.busy {
            return;
          }
          let query = this.filter.read(cx).content.to_lowercase();
          let visible: Vec<_> = this
            .entries
            .iter()
            .filter(|entry| entry.path.file_name().unwrap_or_default().to_string_lossy().to_lowercase().contains(&query))
            .take(2000)
            .map(|entry| entry.path.strip_prefix(&this.root).unwrap_or(&entry.path).to_string_lossy().replace('\\', "/"))
            .collect();
          this.selection.select_all(&visible);
          this.preview.invalidate();
          this.notice = tf("{count} items selected", &[("count", visible.len().to_string())]);
          cx.notify();
          return;
        }
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
          if let Some(selected) = this.selection.current.clone() {
            let path = this.root.join(selected);
            if path.is_dir() && ((key == "right" && !this.expanded_folders.contains(&path)) || (key == "left" && this.expanded_folders.contains(&path))) {
              this.toggle_tree_folder(path, cx);
            } else if key == "left"
              && let Some(parent) = path.parent().filter(|p| *p != this.root)
            {
              let relative = parent.strip_prefix(&this.root).unwrap_or(parent).to_string_lossy().replace('\\', "/");
              this.selection.select(relative);
              cx.notify();
            }
          }
          return;
        }
        let query = this.filter.read(cx).content.to_lowercase();
        let visible: Vec<_> = this
          .entries
          .iter()
          .filter(|entry| entry.path.file_name().unwrap_or_default().to_string_lossy().to_lowercase().contains(&query))
          .take(2000)
          .map(|entry| (entry.path.strip_prefix(&this.root).unwrap_or(&entry.path).to_string_lossy().replace('\\', "/"), entry.directory))
          .collect();
        if visible.is_empty() {
          return;
        }
        let current = this.selection.current.as_ref().and_then(|selected| visible.iter().position(|(path, _)| path == selected));
        let index = match (current, key) {
          (Some(index), "up") => index.saturating_sub(1),
          (Some(index), _) => (index + 1).min(visible.len() - 1),
          (None, "up") => visible.len() - 1,
          (None, _) => 0,
        };
        let (path, directory) = &visible[index];
        if modifiers.shift {
          let visible_paths: Vec<_> = visible.iter().map(|(path, _)| path.clone()).collect();
          this.selection.click(path.clone(), &visible_paths, modifiers.control || modifiers.platform, true);
          this.preview.invalidate();
          this.notice = tf("{count} items selected", &[("count", this.selection.paths.len().to_string())]);
        } else if *directory {
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
      .pr(px(12.))
      .overflow_y_scroll();
    let mut count = 0;
    for (i, entry) in self
      .entries
      .iter()
      .enumerate()
      .filter(|(_, e)| e.path.file_name().unwrap_or_default().to_string_lossy().to_lowercase().contains(&query))
      .take(2000)
    {
      count += 1;
      let relative = entry.path.strip_prefix(&self.root).unwrap_or(&entry.path).to_string_lossy().replace('\\', "/");
      let label = entry.path.file_name().unwrap_or_default().to_string_lossy().into_owned();
      let path = entry.path.clone();
      let drop_path = path.clone();
      let context_path = path.clone();
      let context_relative = relative.clone();
      let context_root = self.root.clone();
      let view = cx.entity().downgrade();
      let directory = entry.directory;
      let depth = entry.path.strip_prefix(&self.root).unwrap_or(&entry.path).components().count().saturating_sub(1);
      let toggle_path = path.clone();
      let change = self.status.changes.iter().find(|c| c.path == relative);
      let marker = change.map_or("", Change::file_marker);
      files = files.child(
        div().id(("tree-context", i)).child(
          div()
            .id(("tree-file", i))
            .h(px(32.))
            .px_3()
            .pl(px(12. + depth as f32 * 16.))
            .flex()
            .gap_2()
            .items_center()
            .bg(rgb(if self.selection.paths.contains(&relative) { Selected } else { Sidebar }))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(Hover)))
            .when(directory && ready, |row| {
              row
                .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(rgb(Selected)))
                .on_drop(cx.listener(move |this, paths: &ExternalPaths, _, cx| {
                  cx.stop_propagation();
                  this.copy_dropped_paths(paths, drop_path.clone(), cx);
                }))
            })
            .child(
              div()
                .id(("tree-toggle", i))
                .on_click(cx.listener(move |this, _, _, cx| {
                  if directory {
                    cx.stop_propagation();
                    this.toggle_tree_folder(toggle_path.clone(), cx);
                  }
                }))
                .w(px(14.))
                .text_color(rgb(MUTED))
                .when(directory, |toggle| {
                  toggle.child(Icon::new(if self.expanded_folders.contains(&path) { IconName::ChevronDown } else { IconName::ChevronRight }).size(px(14.)))
                }),
            )
            .child(
              Icon::new(if directory {
                if self.expanded_folders.contains(&path) { IconName::FolderOpen } else { IconName::Folder }
              } else {
                IconName::FileText
              })
              .size(px(16.))
              .text_color(rgb(if directory { Warning } else { MUTED })),
            )
            .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().child(label))
            .child(div().text_size(px(10.)).text_color(rgb(Accent)).child(marker))
            .child(
              div()
                .text_size(px(10.))
                .text_color(rgb(Warning))
                .child(if !directory && self.locked_paths.contains(&relative) { "L" } else { "" }),
            )
            .child(div().text_size(px(10.)).text_color(rgb(MUTED)).child(if directory { String::new() } else { format_size(entry.size) }))
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
              window.focus(&this.files_focus, cx);
              let modifiers = event.modifiers();
              let additive = modifiers.control || modifiers.platform;
              // A preview from the first click may still be loading.
              if !directory && event.click_count() == 2 && !additive && !modifiers.shift {
                this.open_selected_file(cx);
                return;
              }
              if this.busy {
                return;
              }
              if directory && event.click_count() == 2 && !additive && !modifiers.shift {
                this.toggle_tree_folder(path.clone(), cx);
              } else if !directory && !additive && !modifiers.shift {
                this.select(relative.clone(), cx);
              } else {
                let query = this.filter.read(cx).content.to_lowercase();
                let visible: Vec<String> = this
                  .entries
                  .iter()
                  .filter(|entry| entry.path.file_name().unwrap_or_default().to_string_lossy().to_lowercase().contains(&query))
                  .take(2000)
                  .map(|entry| entry.path.strip_prefix(&this.root).unwrap_or(&entry.path).to_string_lossy().replace('\\', "/"))
                  .collect();
                if modifiers.shift {
                  let anchor = this.selection.anchor.as_ref().and_then(|anchor| visible.iter().position(|p| p == anchor));
                  let end = visible.iter().position(|p| p == &relative);
                  if !additive {
                    this.selection.paths.clear();
                  }
                  if let (Some(start), Some(end)) = (anchor, end) {
                    this.selection.paths.extend(visible[start.min(end)..=start.max(end)].iter().cloned());
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
                this.selection.current = if this.selection.paths.contains(&relative) {
                  Some(relative.clone())
                } else {
                  this.selection.paths.iter().min().cloned()
                };
                this.notice = tf("{count} items selected", &[("count", this.selection.paths.len().to_string())]);
                cx.notify();
              }
            }))
            .context_menu(move |menu, _window, cx| {
              let selected_paths = view
                .upgrade()
                .map(|entity| {
                  let lens = entity.read(cx);
                  let selected = lens.selection.paths.contains(&context_relative);
                  if !selected {
                    return vec![context_relative.clone()];
                  }
                  lens
                    .entries
                    .iter()
                    .filter_map(|entry| {
                      let relative = entry.path.strip_prefix(&lens.root).unwrap_or(&entry.path).to_string_lossy().replace('\\', "/");
                      lens.selection.paths.contains(&relative).then_some(relative)
                    })
                    .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| vec![context_relative.clone()]);
              // A multi-selection must never fall through to clicked-item actions.
              if selected_paths.len() > 1 {
                let Some(entity) = view.upgrade() else {
                  return menu;
                };
                let lens = entity.read(cx);
                if lens.root != context_root {
                  return menu;
                }
                let ready = !lens.busy;
                let vcs = ready && lens.connected;
                let shortcuts = lens.settings.shortcuts.clone();
                let has_folders = selected_paths.iter().any(|p| lens.root.join(p).is_dir());
                let changes: Vec<_> = lens
                  .status
                  .changes
                  .iter()
                  .filter(|c| {
                    selected_paths
                      .iter()
                      .any(|p| c.path == *p || (lens.root.join(p).is_dir() && std::path::Path::new(&c.path).starts_with(p)))
                  })
                  .cloned()
                  .collect();
                let mut menu = menu;
                for (action, label) in [("stage", "Stage selected"), ("unstage", "Unstage selected"), ("reset", "Revert selected files")] {
                  let paths: Vec<_> = changes
                    .iter()
                    .filter(|c| match action {
                      "stage" => !c.staged,
                      "unstage" => c.staged,
                      _ => true,
                    })
                    .map(|c| c.path.clone())
                    .collect();
                  let enabled = vcs && !paths.is_empty();
                  let targets = selected_paths.clone();
                  let root = context_root.clone();
                  let action_view = view.clone();
                  let shortcut = match action {
                    "stage" => "stage",
                    "unstage" => "unstage",
                    _ => "revert",
                  };
                  menu = menu.item(
                    PopupMenuItem::new(shortcuts::shortcut_label(&shortcuts, label, shortcut))
                      .disabled(!enabled)
                      .on_click(move |_, window, cx| {
                        let _ = action_view.update(cx, |this, cx| {
                          if this.busy || !this.connected || this.root != root {
                            return;
                          }
                          if has_folders {
                            this.folder_changes_dialog(targets.clone(), action, window, cx);
                          } else if action == "stage" {
                            if !this.pending_paths_valid(&paths, Some(false)) {
                              return;
                            }
                            let mut args = vec!["stage".into(), "--".into()];
                            args.extend(paths.clone());
                            this.command(args, label, false, true, cx);
                          } else {
                            this.revert_dialog(paths.clone(), action == "unstage", window, cx);
                          }
                        });
                      }),
                  );
                }
                let sources: Vec<_> = selected_paths.iter().map(|p| context_root.join(p)).collect();
                let copy = sources.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n");
                let delete_view = view.clone();
                let root = context_root.clone();
                menu = menu
                  .separator()
                  .item(PopupMenuItem::new(t("Copy selected full paths")).on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                  }))
                  .item(
                    PopupMenuItem::new(shortcuts::shortcut_label(&shortcuts, "Delete…", "delete"))
                      .disabled(!ready)
                      .on_click(move |_, window, cx| {
                        let _ = delete_view.update(cx, |this, cx| {
                          if this.root == root {
                            this.delete_files_dialog(sources.clone(), window, cx);
                          }
                        });
                      }),
                  );
                if lens.obliterate_enabled && !has_folders {
                  let root = context_root.clone();
                  let action_view = view.clone();
                  menu = menu.item(PopupMenuItem::new(t("Obliterate…")).disabled(!vcs).on_click(move |_, window, cx| {
                    let _ = action_view.update(cx, |this, cx| {
                      if this.root == root {
                        this.obliterate_files_dialog(selected_paths.clone(), window, cx);
                      }
                    });
                  }));
                }
                return menu;
              }
              let selected_changes = view
                .upgrade()
                .map(|entity| {
                  let lens = entity.read(cx);
                  lens
                    .status
                    .changes
                    .iter()
                    .filter(|change| selected_paths.iter().any(|path| change.path == *path || std::path::Path::new(&change.path).starts_with(path)))
                    .map(|change| change.path.clone())
                    .collect::<Vec<_>>()
                })
                .unwrap_or_default();
              let selected_staged: Vec<_> = view
                .upgrade()
                .map(|entity| {
                  let lens = entity.read(cx);
                  lens
                    .status
                    .changes
                    .iter()
                    .filter(|change| change.staged && selected_changes.contains(&change.path))
                    .map(|change| change.path.clone())
                    .collect()
                })
                .unwrap_or_default();
              let selected_unstaged: Vec<_> = view
                .upgrade()
                .map(|entity| {
                  let lens = entity.read(cx);
                  lens
                    .status
                    .changes
                    .iter()
                    .filter(|change| !change.staged && selected_changes.contains(&change.path))
                    .map(|change| change.path.clone())
                    .collect()
                })
                .unwrap_or_default();
              let shortcut_settings = view.upgrade().map(|entity| entity.read(cx).settings.shortcuts.clone()).unwrap_or_default();
              let bulk_enabled = view.upgrade().is_some_and(|entity| {
                let lens = entity.read(cx);
                !lens.busy && lens.connected && lens.root == context_root
              });
              let mut menu = menu;
              if selected_paths.len() > 1 {
                let copy_paths = selected_paths.iter().map(|path| context_root.join(path).to_string_lossy().into_owned()).collect::<Vec<_>>().join("\n");
                menu = menu.item(PopupMenuItem::new(t("Copy selected full paths")).on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy_paths.clone()))));
              }
              if !directory
                && view.upgrade().is_some_and(|entity| {
                  let lens = entity.read(cx);
                  lens.root == context_root && lens.status.changes.iter().any(|change| change.path == context_relative && change.conflict)
                })
              {
                let resolve_view = view.clone();
                let resolve_root = context_root.clone();
                let resolve_path = context_relative.clone();
                menu = menu.item(
                  PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, "Resolve", "diff"))
                    .disabled(!bulk_enabled)
                    .on_click(move |_, window, cx| {
                      let _ = resolve_view.update(cx, |this, cx| {
                        if this.root == resolve_root && this.status.changes.iter().any(|change| change.path == resolve_path && change.conflict) {
                          this.resolve_or_diff(resolve_path.clone(), window, cx);
                        }
                      });
                    }),
                );
              }
              if !selected_unstaged.is_empty() {
                let action_view = view.clone();
                let root = context_root.clone();
                menu = menu.item(
                  PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, "Stage selected", "stage"))
                    .disabled(!bulk_enabled)
                    .on_click(move |_, _, cx| {
                      let _ = action_view.update(cx, |this, cx| {
                        if this.root != root || !this.pending_paths_valid(&selected_unstaged, Some(false)) {
                          return;
                        }
                        let mut args = vec!["stage".into(), "--".into()];
                        args.extend(selected_unstaged.clone());
                        this.command(args, "Stage selected", false, true, cx);
                      });
                    }),
                );
              }
              if !selected_staged.is_empty() {
                let action_view = view.clone();
                let root = context_root.clone();
                menu = menu.item(
                  PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, "Unstage selected", "unstage"))
                    .disabled(!bulk_enabled)
                    .on_click(move |_, _, cx| {
                      let _ = action_view.update(cx, |this, cx| {
                        if this.root != root || !this.pending_paths_valid(&selected_staged, Some(true)) {
                          return;
                        }
                        let mut args = vec!["unstage".into(), "--".into()];
                        args.extend(selected_staged.clone());
                        this.command(args, "Unstage selected", false, true, cx);
                      });
                    }),
                );
              }
              if selected_paths.len() > 1 || !selected_changes.is_empty() {
                menu = menu.separator();
              }
              let menu = if directory {
                let folder_view = view.clone();
                let folder_path = context_relative.clone();
                let folder_root = context_root.clone();
                let enabled = view.upgrade().is_some_and(|entity| {
                  let lens = entity.read(cx);
                  !lens.busy && lens.connected && lens.root == folder_root && lens.status.changes.iter().any(|c| std::path::Path::new(&c.path).starts_with(&folder_path))
                });
                menu
                  .item(
                    PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, "Revert folder", "revert"))
                      .disabled(!enabled)
                      .on_click(move |_, window, cx| {
                        let _ = folder_view.update(cx, |this, cx| {
                          if this.root == folder_root {
                            this.folder_changes_dialog(vec![folder_path.clone()], "reset", window, cx);
                          }
                        });
                      }),
                  )
                  .separator()
              } else {
                menu
              };
              let menu = if !directory
                && view.upgrade().is_some_and(|entity| {
                  let lens = entity.read(cx);
                  lens.root == context_root && lens.status.changes.iter().any(|change| change.path == context_relative && change.file_marker() == "M")
                }) {
                let diff_view = view.clone();
                let diff_path = context_relative.clone();
                let diff_root = context_root.clone();
                let enabled = view.upgrade().is_some_and(|entity| {
                  let lens = entity.read(cx);
                  lens.connected && !lens.busy && lens.root == context_root
                });
                menu
                  .item(
                    PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, "Diff", "diff"))
                      .disabled(!enabled)
                      .on_click(move |_, _, cx| {
                        let _ = diff_view.update(cx, |this, cx| {
                          if this.root == diff_root && this.status.changes.iter().any(|change| change.path == diff_path && change.file_marker() == "M") {
                            this.external_diff(diff_path.clone(), cx);
                          }
                        });
                      }),
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
                let mut changes = lens
                  .status
                  .changes
                  .iter()
                  .filter(|c| c.path == context_relative || (directory && std::path::Path::new(&c.path).starts_with(&context_relative)))
                  .peekable();
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
                    PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, label, if unstage { "unstage" } else { "stage" }))
                      .disabled(!enabled)
                      .on_click(move |_, window, cx| {
                        let _ = stage_view.update(cx, |this, cx| {
                          if this.busy
                            || !this.connected
                            || this.root != stage_root
                            || !this
                              .status
                              .changes
                              .iter()
                              .any(|c| (c.path == stage_path || (directory && std::path::Path::new(&c.path).starts_with(&stage_path))) && (!unstage || c.staged))
                          {
                            return;
                          }
                          if directory {
                            this.folder_changes_dialog(vec![stage_path.clone()], if unstage { "unstage" } else { "stage" }, window, cx);
                            return;
                          }
                          this.command(vec![if unstage { "unstage" } else { "stage" }.into(), "--".into(), stage_path.clone()], label, false, true, cx);
                        });
                      }),
                  );
                }
                menu.separator()
              } else {
                menu
              };
              let menu = if !directory
                && view.upgrade().is_some_and(|entity| {
                  let lens = entity.read(cx);
                  lens.root == context_root && lens.status.changes.iter().any(|c| c.path == context_relative)
                }) {
                let discard_view = view.clone();
                let discard_path = context_relative.clone();
                let discard_root = context_root.clone();
                menu
                  .item(PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, "Discard", "revert")).on_click(move |_, _, cx| {
                    let _ = discard_view.update(cx, |this, cx| {
                      if this.busy || this.root != discard_root || !this.status.changes.iter().any(|c| c.path == discard_path) || this.root.join(&discard_path).is_dir() {
                        return;
                      }
                      this.command(vec!["reset".into(), "--purge".into(), "--".into(), discard_path.clone()], "Discard", false, true, cx);
                    });
                  }))
                  .separator()
              } else {
                menu
              };
              let move_view = view.clone();
              let bookmark_view = view.clone();
              let bookmark_path = context_path.clone();
              let bookmark_root = context_root.clone();
              let bookmarked = view.upgrade().is_some_and(|entity| entity.read(cx).settings.is_bookmarked(&context_root, &context_path));
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
                .item(PopupMenuItem::new(t("Move…")).on_click(move |_, window, cx| {
                  let _ = move_view.update(cx, |this, cx| {
                    if this.root == move_root {
                      this.move_dialog(move_path.clone(), window, cx);
                    }
                  });
                }))
                .separator();
              let terminal_view = view.clone();
              let menu = menu
                .item(PopupMenuItem::new(t("Copy full path")).on_click(move |_, _, cx| {
                  cx.write_to_clipboard(ClipboardItem::new_string(copy_path.to_string_lossy().into_owned()));
                }))
                .item(
                  PopupMenuItem::new(shortcuts::shortcut_label(
                    &shortcut_settings,
                    if cfg!(target_os = "macos") { "Show in Finder" } else { "Show in Explorer" },
                    "reveal",
                  ))
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
              let menu = menu.item(
                PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, if bookmarked { "Remove bookmark" } else { "Add bookmark" }, "bookmark")).on_click(move |_, _, cx| {
                  let _ = bookmark_view.update(cx, |this, cx| {
                    if this.root != bookmark_root {
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
              let menu = if !directory {
                let state = std::rc::Rc::new(std::cell::RefCell::new(None::<Result<bool, String>>));
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
                let lock_shortcuts = shortcut_settings.clone();
                let task = cx.background_executor().spawn(async move {
                  backend::run_as(
                    &cli,
                    &root,
                    &["lock".into(), "status".into(), "--".into(), query_path.to_string_lossy().into_owned()],
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
                menu
                  .item(
                    PopupMenuItem::element(move |_, _| {
                      div().child(shortcuts::shortcut_label(
                        &lock_shortcuts,
                        match display_state.borrow().as_ref() {
                          None => "Checking lock status…",
                          Some(Ok(true)) => "Unlock",
                          Some(Ok(false)) => "Lock",
                          Some(Err(_)) => "Lock status unavailable — reopen to retry",
                        },
                        "lock",
                      ))
                    })
                    .on_click(move |_, _, cx| {
                      let Some(Ok(locked)) = action_state.borrow().as_ref().cloned() else {
                        return;
                      };
                      let _ = action_view.update(cx, |this, cx| {
                        if this.root != original_root {
                          return;
                        }
                        this.command(
                          vec![
                            "lock".into(),
                            if locked { "release" } else { "acquire" }.into(),
                            "--".into(),
                            action_path.to_string_lossy().into_owned(),
                          ],
                          if locked { "Unlock" } else { "Lock" },
                          false,
                          true,
                          cx,
                        );
                      });
                    }),
                  )
                  .separator()
              } else {
                menu
              };
              let menu = menu.item(
                PopupMenuItem::new(shortcuts::shortcut_label(&shortcut_settings, "Delete…", "delete"))
                  .disabled(!delete_enabled)
                  .on_click(move |_, window, cx| {
                    let _ = delete_view.update(cx, |this, cx| {
                      if this.root == delete_root {
                        this.delete_dialog(delete_path.clone(), window, cx);
                      }
                    });
                  }),
              );
              if !directory && view.upgrade().is_some_and(|entity| entity.read(cx).obliterate_enabled) {
                let action_view = view.clone();
                let path = context_relative.clone();
                let root = context_root.clone();
                let enabled = view.upgrade().is_some_and(|entity| {
                  let lens = entity.read(cx);
                  !lens.busy && lens.connected && lens.root == root && backend::obliterate_args(&root, &path).is_ok()
                });
                menu.item(PopupMenuItem::new(t("Obliterate…")).disabled(!enabled).on_click(move |_, window, cx| {
                  let _ = action_view.update(cx, |this, cx| {
                    if this.root == root {
                      this.obliterate_dialog(path.clone(), window, cx);
                    }
                  });
                }))
              } else {
                menu
              }
            }),
        ),
      );
    }
    if count == 0 {
      files = files.child(div().p_4().text_color(rgb(MUTED)).child(t("No matching files.")));
    }
    div()
      .id("file-browser-drop-target")
      .when(ready, |panel| {
        panel
          .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(rgb(Selected)))
          .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
            cx.stop_propagation();
            this.copy_dropped_paths(paths, this.directory.clone(), cx);
          }))
      })
      .size_full()
      .flex()
      .flex_col()
      .min_h_0()
      .bg(rgb(Sidebar))
      .border_1()
      .border_color(rgb(BORDER))
      .rounded_lg()
      .overflow_hidden()
      .child(
        div()
          .px_3()
          .h(px(44.))
          .flex_shrink_0()
          .border_b_1()
          .border_color(rgb(BORDER))
          .text_size(px(11.))
          .text_color(rgb(MUTED))
          .font_weight(FontWeight::SEMIBOLD)
          .flex()
          .items_center()
          .gap_2()
          .child(t("FILES")),
      )
      .child(div().px_3().pt_3().font_weight(FontWeight::SEMIBOLD).child(title.clone()))
      .child(
        div()
          .px_3()
          .py_2()
          .text_size(px(12.))
          .text_color(rgb(MUTED))
          .overflow_hidden()
          .text_ellipsis()
          .child(self.root.display().to_string()),
      )
      .child(
        div()
          .mx_3()
          .mb_2()
          .pl_2()
          .flex()
          .flex_shrink_0()
          .items_center()
          .bg(rgb(BG))
          .overflow_hidden()
          .border_1()
          .border_color(rgb(BORDER))
          .rounded_md()
          .child(Icon::new(IconName::Search).size(px(16.)).text_color(rgb(MUTED)))
          .child(div().flex_1().min_w_0().child(self.filter.clone())),
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
          .child(self.button("up", "↑", ready && self.directory != self.root).on_click(cx.listener(|this, _, _, cx| {
            if this.busy || this.directory == this.root {
              return;
            }
            if let Some(parent) = this.directory.parent() {
              this.directory = parent.to_path_buf();
              this.load_directory(cx);
            }
          })))
          .child(
            div()
              .flex_1()
              .text_size(px(11.))
              .overflow_hidden()
              .text_ellipsis()
              .child(format!("/{}", self.directory.strip_prefix(&self.root).unwrap_or(&self.directory).to_string_lossy().replace('\\', "/"))),
          ),
      )
      .child(
        div()
          .relative()
          .flex_1()
          .min_h_0()
          .flex()
          .flex_col()
          .child(files)
          .child(gpui_component::scroll::Scrollbar::vertical(&self.files_scroll).mode(gpui_component::scroll::ScrollbarMode::Always)),
      )
      .when(self.entries.len() > 2000, |d| {
        d.child(div().p_2().text_size(px(10.)).text_color(rgb(MUTED)).child(t("Up to 2,000 matches. Filter to narrow.")))
      })
  }
}
