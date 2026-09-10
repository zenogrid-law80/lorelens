use super::*;
use std::collections::BTreeMap;

// Stable identifiers keep user bindings independent of translated labels.
const COMMANDS: &[(&str, &str, &str, bool)] = &[
  ("open", "Open repository…", "Ctrl+O", true),
  ("refresh", "Refresh", "F5", true),
  ("search", "Search files or folders", "Ctrl+F", true),
  ("stage", "Stage", "Ctrl+Shift+A", true),
  ("unstage", "Unstage", "Ctrl+Shift+U", true),
  ("delete", "Delete…", "Ctrl+D", true),
  ("revert", "Revert file…", "Ctrl+R", true),
  ("commit", "Commit staged", "Ctrl+S", true),
  ("sync", "Sync", "Ctrl+Shift+G", true),
  ("push", "Push", "Ctrl+Shift+P", true),
  ("history", "File history", "Ctrl+Shift+H", true),
  ("diff", "Diff", "Ctrl+Shift+D", true),
  ("lock", "Lock", "Ctrl+L", true),
  ("bookmark", "Bookmark", "Ctrl+B", true),
  ("terminal", "Open Command Window Here", "Ctrl+H", true),
  ("reveal", "Show in Explorer", "Ctrl+I", true),
];

fn normalize(value: &str) -> Result<String, String> {
  if value.trim().is_empty() {
    return Ok(String::new());
  }
  let parts: Vec<_> = value.split('+').map(|part| part.trim().to_ascii_lowercase()).collect();
  let (key, modifiers) = parts.split_last().unwrap();
  let mut ctrl = false;
  let mut shift = false;
  let mut alt = false;
  for modifier in modifiers {
    let flag = match modifier.as_str() {
      "ctrl" => &mut ctrl,
      "shift" => &mut shift,
      "alt" => &mut alt,
      _ => return Err(t("Use Ctrl, Shift, Alt and a letter, number or F1–F12.")),
    };
    if *flag {
      return Err(t("Duplicate modifier."));
    }
    *flag = true;
  }
  let function = key.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()).is_some_and(|n| (1..=12).contains(&n));
  let character = key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric();
  if !(function || character && (ctrl || alt)) {
    return Err(t("Use Ctrl, Shift, Alt and a letter, number or F1–F12."));
  }
  // Preserve standard text editing in all application inputs.
  if ctrl && !alt && !shift && ["a", "c", "v", "x", "z", "y"].contains(&key.as_str()) {
    return Err(t("This shortcut is reserved for text editing."));
  }
  Ok(format!(
    "{}{}{}{}",
    if ctrl { "Ctrl+" } else { "" },
    if alt { "Alt+" } else { "" },
    if shift { "Shift+" } else { "" },
    key.to_uppercase()
  ))
}

fn binding(settings: &settings::Settings, id: &str, default: &str) -> String {
  normalize(settings.shortcuts.get(id).map(String::as_str).unwrap_or(default)).unwrap_or_default()
}

pub(super) fn shortcut_label(shortcuts: &BTreeMap<String, String>, label: &str, id: &str) -> String {
  let Some(&(_, _, default, _)) = COMMANDS.iter().find(|&&(command, _, _, _)| command == id) else {
    return t(label);
  };
  let key = normalize(shortcuts.get(id).map(String::as_str).unwrap_or(default)).unwrap_or_default();
  if key.is_empty() { t(label) } else { format!("{}    {key}", t(label)) }
}

impl Lens {
  fn shortcut_enabled(&self, id: &str) -> bool {
    if self.busy {
      return false;
    }
    if id == "open" {
      return true;
    }
    if self.root.as_os_str().is_empty() {
      return false;
    }
    match id {
      "close" | "refresh" | "search" => true,
      "commit" => self.connected && self.status.changes.iter().any(|change| change.staged),
      "sync" | "push" => self.connected,
      "bookmark" | "terminal" | "reveal" => self.selection.current.is_some(),
      "stage" | "unstage" | "history" | "diff" | "delete" | "revert" | "lock" => self.connected && self.selection.current.is_some(),
      _ => false,
    }
  }

