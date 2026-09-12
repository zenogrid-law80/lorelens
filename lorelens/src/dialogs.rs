use super::*;
use gpui_component::scroll::ScrollableElement as _;

impl Lens {
  pub(super) fn sparse_workspace_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy {
      return;
    }
    let root = self.root.clone();
    let original = match backend::sparse::load(&root) {
      Ok(view) => std::rc::Rc::new(view),
      Err(error) => {
        self.error = true;
        self.notice = error;
        cx.notify();
        return;
      }
    };
    let branch = self.status.branch.clone();
    let editor = cx.new(|cx| sparse_editor::SparseEditor::new(root.clone(), self.cli.clone(), self.settings.identity.clone(), original.contents.clone().unwrap_or_default(), cx));
    let failure = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let view = cx.entity().downgrade();
    window.open_dialog(cx, move |dialog, _, cx| {
      let editor = editor.clone();
      let original = original.clone();
      let root = root.clone();
      let branch = branch.clone();
      let view = view.clone();
      let close_view = view.clone();
      let close_root = root.clone();
      let failure = failure.clone();
      let error_text = failure.borrow().clone();
      dialog
        .title(t("Sparse workspace"))
        .w(px(720.))
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("sparse-save", t("Save"), true))
        .child(div().text_sm().child(root.display().to_string()))
        .child(editor.clone())
        .child(t(
          "Save updates only the local view file. Later Lore operations may download or remove files to match these rules. Review pending changes before syncing.",
        ))
        .child(div().text_sm().text_color(palette(cx)(Danger)).child(error_text))
        .on_close(move |_, _, cx| {
          let _ = close_view.update(cx, |this, cx| {
            if this.root == close_root {
              this.refresh(cx);
            }
          });
        })
        .on_ok(move |_, window, cx| {
          let changes = match editor.read(cx).selections() {
            Ok(changes) => changes,
            Err(error) => {
              *failure.borrow_mut() = error;
              window.refresh();
              return false;
            }
          };
          let result = view.update(cx, |this, cx| {
            if this.busy || this.root != root || this.status.branch != branch {
              return Err(t("The workspace changed or is busy. Reopen the sparse editor."));
            }
            backend::sparse::merge_selections(&root, &original, &changes)?;
            this.error = false;
            this.notice = t("Sparse workspace rules saved.");
            cx.notify();
            Ok(())
          });
          match result {
            Ok(Ok(())) => true,
            Ok(Err(error)) => {
              *failure.borrow_mut() = error;
              window.refresh();
              false
            }
            Err(_) => false,
          }
        })
    });
  }

  pub(super) fn resolve_or_diff(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy || !self.connected {
      return;
    }
    if !self.status.changes.iter().any(|change| change.path == path && change.conflict) || !backend::is_binary_merge(&self.root, &path) {
      self.external_diff(path, cx);
      return;
    }
    let root = self.root.clone();
    let branch = self.status.branch.clone();
    let view = cx.entity().downgrade();
    window.open_dialog(cx, move |dialog, _, _| {
      let mut choices = div().flex().gap_2();
      for (id, label, side) in [
        ("binary-base", "Mine (Base)", backend::BinarySide::Base),
        ("binary-theirs", "Remote (Theirs)", backend::BinarySide::Theirs),
      ] {
        let view = view.clone();
        let root = root.clone();
        let branch = branch.clone();
        let path = path.clone();
        choices = choices.child(Button::new(id).label(t(label)).on_click(move |_, window, cx| {
          window.close_dialog(cx);
          let _ = view.update(cx, |this, cx| {
            if this.busy || !this.connected || this.root != root || this.status.branch != branch || !this.status.changes.iter().any(|change| change.path == path && change.conflict) {
              return;
            }
            this.busy = true;
            this.preview.invalidate();
            let root = root.clone();
            let path = path.clone();
            let task_path = path.clone();
            let task = cx.background_executor().spawn(async move { backend::select_binary_merge(&root, &task_path, side) });
            cx.spawn(async move |this, cx| {
              let result = task.await;
              let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                  Ok(()) => this.command(vec!["branch".into(), "merge".into(), "resolve".into(), "--".into(), path], "Resolve merge conflict", false, true, cx),
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
          });
        }));
      }
      dialog
        .title(t("Resolve binary merge"))
        .footer(dialog_footer("resolve-binary-cancel", t("Cancel"), false))
        .child(tf("Choose the file to overwrite {path}. Merge backups will be deleted after resolution.", &[("path", path.clone())]))
        .child(choices)
    });
  }

  pub(super) fn theme_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let themes = gpui_component::ThemeRegistry::global(cx).sorted_themes();
    let mut light = themes.iter().filter(|theme| !theme.mode.is_dark()).map(|theme| theme.name.to_string()).collect::<Vec<_>>();
    let mut dark = themes.iter().filter(|theme| theme.mode.is_dark()).map(|theme| theme.name.to_string()).collect::<Vec<_>>();
    light.sort_by_key(|name| name != theme::LIGHT_THEME);
    dark.sort_by_key(|name| name != theme::DARK_THEME);
    let show_dark = std::rc::Rc::new(std::cell::Cell::new(
      themes
        .iter()
        .find(|theme| theme.name.as_ref() == self.settings.theme)
        .map(|theme| theme.mode.is_dark())
        .unwrap_or_else(|| gpui_component::Theme::global(cx).is_dark()),
    ));
    // The dialog is rendered while Lens is borrowed; keep its selection locally.
    let selection = std::rc::Rc::new(std::cell::RefCell::new(self.settings.theme.clone()));
    let view = cx.entity().downgrade();
    window.open_dialog(cx, move |dialog, _, cx| {
      let current = selection.borrow().clone();
      let active_theme = gpui_component::Theme::global(cx).theme_name().to_string();
      let system_selection = selection.clone();
      let system_view = view.clone();
      let dark_selected = show_dark.get();
      let mut theme_list = div().flex().flex_col().gap_1();
      for (index, name) in if dark_selected { &dark } else { &light }.iter().enumerate() {
        let theme_view = view.clone();
        let theme_selection = selection.clone();
        let theme = name.clone();
        let label = if theme == theme::LIGHT_THEME || theme == theme::DARK_THEME {
          tf("{theme} (Default)", &[("theme", theme.clone())])
        } else {
          theme.clone()
        };
        let label = if current == theme { format!("✓ {label}") } else { label };
        theme_list = theme_list.child(Button::new(("theme-option", index)).label(label).w_full().on_click(move |_, window, cx| {
          let selected_theme = theme.clone();
          *theme_selection.borrow_mut() = selected_theme.clone();
          let _ = theme_view.update(cx, |this, cx| {
            this.settings.theme = theme.clone();
            this.save_settings();
            cx.notify();
          });
          defer_theme(selected_theme, window, cx);
        }));
      }
      let light_mode = show_dark.clone();
      let dark_mode = show_dark.clone();
      let default_label = t("Default theme: LoreLens Light / Dark");
      dialog
        .title(t("Choose theme"))
        .w(px(520.))
        .footer(dialog_footer("theme-close", t("Close"), false))
        .child(
          Button::new("system-theme")
            .label(if current == theme::DEFAULT_THEME { format!("✓ {default_label}") } else { default_label })
            .w_full()
            .on_click(move |_, window, cx| {
              *system_selection.borrow_mut() = theme::DEFAULT_THEME.into();
              let _ = system_view.update(cx, |this, cx| {
                this.settings.theme = theme::DEFAULT_THEME.into();
                this.save_settings();
                cx.notify();
              });
              defer_theme(theme::DEFAULT_THEME, window, cx);
            }),
        )
        .child(
          div()
            .text_sm()
            .text_color(palette(cx)(MUTED))
            .child(t("Automatically switches between light and dark to match your system settings.")),
        )
        .child(div().text_sm().child(tf("Current theme: {theme}", &[("theme", active_theme)])))
        .child(
          div()
            .flex()
            .gap_2()
            .child(
              Button::new("light-theme-mode")
                .label(if dark_selected { t("Light") } else { format!("✓ {}", t("Light")) })
                .on_click(move |_, window, _| {
                  light_mode.set(false);
                  window.refresh();
                }),
            )
            .child(
              Button::new("dark-theme-mode")
                .label(if dark_selected { format!("✓ {}", t("Dark")) } else { t("Dark") })
                .on_click(move |_, window, _| {
                  dark_mode.set(true);
                  window.refresh();
                }),
            ),
        )
        .child(div().h(px(460.)).min_h_0().overflow_y_scrollbar().pr_2().child(theme_list))
    });
  }

  pub(super) fn custom_tool_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy {
      return;
    }
    let name = cx.new(|cx| TextInput::new("Application name", cx));
    name.update(cx, |input, _| input.content = self.settings.custom_tool_name.clone().into());
    let location = cx.new(|cx| TextInput::new("Application location", cx));
    location.update(cx, |input, _| input.content = self.settings.custom_tool_path.to_string_lossy().into_owned().into());
    let arguments = cx.new(|cx| TextInput::new("Arguments", cx));
    arguments.update(cx, |input, _| input.content = self.settings.custom_tool_arguments.clone().into());
    let presets = [
      ("RustRover / IntelliJ", "idea", "merge {theirs} {yours} {base} {result}"),
      ("P4Merge", "p4merge", "{base} {theirs} {yours} {result}"),
      ("TortoiseGitMerge", "TortoiseGitMerge", "/base:{base} /mine:{yours} /theirs:{theirs} /merged:{result}"),
      ("WinMerge", "WinMergeU", "/e /u /wl /wm /wr {base} {yours} {theirs} /o {result}"),
    ];
    let view = cx.entity().downgrade();
    let validation = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    window.open_dialog(cx, move |dialog, _, _| {
      let name_input = name.clone();
      let location_input = location.clone();
      let arguments_input = arguments.clone();
      let view = view.clone();
      let validation = validation.clone();
      let error = validation.borrow().clone();
      dialog
        .title(t("Custom diff / merge tool"))
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("custom-tool-save", t("Save"), true))
        .child(t("Application name"))
        .child(name.clone())
        .child(t("Application location"))
        .child(location.clone())
        .child(t("Arguments"))
        .child(arguments.clone())
        .child({
          let (name, location, arguments) = (name.clone(), location.clone(), arguments.clone());
          Button::new("custom-tool-preset").label(t("Tool preset…")).dropdown_menu(move |mut menu, _, _| {
            for (label, tool, template) in presets {
              let (name, location, arguments) = (name.clone(), location.clone(), arguments.clone());
              let executable = external_tools::suggested_executable(tool).unwrap_or_default().to_string_lossy().into_owned();
              menu = menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                for (input, value) in [(&name, label), (&location, executable.as_str()), (&arguments, template)] {
                  input.update(cx, |input, cx| {
                    input.reset();
                    input.content = value.to_string().into();
                    cx.notify();
                  });
                }
              }));
            }
            let (name, location, arguments) = (name.clone(), location.clone(), arguments.clone());
            menu.separator().item(PopupMenuItem::new(t("Reset")).on_click(move |_, _, cx| {
              for (input, value) in [(&name, "Custom"), (&location, ""), (&arguments, "{base} {yours}")] {
                input.update(cx, |input, cx| {
                  input.reset();
                  input.content = value.into();
                  cx.notify();
                });
              }
            }))
          })
        })
        .child(t("Placeholders: {base}, {theirs}, {yours}, {result}"))
        .child(t(&error))
        .on_ok(move |_, window, cx| {
          let name = name_input.read(cx).content.trim().to_string();
          let path = PathBuf::from(location_input.read(cx).content.trim());
          let arguments = arguments_input.read(cx).content.trim().to_string();
          if name.is_empty() || !path.is_absolute() || !external_tools::is_executable(&path) || arguments.is_empty() {
            *validation.borrow_mut() = "Enter a name, an absolute executable path, and arguments.".into();
            window.refresh();
            return false;
          }
          if external_tools::arguments(
            "custom",
            Some(&arguments),
            false,
            std::path::Path::new("base"),
            std::path::Path::new("theirs"),
            std::path::Path::new("yours"),
            std::path::Path::new("result"),
          )
          .is_err()
          {
            *validation.borrow_mut() = "Check the argument quotes and try again.".into();
            window.refresh();
            return false;
          }
          view
            .update(cx, |this, cx| {
              this.settings.custom_tool_name = name;
              this.settings.custom_tool_path = canonicalize_path(path);
              this.settings.custom_tool_arguments = arguments;
              this.settings.external_tool = "custom".into();
              let saved = this.save_settings();
              cx.notify();
              saved
            })
            .unwrap_or(false)
        })
    });
  }

  pub(super) fn local_commits_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy || !self.connected {
      return;
    }
    let items = self.pending_push.clone();
    let root = self.root.clone();
    let branch = self.status.branch.clone();
    let identity = self.settings.identity.clone();
    let view = cx.entity().downgrade();
    let validation = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    window.open_dialog(cx, move |dialog, _, _| {
      let (root, branch, identity, view, items, validation) = (root.clone(), branch.clone(), identity.clone(), view.clone(), items.clone(), validation.clone());
      let rows = items.clone().unwrap_or_default();
      let error = items.as_ref().err().cloned().unwrap_or_default();
      let validation_text = validation.borrow().clone();
      dialog
        .title(t("Unpushed local commits"))
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("discard-local-commits", t("Discard all unpushed commits"), true))
        .child(
          div()
            .flex()
            .flex_col()
            .gap_2()
            .child(format!("{} · {}", root.display(), branch))
            .child(tf("{count} unpushed local commits", &[("count", rows.len().to_string())]))
            .child(
              uniform_list("local-commit-list", rows.len(), move |range, _, _| {
                range.map(|i| div().h(px(26.)).overflow_hidden().text_ellipsis().child(rows[i].label())).collect::<Vec<_>>()
              })
              .h(px(200.)),
            )
            .child(t(&error))
            .child(t(
              "Discarding all commits restores working files to the remote revision. Commit changes will not remain staged. Working changes must be resolved first.",
            ))
            .child(validation_text),
        )
        .on_ok(move |_, window, cx| {
          view
            .update(cx, |this, cx| {
              let valid =
                !this.busy && this.connected && this.root == root && this.status.branch == branch && this.settings.identity == identity && this.status.changes.is_empty() && this.pending_push == items;
              let commits = items.clone().unwrap_or_default();
              if !valid || commits.is_empty() {
                *validation.borrow_mut() = t("Working changes or a query error exist, or there are no commits to discard. Refresh and try again.");
                window.refresh();
                return false;
              }
              this.busy = true;
              this.error = false;
              this.notice = "Checking and discarding unpushed commits…".into();
              this.preview.invalidate();
              let (cli, root, branch, identity) = (this.cli.clone(), root.clone(), branch.clone(), identity.clone());
              let task = cx
                .background_executor()
                .spawn(async move { backend::discard_local_commits(&cli, &root, identity.as_deref(), &branch, &commits) });
              cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                  this.busy = false;
                  match result {
                    Ok(output) => {
                      this.log(output);
                      this.notice = "Unpushed commits discarded.".into();
                    }
                    Err(error) => {
                      this.log(error.clone());
                      this.error = true;
                      this.notice = error;
                    }
                  }
                  this.refresh(cx);
                  cx.notify();
                });
              })
              .detach();
              cx.notify();
              true
            })
            .unwrap_or(false)
        })
    });
  }

  pub(super) fn deduplicate_files_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let paths = backend::duplicate_change_paths(&self.status.changes);
    self.deduplicate_paths_dialog(paths, window, cx);
  }

  fn deduplicate_paths_dialog(&mut self, mut paths: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy || !self.connected {
      return;
    }
    paths.sort();
    paths.dedup();
    let duplicates = backend::duplicate_change_paths(&self.status.changes);
    if paths.is_empty() || !paths.iter().all(|path| duplicates.contains(path)) {
      return;
    }
    let root = self.root.clone();
    let branch = self.status.branch.clone();
    let revision = self.status.revision.clone();
    let identity = self.settings.identity.clone();
    let commands = backend::deduplicate_commands(&paths);
    let local_files = match backend::duplicate_local_files(&self.root, &self.status.changes, &paths) {
      Ok(files) => files,
      Err(error) => {
        self.error = true;
        self.notice = "Deduplication failed · see details".into();
        self.log(error);
        cx.notify();
        return;
      }
    };
    let unrelated_staged = self.status.changes.iter().any(|change| change.staged && !paths.contains(&change.path));
    let validation = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let view = cx.entity().downgrade();
    window.open_dialog(cx, move |dialog, _, _| {
      let (paths, root, branch, revision, identity, commands, local_files, validation, view) = (
        paths.clone(),
        root.clone(),
        branch.clone(),
        revision.clone(),
        identity.clone(),
        commands.clone(),
        local_files.clone(),
        validation.clone(),
        view.clone(),
      );
      let error = t(&validation.borrow());
      dialog
        .title(t("Reset Files"))
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("deduplicate-files-confirm", t("Reset Files"), true))
        .child(
          div()
            .flex()
            .flex_col()
            .gap_2()
            .child(root.display().to_string())
            .child(tf("{count} duplicate Change paths found", &[("count", paths.len().to_string())]))
            .child(t("Only paths that appear at least twice in Changes are included."))
            .child(
              div()
                .id("deduplicate-paths")
                .max_h(px(180.))
                .overflow_y_scroll()
                .children(paths.iter().map(|path| div().child(path.clone()))),
            )
            .when(!local_files.is_empty(), |d| {
              d.child(t("Files with both delete Staged and keep Unstaged states will be deleted locally before the workflow runs."))
                .child(
                  div()
                    .id("deduplicate-local-files")
                    .max_h(px(140.))
                    .overflow_y_scroll()
                    .children(local_files.iter().map(|path| div().child(path.clone()))),
                )
            })
            .child(t(
              "This workflow stages the listed files, commits them as \"Remove unavailable local server binaries\", pushes the commit, refreshes status, and verifies the repository.",
            ))
            .child(
              div()
                .id("deduplicate-commands")
                .max_h(px(180.))
                .overflow_y_scroll()
                .children(commands.iter().map(|args| div().child(lore_command_label(args)))),
            )
            .when(unrelated_staged, |d| {
              d.child(t("Other staged files must be unstaged first so they are not included in the automatic commit."))
            })
            .child(error),
        )
        .on_ok(move |_, window, cx| {
          let (paths, root, branch, revision, identity, commands, validation) = (paths.clone(), root.clone(), branch.clone(), revision.clone(), identity.clone(), commands.clone(), validation.clone());
          view
            .update(cx, move |this, cx| {
              let current_paths = backend::duplicate_change_paths(&this.status.changes);
              if this.busy
                || !this.connected
                || this.root != root
                || this.status.branch != branch
                || this.status.revision != revision
                || this.settings.identity != identity
                || !paths.iter().all(|path| current_paths.contains(path))
              {
                *validation.borrow_mut() = "Repository state changed. Reopen this dialog.".into();
                window.refresh();
                return false;
              }
              if this.status.changes.iter().any(|change| change.staged && !paths.contains(&change.path)) {
                *validation.borrow_mut() = "Other staged files must be unstaged first so they are not included in the automatic commit.".into();
                window.refresh();
                return false;
              }
              this.busy = true;
              this.error = false;
              this.notice = "Deduplicating files…".into();
              this.preview.invalidate();
              for args in &commands {
                this.log(lore_command_label(args));
              }
              let cli = this.cli.clone();
              let task = cx
                .background_executor()
                .spawn(async move { backend::deduplicate_files(&cli, &root, identity.as_deref(), &branch, &revision, &paths) });
              cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                  this.busy = false;
                  this.output_title = "Reset Files".into();
                  match result {
                    Ok(output) => {
                      this.output = output;
                      this.notice = "Duplicate Change entries removed and pushed.".into();
                      this.log(t("Duplicate Change entries removed and pushed."));
                    }
                    Err(error) => {
                      this.output = error.clone();
                      this.error = true;
                      this.notice = "Deduplication failed · see details".into();
                      this.log(error);
                    }
                  }
                  this.refresh(cx);
                  cx.notify();
                });
              })
              .detach();
              cx.notify();
              true
            })
            .unwrap_or(false)
        })
    });
  }

  pub(super) fn folder_changes_dialog(&mut self, targets: Vec<String>, action: &'static str, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy || !self.connected {
      return;
    }
    let targets: Vec<_> = targets
      .into_iter()
      .map(|path| {
        let directory = self.root.join(&path).is_dir();
        (path, directory)
      })
      .collect();
    let collect_paths = |recursive| folder_change_paths(&self.status.changes, &targets, action, recursive);
    let direct = std::rc::Rc::new(collect_paths(false));
    let recursive_paths = std::rc::Rc::new(collect_paths(true));
    let recursive = std::rc::Rc::new(std::cell::Cell::new(None::<usize>));
    let validation = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let root = self.root.clone();
    let branch = self.status.branch.clone();
    let revision = self.status.revision.clone();
    let identity = self.settings.identity.clone();
    let view = cx.entity().downgrade();
    let title = match action {
      "stage" => "Stage folder",
      "unstage" => "Unstage folder",
      "reset" => "Reset folder",
      _ => "Revert folder",
    };
    window.open_dialog(cx, move |dialog, _, _| {
      let paths = match recursive.get() {
        Some(0) => direct.clone(),
        Some(1) => recursive_paths.clone(),
        _ => std::rc::Rc::new(Vec::new()),
      };
      let toggle = recursive.clone();
      let recursive = recursive.clone();
      let validation = validation.clone();
      let error = t(&validation.borrow());
      let (root, branch, revision, identity, targets, view) = (root.clone(), branch.clone(), revision.clone(), identity.clone(), targets.clone(), view.clone());
      dialog
        .title(t(title))
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("folder-changes-confirm", t(title), true))
        .child(
          div()
            .flex()
            .flex_col()
            .gap_2()
            .child(root.display().to_string())
            .child(t("Choose whether to apply this command to the selected folders only or include all subfolders."))
            .child(
              gpui_component::radio::RadioGroup::vertical("folder-scope")
                .child(t("Selected folders only (direct files)"))
                .child(t("Include all subfolders"))
                .selected_index(recursive.get())
                .on_click(move |index, window, _| {
                  toggle.set(Some(*index));
                  window.refresh();
                }),
            )
            .child(tf("{count} items selected", &[("count", paths.len().to_string())]))
            .child(
              uniform_list("folder-change-paths", paths.len(), {
                let paths = paths.clone();
                move |range, _, _| range.map(|i| div().h(px(24.)).child(paths[i].clone())).collect::<Vec<_>>()
              })
              .h(px(160.)),
            )
            .when(action == "reset", |d| {
              d.child(t(
                "Restore all listed files to the current committed revision? Deleted files will be restored and staged changes discarded. Local edits will be lost; newly added files will be deleted.",
              ))
            })
            .child(error),
        )
        .on_ok(move |_, window, cx| {
          view
            .update(cx, |this, cx| {
              let Some(scope) = recursive.get() else {
                *validation.borrow_mut() = "Choose a folder scope before continuing.".into();
                window.refresh();
                return false;
              };
              if paths.is_empty() {
                *validation.borrow_mut() = "No matching changes in the selected scope.".into();
                window.refresh();
                return false;
              }
              if this.busy
                || !this.connected
                || this.root != root
                || this.status.branch != branch
                || this.status.revision != revision
                || this.settings.identity != identity
                || folder_change_paths(&this.status.changes, &targets, action, scope == 1) != *paths
              {
                *validation.borrow_mut() = "Repository state changed. Reopen this dialog.".into();
                window.refresh();
                return false;
              }
              if action == "reset" {
                this.command_batch(backend::reset_commands(&this.root, &paths), title, false, true, cx);
              } else {
                let mut args = vec![action.into(), "--".into()];
                args.extend(paths.iter().cloned());
                this.command(args, title, false, true, cx);
              }
              true
            })
            .unwrap_or(false)
        })
    });
  }

  pub(super) fn obliterate_dialog(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
    self.obliterate_files_dialog(vec![path], window, cx);
  }

  pub(super) fn obliterate_files_dialog(&mut self, paths: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy || !self.connected || !self.obliterate_enabled || paths.is_empty() {
      return;
    }
    if let Err(error) = paths.iter().map(|path| backend::obliterate_args(&self.root, path)).collect::<Result<Vec<_>, _>>() {
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
            let (paths, root, branch, revision, identity) =
                (paths.clone(), root.clone(), branch.clone(), revision.clone(), identity.clone());
            let input = confirmation.clone();
            let view = view.clone();
            let validation = validation.clone();
            let error = t(&validation.borrow());
            dialog.title(t("Obliterate file")).close_button(false).overlay_closable(false)
                .footer(dialog_footer("obliterate-confirm", t("Obliterate"), true))
                .child(div().flex().flex_col().gap_2()
                    .child(root.display().to_string())
                    .child(tf("{count} items selected", &[("count", paths.len().to_string())]))
                    .child(div().id("obliterate-paths").max_h(px(240.)).overflow_y_scroll()
                        .children(paths.iter().map(|path| div().child(path.clone()))))
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
                        if this.busy || !this.connected || !this.obliterate_enabled || this.root != root || this.status.branch != branch
                            || this.status.revision != revision || this.settings.identity != identity {
                            *validation.borrow_mut() = "Repository state changed. Reopen this dialog.".into();
                            window.refresh();
                            return false;
                        }
                        match paths.iter().map(|path| backend::obliterate_args(&root, path)).collect::<Result<Vec<_>, _>>() {
                            Ok(args) => {
                                this.selection.clear();
                                this.command_batch(args, "Obliterate file", false, true, cx);
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
    if self.busy {
      return;
    }
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
      dialog
        .title(t("Create repository"))
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("create-repository-confirm", t("Create"), true))
        .child(
          div()
            .flex()
            .flex_col()
            .gap_2()
            .child(t("Create a remote repository and its local workspace. Log in to the server first."))
            .child(t("Repository URL"))
            .child(Self::history_input("create-url-history", url.clone(), urls.clone()))
            .child(t("Destination Path"))
            .child(Self::history_input("create-path-history", destination.clone(), destinations.clone()))
            .child(validation_text),
        )
        .on_close(move |_, _, cx| {
          let _ = close_view.update(cx, |this, cx| {
            this.settings.remember_create(&close_url.read(cx).content, &close_destination.read(cx).content);
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
          } else {
            None
          };
          if let Some(error) = error {
            *validation.borrow_mut() = error.into();
            window.refresh();
            return false;
          }
          view
            .update(cx, |this, cx| {
              if this.busy {
                return false;
              }
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
              let task = cx.background_executor().spawn(async move { backend::create_repository(&cli, &remote, &target, identity.as_deref()) });
              cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                  this.busy = false;
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
              })
              .detach();
              cx.notify();
              true
            })
            .unwrap_or(false)
        })
    });
  }

  pub(super) fn revert_dialog(&mut self, paths: Vec<String>, unstage_only: bool, window: &mut Window, cx: &mut Context<Self>) {
    let title = if unstage_only {
      "Unstage"
    } else if paths.len() > 1 {
      "Revert selected files"
    } else {
      "Revert file"
    };
    self.file_changes_dialog(paths, unstage_only, title, window, cx);
  }

  pub(super) fn reset_dialog(&mut self, paths: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
    self.file_changes_dialog(paths, false, "Reset files", window, cx);
  }

  fn file_changes_dialog(&mut self, mut paths: Vec<String>, unstage_only: bool, title: &'static str, window: &mut Window, cx: &mut Context<Self>) {
    paths.sort();
    paths.dedup();
    if self.busy || !self.connected {
      return;
    }
    if !self.pending_paths_valid(&paths, unstage_only.then_some(true)) {
      return;
    }
    if unstage_only && paths.len() == 1 {
      if self.root.join(&paths[0]).is_dir() {
        self.folder_changes_dialog(paths, "unstage", window, cx);
      } else {
        let mut args = vec!["unstage".into(), "--".into()];
        args.extend(paths);
        self.selection.clear();
        self.command(args, "Unstage", false, true, cx);
      }
      return;
    }
    let root = self.root.clone();
    let revision = self.status.revision.clone();
    let branch = self.status.branch.clone();
    let view = cx.entity().downgrade();
    let validation = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    window.open_dialog(cx, move |dialog, _, _| {
      let paths = paths.clone();
      let root = root.clone();
      let revision = revision.clone();
      let branch = branch.clone();
      let view = view.clone();
      let validation = validation.clone();
      let error = t(&validation.borrow());
      dialog
        .title(t(title))
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("revert-confirm", t(title), true))
        .child(
          div()
            .flex()
            .flex_col()
            .gap_2()
            .child(root.display().to_string())
            .child(tf("{count} items selected", &[("count", paths.len().to_string())]))
            .child(
              div()
                .id("revert-paths")
                .max_h(px(240.))
                .overflow_y_scroll()
                .children(paths.iter().map(|path| div().child(path.clone()))),
            )
            .child(t(if unstage_only {
              "Remove this file from staging? Local file changes will be kept."
            } else if paths.len() > 1 {
              "Restore all listed files to the current committed revision? Deleted files will be restored and staged changes discarded. Local edits will be lost; newly added files will be deleted."
            } else {
              "Restore this file to the current committed revision? Deleted files will be restored and staged changes discarded. Local edits will be lost; newly added files will be deleted."
            }))
            .child(error),
        )
        .on_ok(move |_, window, cx| {
          view
            .update(cx, |this, cx| {
              if this.busy
                || !this.connected
                || this.root != root
                || this.status.branch != branch
                || this.status.revision != revision
                || !this.pending_paths_valid(&paths, unstage_only.then_some(true))
              {
                *validation.borrow_mut() = "Repository state changed. Reopen this dialog.".into();
                window.refresh();
                return false;
              }
              let commands = if unstage_only {
                let mut args = vec!["unstage".into(), "--".into()];
                args.extend(paths.iter().cloned());
                vec![args]
              } else {
                backend::reset_commands(&this.root, &paths)
              };
              this.selection.clear();
              this.command_batch(commands, title, false, true, cx);
              true
            })
            .unwrap_or(true)
        })
    });
  }
}

