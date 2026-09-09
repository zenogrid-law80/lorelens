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
  ("revert", "Revert file…", "Ctrl+Shift+R", true),
  ("commit", "Commit staged", "Ctrl+S", true),
  ("sync", "Sync", "Ctrl+Shift+G", true),
  ("push", "Push", "Ctrl+Shift+P", true),
  ("history", "File history", "Ctrl+H", true),
  ("diff", "Diff", "Ctrl+Shift+D", true),
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
      "stage" | "unstage" | "history" | "diff" | "delete" | "revert" => self.connected && self.selection.current.is_some(),
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
      "search" => window.focus(&self.filter.read(cx).focus_handle(cx)),
      _ if !self.connected => return,
      "stage" | "unstage" | "history" | "diff" => self.file_command(id, window, cx),
      "commit" => self.commit_staged(cx),
      "sync" => self.command(vec!["sync".into()], "Sync", false, true, cx),
      "push" => self.command(vec!["push".into()], "Push", false, true, cx),
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

  pub(super) fn shortcut_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let view = cx.entity().downgrade();
    let rows: Vec<_> = COMMANDS
      .iter()
      .map(|&(id, label, default, supported)| (id, label, binding(&self.settings, id, default), supported))
      .collect();
    let enabled: Vec<_> = COMMANDS.iter().map(|&(id, _, _, _)| self.shortcut_enabled(id)).collect();
    Button::new("shortcuts-menu").label(format!("{} ▾", t("Keyboard shortcuts"))).dropdown_menu(move |mut menu, _, _| {
      let settings_view = view.clone();
      menu = menu
        .item(PopupMenuItem::new(t("Edit keyboard shortcuts…")).on_click(move |_, window, cx| {
          let _ = settings_view.update(cx, |this, cx| this.shortcuts_dialog(window, cx));
        }))
        .separator();
      for (index, (id, label, key, supported)) in rows.iter().enumerate() {
        let view = view.clone();
        let id = *id;
        let label = format!("{}   {}{}", t(label), key, if *supported { String::new() } else { format!(" — {}", t("Not supported in Lore")) });
        menu = menu.item(PopupMenuItem::new(label).disabled(!supported || !enabled[index]).on_click(move |_, window, cx| {
          let _ = view.update(cx, |this, cx| this.run_shortcut(id, window, cx));
        }));
      }
      menu
    })
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
        .confirm()
        .w(px(680.))
        .button_props(DialogButtonProps::default().cancel_text(t("Cancel")).ok_text(t("Save")))
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