  pub(super) fn handle_shortcut(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
    if window.has_active_dialog(cx) || event.is_held {
      return;
    }
    let m = event.keystroke.modifiers;
    if m.platform || m.function {
      return;
    }
    let key = format!(
      "{}{}{}{}",
      if m.control { "Ctrl+" } else { "" },
      if m.alt { "Alt+" } else { "" },
      if m.shift { "Shift+" } else { "" },
      event.keystroke.key.to_uppercase()
    );
    let Some(&(id, _, _, _)) = COMMANDS.iter().find(|&&(id, _, default, supported)| supported && binding(&self.settings, id, default) == key) else {
      return;
    };
    cx.stop_propagation();
    self.run_shortcut(id, window, cx);
  }

  fn run_shortcut(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
    if !self.shortcut_enabled(id) {
      return;
    }
    if id == "open" {
      self.choose(false, cx);
      return;
    }
    if self.root.as_os_str().is_empty() {
      return;
    }
    match id {
      "close" => {
        self.preview.invalidate();
        self.root.clear();
        self.directory.clear();
        self.entries.clear();
        self.expanded_folders.clear();
        self.selection.clear();
        self.status = Status::default();
        self.connected = false;
        self.connect_after_load = false;
        self.refresh_pending = false;
        self.folder_to_select = None;
        self.locked_paths.clear();
        self.local_branches.clear();
        self.remote_branches.clear();
        self.branch_output.clear();
        self.pending_push = Ok(Vec::new());
        self.output.clear();
        self.output_title.clear();
        self.message.update(cx, |input, cx| {
          input.reset();
          cx.notify();
        });
        self.filter.update(cx, |input, cx| {
          input.reset();
          cx.notify();
        });
        self.pending_filter.update(cx, |input, cx| {
          input.reset();
          cx.notify();
        });
        self.error = false;
        self.notice = t("Workspace closed.");
      }
      "refresh" => self.refresh(cx),
      "search" => window.focus(&self.filter.read(cx).focus_handle(cx), cx),
      "terminal" | "reveal" => {
        let Some(relative) = self.selection.current.clone() else {
          return;
        };
        let path = self.root.join(relative);
        if id == "terminal" {
          self.open_command_window(&path);
        } else {
          cx.reveal_path(&path);
        }
      }
      "bookmark" => {
        let Some(relative) = self.selection.current.clone() else {
          return;
        };
        let path = self.root.join(relative);
        let added = self.settings.toggle_bookmark(&self.root, &path);
        if self.save_settings() {
          self.error = false;
          self.notice = tf(
            if added { "Bookmark added: {path}" } else { "Bookmark removed: {path}" },
            &[("path", path.strip_prefix(&self.root).unwrap_or(&path).display().to_string())],
          );
        }
      }
      _ if !self.connected => return,
      "stage" | "unstage" | "history" | "diff" => self.file_command(id, window, cx),
      "commit" => self.commit_staged(cx),
      "sync" => self.command(vec!["sync".into()], "Sync", false, true, cx),
      "push" => self.command(vec!["push".into()], "Push", false, true, cx),
      "lock" => {
        let Some(path) = self.selection.current.clone() else {
          return;
        };
        let locked = self.locked_paths.contains(&path);
        self.command(
          vec!["lock".into(), if locked { "release" } else { "acquire" }.into(), "--".into(), path],
          if locked { "Unlock" } else { "Lock" },
          false,
          true,
          cx,
        );
      }
      "revert" | "delete" => {
        let mut paths: Vec<_> = self.selection.paths.iter().cloned().collect();
        if paths.is_empty() {
          paths.extend(self.selection.current.clone());
        }
        paths.sort();
        if paths.is_empty() {
          return;
        }
        if id == "revert" {
          self.revert_dialog(paths, false, window, cx);
        } else {
          self.delete_files_dialog(paths.iter().map(|path| self.root.join(path)).collect(), window, cx);
        }
      }
      _ => {}
    }
    cx.notify();
  }