impl Lens {
  pub(super) fn delete_dialog(&mut self, source: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
    self.delete_files_dialog(vec![source], window, cx);
  }

  pub(super) fn delete_files_dialog(&mut self, sources: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy || sources.is_empty() {
      return;
    }
    let root = self.root.clone();
    let view = cx.entity().downgrade();
    window.open_dialog(cx, move |dialog, _, _| {
      let sources = sources.clone();
      let root = root.clone();
      let view = view.clone();
      dialog
        .title(t("Delete"))
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("delete-confirm", t("Delete"), true))
        .child(
          div()
            .flex()
            .flex_col()
            .gap_2()
            .child(tf("{count} items selected", &[("count", sources.len().to_string())]))
            .child(
              div()
                .id("delete-targets")
                .max_h(px(240.))
                .overflow_y_scroll()
                .children(sources.iter().map(|source| div().child(source.display().to_string()))),
            )
            .child(t("Permanently delete this item? Folders and all their contents will be deleted. This does not use the Recycle Bin.")),
        )
        .on_ok(move |_, _, cx| {
          let root = root.clone();
          let sources = sources.clone();
          view
            .update(cx, |this, cx| {
              if this.busy || this.root != root {
                return false;
              }
              this.busy = true;
              this.preview.invalidate();
              this.notice = "Deleting…".into();
              let task = cx.background_executor().spawn(async move {
                for source in &sources {
                  backend::delete_entry(&root, source).map_err(|error| format!("{}: {error}", source.display()))?;
                }
                Ok::<(), String>(())
              });
              cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                  this.busy = false;
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

impl Lens {
  pub(super) fn logout_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy {
      return;
    }
    let view = cx.entity().downgrade();
    window.open_dialog(cx, move |dialog, _, _| {
      let view = view.clone();
      dialog
        .title(t("Logout"))
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("logout-confirm", t("Logout"), true))
        .child(t("Log out all Lore CLI accounts on this device? URL history and local repositories will be kept."))
        .on_ok(move |_, _, cx| {
          view
            .update(cx, |this, cx| {
              if this.busy {
                return false;
              }
              this.command(vec!["auth".into(), "clear".into()], "Logout", false, false, cx);
              true
            })
            .unwrap_or(false)
        })
    });
  }

  pub(super) fn login_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy {
      return;
    }
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
      dialog
        .title(t("Login"))
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("login-submit", t("Login"), true))
        .child(
          div()
            .flex()
            .flex_col()
            .gap_2()
            .when(no_repository, |el| el.child(t("No local repository. Enter the server URL to log in, then clone a repository.")))
            .child(t("Server URL"))
            .child(Self::history_input("login-remote-url", url.clone(), history.clone()))
            .child(validation_text),
        )
        .on_ok(move |_, window, cx| {
          let remote = input.read(cx).content.trim().to_owned();
          if !remote.contains("://") || remote.ends_with("://") || remote.chars().any(char::is_whitespace) {
            *validation.borrow_mut() = "Enter a server URL including its scheme.".into();
            window.refresh();
            return false;
          }
          view
            .update(cx, |this, cx| {
              if this.busy {
                return false;
              }
              this.settings.remember_login(&remote);
              if !this.save_settings() {
                *validation.borrow_mut() = "Could not save settings · see command log".into();
                window.refresh();
                return false;
              }
              this.command(vec!["login".into(), remote], "Login", false, false, cx);
              true
            })
            .unwrap_or(false)
        })
    });
  }
  pub(super) fn merge_branch_dialog(&mut self, source: String, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy || !self.connected {
      return;
    }
    let name = cx.new(|cx| {
      let mut input = TextInput::new("Select source branch", cx);
      input.content = source.into();
      input
    });
    let target = self.status.branch.clone();
    let branches: Vec<String> = self.local_branches.iter().filter(|branch| **branch != target).cloned().collect();
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
      dialog
        .title(t("Merge local branch"))
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("merge-branch-confirm", t("Merge"), true))
        .child(tf("Target (current branch): {target}", &[("target", target.to_string())]))
        .child(t("Source branch"))
        .child(Self::history_input("merge-source-branch", name.clone(), branches.clone()))
        .child(t("A conflict-free merge creates a local commit automatically. Conflicts are shown in Details."))
        .child(t(&error))
        .on_ok(move |_, window, cx| {
          let branch = input.read(cx).content.trim().to_string();
          view
            .update(cx, |this, cx| {
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
              this.command(vec!["branch".into(), "merge".into(), "--".into(), branch], "Merge local branch", false, true, cx);
              true
            })
            .unwrap_or(true)
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
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("switch-branch-confirm", t("Switch"), true))
        .child(Self::history_input("switch-local-branch", name.clone(), branches.clone()))
        .on_ok(move |_, _, cx| {
          let branch = input.read(cx).content.trim().to_string();
          view
            .update(cx, |this, cx| {
              if this.busy || this.root != root || !this.local_branches.contains(&branch) {
                return false;
              }
              if branch == this.status.branch {
                return true;
              }
              this.command(commands::switch_branch_args(branch), "Switch local branch", false, true, cx);
              true
            })
            .unwrap_or(true)
        })
    });
  }

  pub(super) fn archive_branch_dialog(&mut self, branch: String, scope: commands::BranchArchiveScope, window: &mut Window, cx: &mut Context<Self>) {
    let branch_exists = match scope {
      commands::BranchArchiveScope::Local => self.local_branches.contains(&branch),
      commands::BranchArchiveScope::Remote => self.remote_branches.contains(&branch),
      commands::BranchArchiveScope::LocalAndRemote => self.local_branches.contains(&branch),
    };
    let archives_local = matches!(scope, commands::BranchArchiveScope::Local | commands::BranchArchiveScope::LocalAndRemote);
    if self.busy || !self.connected || !branch_exists || archives_local && branch == self.status.branch {
      return;
    }
    let root = self.root.clone();
    let current = self.status.branch.clone();
    let view = cx.entity().downgrade();
    let (title, confirm) = match scope {
      commands::BranchArchiveScope::Local => ("Delete local branch", "Delete"),
      commands::BranchArchiveScope::Remote => ("Delete remote branch", "Delete"),
      commands::BranchArchiveScope::LocalAndRemote => ("Archive branch", "Archive"),
    };
    window.open_dialog(cx, move |dialog, _, _| {
      let branch = branch.clone();
      let root = root.clone();
      let current = current.clone();
      let view = view.clone();
      dialog
        .title(t(title))
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("archive-branch-confirm", t(confirm), true))
        .child(match scope {
          commands::BranchArchiveScope::Local => tf(
            "Delete local branch '{branch}'? The remote branch will be kept and can be restored by switching to it later.",
            &[("branch", branch.clone())],
          ),
          commands::BranchArchiveScope::Remote => tf("Delete remote branch '{branch}'? Any local branch with the same name will be kept.", &[("branch", branch.clone())]),
          commands::BranchArchiveScope::LocalAndRemote => tf(
            "Archive branch '{branch}' locally and remotely? It will no longer appear in active branch lists.",
            &[("branch", branch.clone())],
          ),
        })
        .child(t("Unmerged revisions may later be removed by garbage collection."))
        .on_ok(move |_, _, cx| {
          view
            .update(cx, |this, cx| {
              let branch_exists = match scope {
                commands::BranchArchiveScope::Local => this.local_branches.contains(&branch),
                commands::BranchArchiveScope::Remote => this.remote_branches.contains(&branch),
                commands::BranchArchiveScope::LocalAndRemote => this.local_branches.contains(&branch),
              };
              let archives_local = matches!(scope, commands::BranchArchiveScope::Local | commands::BranchArchiveScope::LocalAndRemote);
              if this.busy || !this.connected || this.root != root || this.status.branch != current || !branch_exists || archives_local && branch == this.status.branch {
                return false;
              }
              this.command(commands::archive_branch_args(branch.clone(), scope), title, false, true, cx);
              true
            })
            .unwrap_or(false)
        })
    });
  }

  pub(super) fn new_branch_dialog(&mut self, remote: bool, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy || !self.connected {
      return;
    }
    let name = cx.new(|cx| TextInput::new("Branch name", cx));
    let view = cx.entity().downgrade();
    let root = self.root.clone();
    let title = if remote { "New remote branch" } else { "New local branch" };
    window.open_dialog(cx, move |dialog, _, _| {
      let input = name.clone();
      let view = view.clone();
      let root = root.clone();
      dialog
        .title(t(title))
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("new-branch-confirm", t("Create"), true))
        .child(t("Branch name"))
        .child(name.clone())
        .child(if remote {
          "Create and switch to the new branch, then push it to the remote."
        } else {
          "Create and switch to the new local branch."
        })
        .on_ok(move |_, _, cx| {
          let name = input.read(cx).content.trim().to_string();
          if name.is_empty() {
            return false;
          }
          view
            .update(cx, |this, cx| {
              if this.busy || !this.connected || this.root != root {
                return false;
              }
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
                } else {
                  Ok(created)
                }
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
                  this.log(this.output.clone());
                  this.show_branches = true;
                  this.refresh(cx);
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

  pub(super) fn move_dialog(&mut self, source: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy {
      return;
    }
    let destination = cx.new(|cx| TextInput::new("Full destination path including file or folder name", cx));
    destination.update(cx, |input, _| input.content = source.to_string_lossy().into_owned().into());
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
      dialog
        .title(t("Move"))
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("move-confirm", t("Move"), true))
        .child(
          div()
            .flex()
            .flex_col()
            .gap_2()
            .child(t("Source"))
            .child(source.display().to_string())
            .child(t("Destination"))
            .child(destination.clone())
            .child(t("Include the new file or folder name. Parent folder must exist."))
            .child(t(&error)),
        )
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
          view
            .update(cx, |this, cx| {
              if this.busy || this.root != root {
                return false;
              }
              this.busy = true;
              this.notice = "Moving…".into();
              let task = cx.background_executor().spawn(async move { backend::move_entry(&root, &source, &destination) });
              cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                  this.busy = false;
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
              })
              .detach();
              cx.notify();
              true
            })
            .unwrap_or(true)
        })
    });
  }

  pub(super) fn history_input(id: &'static str, input: Entity<TextInput>, history: Vec<String>) -> impl IntoElement {
    let target = input.clone();
    div()
      .flex()
      .gap_2()
      .items_center()
      .child(div().flex_1().min_w_0().child(input))
      .child(Button::new(id).label("▾").dropdown_menu(move |mut menu, _, _| {
        if history.is_empty() {
          return menu.item(PopupMenuItem::new(t("No history")).disabled(true));
        }
        for value in &history {
          let value = value.clone();
          let target = target.clone();
          menu = menu.item(PopupMenuItem::new(value.clone()).on_click(move |_, _, cx| {
            target.update(cx, |input, cx| {
              input.reset();
              input.content = value.clone().into();
              cx.notify();
            });
          }));
        }
        menu
      }))
  }

  pub(super) fn clone_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy {
      return;
    }
    let remote = self.settings.clone_remote().unwrap_or_else(|| self.settings.clone_url.clone());
    self.settings.remember_clone();
    let mut urls = self.settings.clone_urls.clone();
    for url in std::iter::once(&remote).chain(self.settings.login_urls.iter()) {
      if !url.trim().is_empty() && !urls.contains(url) {
        urls.push(url.clone());
      }
    }
    let url = cx.new(|cx| {
      let mut input = TextInput::new("lores://server:port/repository", cx);
      input.content = remote.into();
      input
    });
    let destinations = self.settings.clone_destinations.clone();
    let destination = cx.new(|cx| TextInput::new("Absolute destination path", cx));
    destination.update(cx, |input, _| input.content = self.settings.clone_destination.clone().into());
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
        .close_button(false)
        .overlay_closable(false)
        .footer(dialog_footer("clone-confirm", t("Clone"), true))
        .child(
          div()
            .flex()
            .flex_col()
            .gap_2()
            .child(t("Server URL"))
            .child(Self::history_input("clone-server-history", url.clone(), urls.clone()))
            .child(t("Destination Path"))
            .child(Self::history_input("clone-path-history", destination.clone(), destinations.clone()))
            .child(validation_text),
        )
        .on_close(move |_, _, cx| {
          let _ = close_view.update(cx, |this, cx| {
            this.settings.clone_destination = close_destination.read(cx).content.to_string();
            this.settings.remember_clone();
            this.save_settings();
          });
        })
        .on_ok(move |_, window, cx| {
          let remote = url_input.read(cx).content.trim().to_owned();
          let valid_url = remote
            .split_once("://")
            .is_some_and(|(_, rest)| rest.split_once('/').is_some_and(|(host, repository)| !host.is_empty() && !repository.trim_matches('/').is_empty()))
            && !remote.chars().any(char::is_whitespace);
          if !valid_url {
            *validation.borrow_mut() = t("Enter a server URL including the repository name.");
            window.refresh();
            return false;
          }
          let path = PathBuf::from(destination_input.read(cx).content.trim());
          if !path.is_absolute() {
            *validation.borrow_mut() = t("Enter an absolute destination path.");
            window.refresh();
            return false;
          }
          view
            .update(cx, |this, cx| {
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
              let task = cx.background_executor().spawn(async move { backend::clone_repository(&cli, &remote, &target, identity.as_deref()) });
              cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                  this.busy = false;
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

fn lore_command_label(args: &[String]) -> String {
  if args.first().is_some_and(|arg| arg == "stage") && args.len() > 1 {
    let paths = args[1..].iter().map(|arg| format!("{arg:?}")).collect::<Vec<_>>().join("`\n");
    return format!("lore stage `\n{paths}");
  }
  let args = args
    .iter()
    .map(|arg| if arg.chars().any(char::is_whitespace) { format!("{arg:?}") } else { arg.clone() })
    .collect::<Vec<_>>()
    .join(" ");
  format!("lore {args}")
}

// Pass explicit files to the CLI so a non-recursive choice cannot expand a folder.
fn folder_change_paths(changes: &[Change], targets: &[(String, bool)], action: &str, recursive: bool) -> Vec<String> {
  changes
    .iter()
    .filter(|change| {
      (match action {
        "stage" => !change.staged,
        "unstage" => change.staged,
        _ => true,
      }) && targets.iter().any(|(target, directory)| {
        let path = std::path::Path::new(&change.path);
        let target = std::path::Path::new(target);
        if *directory {
          path != target && if recursive { path.starts_with(target) } else { path.parent() == Some(target) }
        } else {
          path == target
        }
      })
    })
    .map(|change| change.path.clone())
    .collect()
}

#[cfg(test)]
mod folder_scope_tests {
  use super::{Change, folder_change_paths};

  #[test]
  fn folder_scope_respects_depth_stage_state_and_path_boundaries() {
    let changes: Vec<_> = [
      ("folder/direct", false),
      ("folder/staged", true),
      ("folder/sub/deleted", false),
      ("folder/sub/deep/staged", true),
      ("folder-other/file", false),
      ("separate", false),
    ]
    .into_iter()
    .map(|(path, staged)| Change {
      path: path.into(),
      staged,
      ..Default::default()
    })
    .collect();
    let targets = vec![("folder".into(), true)];
    assert_eq!(folder_change_paths(&changes, &targets, "stage", false), ["folder/direct"]);
    assert_eq!(folder_change_paths(&changes, &targets, "unstage", false), ["folder/staged"]);
    assert_eq!(folder_change_paths(&changes, &targets, "stage", true), ["folder/direct", "folder/sub/deleted"]);
    assert_eq!(folder_change_paths(&changes, &targets, "unstage", true), ["folder/staged", "folder/sub/deep/staged"]);
    assert_eq!(folder_change_paths(&changes, &targets, "reset", false).len(), 2);
    assert_eq!(folder_change_paths(&changes, &targets, "reset", true).len(), 4);
    let targets = vec![("folder".into(), true), ("folder/sub".into(), true), ("separate".into(), false)];
    assert_eq!(folder_change_paths(&changes, &targets, "reset", true).len(), 5);
    assert_eq!(folder_change_paths(&changes, &[("missing".into(), true)], "stage", true).len(), 0);
  }
}