  pub(super) fn options_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let view = cx.entity().downgrade();
    let selected = self.settings.external_tool.clone();
    let selected_label = if selected == "custom" { self.settings.custom_tool_name.clone() } else { selected.clone() };
    let custom_selected = selected == "custom";
    let obliterate_enabled = self.obliterate_enabled;
    let line_ending = self.settings.text_line_ending.clone();
    let encoding = self.settings.text_encoding.clone();
    let ready = !self.busy;
    Button::new("options-menu").label(format!("{} ▾", t("Options"))).dropdown_menu(move |menu, window, cx| {
      let selection_view = view.clone();
      let current = selected.clone();
      let menu = menu.submenu(tf("Diff / Merge: {selected}", &[("selected", selected_label.clone())]), window, cx, move |mut menu, _, _| {
        for tool in external_tools::TOOLS {
          let view = selection_view.clone();
          menu = menu.item(PopupMenuItem::new(if tool == current { format!("✓ {tool}") } else { tool.into() }).on_click(move |_, _, cx| {
            let _ = view.update(cx, |this, cx| {
              this.settings.external_tool = tool.into();
              this.save_settings();
              if external_tools::resolve(tool, this.settings.tool_paths.get(tool)).is_none() {
                this.choose_tool(tool.into(), None, cx);
              }
              cx.notify();
            });
          }));
        }
        let custom_view = selection_view.clone();
        menu = menu.separator().item(PopupMenuItem::new(t("Custom tool…")).on_click(move |_, window, cx| {
          let _ = custom_view.update(cx, |this, cx| this.custom_tool_dialog(window, cx));
        }));
        menu
      });
      let cli_view = view.clone();
      let menu = menu.item(PopupMenuItem::new(t("Locate Lore CLI…")).on_click(move |_, _, cx| {
        let _ = cli_view.update(cx, |this, cx| this.choose(true, cx));
      }));
      let line_ending_view = view.clone();
      let current_line_ending = line_ending.clone();
      let menu = menu
        .separator()
        .submenu(tf("Line endings: {selected}", &[("selected", line_ending.clone())]), window, cx, move |mut menu, _, _| {
          for value in ["System", "LF", "CR", "CRLF"] {
            let view = line_ending_view.clone();
            menu = menu.item(PopupMenuItem::new(t(value)).checked(value == current_line_ending).on_click(move |_, _, cx| {
              let _ = view.update(cx, |this, cx| {
                this.settings.text_line_ending = value.into();
                this.save_settings();
                cx.notify();
              });
            }));
          }
          menu
        });
      let encoding_view = view.clone();
      let current_encoding = encoding.clone();
      let menu = menu.submenu(tf("Encoding: {selected}", &[("selected", encoding.clone())]), window, cx, move |mut menu, _, _| {
        for value in ["System", "UTF-8", "UTF-8 no BOM"] {
          let view = encoding_view.clone();
          menu = menu.item(PopupMenuItem::new(t(value)).checked(value == current_encoding).on_click(move |_, _, cx| {
            let _ = view.update(cx, |this, cx| {
              this.settings.text_encoding = value.into();
              this.save_settings();
              cx.notify();
            });
          }));
        }
        menu
      });
      let extensions_view = view.clone();
      let menu = menu.item(PopupMenuItem::new(t("Manage text file extensions…")).on_click(move |_, window, cx| {
        let _ = extensions_view.update(cx, |this, cx| this.text_extensions_dialog(window, cx));
      }));
      let locate_view = view.clone();
      let path_view = view.clone();
      let obliterate_view = view.clone();
      let view = view.clone();
      let menu = menu
        .separator()
        .item(PopupMenuItem::new(t("Locate executable…")).on_click(move |_, _, cx| {
          let _ = locate_view.update(cx, |this, cx| {
            let tool = this.settings.external_tool.clone();
            this.choose_tool(tool, None, cx);
          });
        }))
        .item(PopupMenuItem::new(t("Use PATH")).disabled(custom_selected).on_click(move |_, _, cx| {
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
        .separator()
        .item(PopupMenuItem::new(t("Obliterate")).checked(obliterate_enabled).disabled(!ready).on_click(move |_, _, cx| {
          let _ = obliterate_view.update(cx, |this, cx| {
            if !this.busy {
              this.obliterate_enabled = !this.obliterate_enabled;
              cx.notify();
            }
          });
        }))
        .separator();
      let bookmark_view = view.clone();
      let settings_view = view.clone();
      menu
        .item(PopupMenuItem::new(t("Manage bookmarks…")).on_click(move |_, window, cx| {
          let _ = bookmark_view.update(cx, |this, cx| this.bookmarks_dialog(window, cx));
        }))
        .item(PopupMenuItem::new(t("Keyboard shortcuts")).on_click(move |_, window, cx| {
          let _ = settings_view.update(cx, |this, cx| this.shortcuts_dialog(window, cx));
        }))
    })
  }

  fn text_extensions_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let extensions = cx.new(|cx| TextInput::new("Extensions", cx));
    extensions.update(cx, |input, _| input.content = self.settings.text_extensions.join(", ").into());
    let view = cx.entity().downgrade();
    window.open_dialog(cx, move |dialog, _, _| {
      let extensions_input = extensions.clone();
      let view = view.clone();
      dialog
        .title(t("Text file extensions"))
        .close_button(false)
        .overlay_closable(false)
        .button_props(DialogButtonProps::default().show_cancel(true).cancel_text(t("Cancel")).ok_text(t("Save")))
        .child(t("Extensions"))
        .child(extensions.clone())
        .child(t("Enter extensions separated by commas, without wildcards. Only these files are checked before staging."))
        .on_ok(move |_, _, cx| {
          let values = settings::Settings::normalize_extensions(vec![extensions_input.read(cx).content.to_string()]);
          view
            .update(cx, |this, cx| {
              this.settings.text_extensions = values;
              let saved = this.save_settings();
              cx.notify();
              saved
            })
            .unwrap_or(false)
        })
    });
  }

  fn bookmarks_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let rows: Vec<_> = self
      .settings
      .bookmarks
      .iter()
      .enumerate()
      .map(|(index, bookmark)| {
        let input = cx.new(|cx| {
          let mut input = TextInput::new("Relative path", cx);
          input.content = bookmark.path.to_string_lossy().into_owned().into();
          input
        });
        (index, bookmark.root.clone(), input, std::rc::Rc::new(std::cell::Cell::new(false)))
      })
      .collect();
    let view = cx.entity().downgrade();
    let error = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    window.open_dialog(cx, move |dialog, _, _| {
      let save_rows = rows.clone();
      let save_view = view.clone();
      let validation = error.clone();
      let content = div().when(rows.is_empty(), |content| content.child(t("No bookmarks"))).when(!rows.is_empty(), |content| {
        content.child(
          div()
            .id("bookmark-settings-scroll")
            .max_h(px(390.))
            .overflow_y_scroll()
            .children(rows.iter().filter(|(_, _, _, removed)| !removed.get()).map(|(index, root, input, removed)| {
              let removed = removed.clone();
              div()
                .flex()
                .items_center()
                .gap_3()
                .py_1()
                .child(div().w(px(250.)).overflow_hidden().text_ellipsis().child(root.display().to_string()))
                .child(div().flex_1().child(input.clone()))
                .child(Button::new(("remove-bookmark", *index)).label(t("Remove")).small().on_click(move |_, window, _| {
                  removed.set(true);
                  window.refresh();
                }))
            })),
        )
      });
      dialog
        .title(t("Bookmark management"))
        .close_button(false)
        .overlay_closable(false)
        .w(px(760.))
        .button_props(DialogButtonProps::default().show_cancel(true).cancel_text(t("Cancel")).ok_text(t("Save")))
        .child(t("Edit paths relative to their repository root. Remove a row to delete its bookmark."))
        .child(content)
        .child(error.borrow().clone())
        .on_ok(move |_, window, cx| {
          let mut bookmarks = Vec::new();
          for (_, root, input, removed) in &save_rows {
            if removed.get() {
              continue;
            }
            let path = PathBuf::from(input.read(cx).content.trim());
            if path.as_os_str().is_empty() || path.is_absolute() || !root.join(&path).exists() {
              *validation.borrow_mut() = tf("Enter an existing relative bookmark path: {path}", &[("path", path.display().to_string())]);
              window.refresh();
              return false;
            }
            bookmarks.push(settings::Bookmark { root: root.clone(), path });
          }
          save_view
            .update(cx, |this, cx| {
              let old = this.settings.bookmarks.clone();
              this.settings.replace_bookmarks(bookmarks);
              if !this.save_settings() {
                this.settings.bookmarks = old;
                *validation.borrow_mut() = this.notice.clone();
                window.refresh();
                return false;
              }
              this.error = false;
              this.notice = t("Bookmarks updated.");
              cx.notify();
              true
            })
            .unwrap_or(false)
        })
    });
  }

  fn shortcuts_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let inputs: Vec<_> = COMMANDS
      .iter()
      .map(|&(id, _, default, _)| {
        cx.new(|cx| {
          let mut input = TextInput::new("Unassigned", cx);
          input.content = binding(&self.settings, id, default).into();
          input
        })
      })
      .collect();
    let view = cx.entity().downgrade();
    let error = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    window.open_dialog(cx, move |dialog, _, _| {
      let save_inputs = inputs.clone();
      let reset_inputs = inputs.clone();
      let view = view.clone();
      let validation = error.clone();
      dialog
        .title(t("Keyboard shortcuts"))
        .close_button(false)
        .overlay_closable(false)
        .w(px(680.))
        .button_props(DialogButtonProps::default().show_cancel(true).cancel_text(t("Cancel")).ok_text(t("Save")))
        .child(t("Enter a shortcut such as Ctrl+Shift+G. Leave blank to unassign."))
        .child(t(
          "Shortcuts use Lore's stage, commit, sync, and push workflow. Delete and revert keep their existing confirmation steps.",
        ))
        .child(
          div()
            .id("shortcut-settings-scroll")
            .max_h(px(390.))
            .overflow_y_scroll()
            .children(COMMANDS.iter().enumerate().map(|(i, &(_, label, _, supported))| {
              div()
                .flex()
                .items_center()
                .gap_3()
                .py_1()
                .child(div().w(px(330.)).child(t(label)).when(!supported, |d| d.child(format!(" ({})", t("Not supported in Lore")))))
                .child(div().flex_1().child(inputs[i].clone()))
            })),
        )
        .child(Button::new("reset-shortcuts").label(t("Restore defaults")).on_click(move |_, _, cx| {
          for (input, &(_, _, default, _)) in reset_inputs.iter().zip(COMMANDS) {
            input.update(cx, |input, cx| {
              input.reset();
              input.content = default.into();
              cx.notify();
            });
          }
        }))
        .child(error.borrow().clone())
        .on_ok(move |_, window, cx| {
          let mut shortcuts = BTreeMap::new();
          let mut used = BTreeMap::new();
          for (input, &(id, label, _, _)) in save_inputs.iter().zip(COMMANDS) {
            let key = match normalize(&input.read(cx).content) {
              Ok(key) => key,
              Err(message) => {
                *validation.borrow_mut() = format!("{}: {message}", t(label));
                window.refresh();
                return false;
              }
            };
            if !key.is_empty()
              && let Some(previous) = used.insert(key.clone(), label)
            {
              *validation.borrow_mut() = format!("{}: {key} — {} / {}", t("Shortcut conflict"), t(previous), t(label));
              window.refresh();
              return false;
            }
            shortcuts.insert(id.to_string(), key);
          }
          view
            .update(cx, |this, cx| {
              let old = std::mem::replace(&mut this.settings.shortcuts, shortcuts);
              if !this.save_settings() {
                this.settings.shortcuts = old;
                *validation.borrow_mut() = this.notice.clone();
                window.refresh();
                return false;
              }
              cx.notify();
              true
            })
            .unwrap_or(false)
        })
    });
  }
}

#[cfg(test)]
mod tests {
  use super::{COMMANDS, binding, normalize};
  use crate::settings;
  #[test]
  fn validates_shortcuts_and_preserves_text_editing() {
    assert_eq!(normalize(" shift + ctrl + g ").unwrap(), "Ctrl+Shift+G");
    assert_eq!(normalize("").unwrap(), "");
    for invalid in ["G", "Shift+G", "Ctrl+C", "Ctrl+Ctrl+G", "Ctrl+", "F13", "Ctrl+Space"] {
      assert!(normalize(invalid).is_err(), "{invalid}");
    }
    let mut seen = std::collections::HashSet::new();
    for &(_, _, key, _) in COMMANDS {
      assert!(seen.insert(normalize(key).unwrap()));
    }
  }
  #[test]
  fn overrides_and_unassigned_survive_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut settings = settings::Settings::default();
    assert_eq!(binding(&settings, "open", "Ctrl+O"), "Ctrl+O");
    settings.shortcuts.insert("open".into(), "Alt+O".into());
    settings.shortcuts.insert("commit".into(), String::new());
    settings.save(&path).unwrap();
    let loaded = settings::Settings::load(&path).unwrap();
    assert_eq!(binding(&loaded, "open", "Ctrl+O"), "Alt+O");
    assert_eq!(binding(&loaded, "commit", "Ctrl+S"), "");
  }
}
