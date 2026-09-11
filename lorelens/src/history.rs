use super::*;

#[derive(Default)]
pub(super) struct FileHistoryState {
  path: Option<String>,
  revisions: Vec<backend::FileRevision>,
  selection: SelectionState,
  scroll: UniformListScrollHandle,
  limit: usize,
  message: String,
}

impl FileHistoryState {
  fn selected_pair(&self) -> Option<(backend::FileRevision, backend::FileRevision)> {
    if self.selection.paths.len() != 2 {
      return None;
    }
    let selected: Vec<_> = self.revisions.iter().filter(|revision| self.selection.paths.contains(&revision.hash)).collect();
    if selected.len() != 2 {
      return None;
    }
    Some((selected[1].clone(), selected[0].clone()))
  }

  fn click(&mut self, hash: String, additive: bool, range: bool) {
    let visible = self.revisions.iter().map(|revision| revision.hash.clone()).collect::<Vec<_>>();
    self.selection.click(hash, &visible, additive, range);
  }

  pub(super) fn matches_pair(&self, path: &str, older: &str, newer: &str) -> bool {
    self.path.as_deref() == Some(path) && self.selected_pair().is_some_and(|(left, right)| left.hash == older && right.hash == newer)
  }
}

impl Lens {
  pub(super) fn open_file_history(&mut self, path: String, limit: usize, cx: &mut Context<Self>) {
    if self.busy {
      return;
    }
    self.file_history = FileHistoryState::default();
    self.tab = Tab::History;
    if !self.connected || self.root.join(&path).is_dir() {
      self.file_history.message = t("Select a repository file to view its history.");
      cx.notify();
      return;
    }
    self.selection.select(path.clone());
    self.preview.invalidate();
    self.file_history.path = Some(path.clone());
    self.file_history.limit = limit;
    self.file_history.message = t("Loading file history…");
    self.busy = true;
    let root = self.root.clone();
    let expected_root = root.clone();
    let cli = self.cli.clone();
    let identity = self.settings.identity.clone();
    self.log(format!("lore file history -- {path:?} {limit}"));
    let task = cx.background_executor().spawn(async move { backend::file_history(&cli, &root, &path, limit, identity.as_deref()) });
    cx.spawn(async move |this, cx| {
      let result = task.await;
      let _ = this.update(cx, |this, cx| {
        this.busy = false;
        if this.root != expected_root {
          return;
        }
        match result {
          Ok(revisions) => {
            this.file_history.message = if revisions.is_empty() { t("No file history found.") } else { String::new() };
            this.file_history.revisions = revisions;
            this.error = false;
          }
          Err(error) => {
            this.log(error.clone());
            this.file_history.message = error;
            this.error = true;
          }
        }
        this.notice = "File History".into();
        cx.notify();
      });
    })
    .detach();
    cx.notify();
  }

  pub(super) fn diff_selected_history(&mut self, cx: &mut Context<Self>) {
    if self.busy || !self.connected {
      return;
    }
    let Some((older, newer)) = self.file_history.selected_pair() else { return };
    let Some(path) = self.file_history.path.clone() else { return };
    let tool = self.settings.external_tool.clone();
    let custom_arguments = (tool == "custom").then(|| self.settings.custom_tool_arguments.clone());
    let configured = if tool == "custom" {
      Some(&self.settings.custom_tool_path)
    } else {
      self.settings.tool_paths.get(&tool)
    };
    let display_name = if tool == "custom" { self.settings.custom_tool_name.clone() } else { tool.clone() };
    let Some(executable) = external_tools::resolve(&tool, configured) else {
      self.choose_tool(
        tool,
        Some(DiffRetry::History {
          root: self.root.clone(),
          path,
          older: older.hash,
          newer: newer.hash,
        }),
        cx,
      );
      return;
    };
    self.busy = true;
    self.notice = tf(
      "Opening {tool} diff for {path}…",
      &[("tool", display_name), ("path", format!("{path} (r{} → r{})", older.number, newer.number))],
    );
    self.log(format!("Diff Selected: {path:?} r{} ({}) -> r{} ({})", older.number, older.hash, newer.number, newer.hash));
    let root = self.root.clone();
    let expected_root = root.clone();
    let cli = self.cli.clone();
    let identity = self.settings.identity.clone();
    let task = cx.background_executor().spawn(async move {
      external_tools::diff_revisions(
        &cli,
        &root,
        &path,
        &older,
        &newer,
        identity.as_deref(),
        external_tools::Tool {
          name: &tool,
          custom_arguments: custom_arguments.as_deref(),
          executable: &executable,
        },
      )
    });
    cx.spawn(async move |this, cx| {
      let result = task.await;
      let _ = this.update(cx, |this, cx| {
        this.busy = false;
        if this.root != expected_root {
          return;
        }
        match result {
          Ok(()) => {
            this.output_title = "Diff Selected".into();
            this.notice = "External diff opened".into();
            this.error = false;
          }
          Err(error) => {
            this.log(error.clone());
            this.notice = error;
            this.error = true;
          }
        }
        cx.notify();
      });
    })
    .detach();
    cx.notify();
  }

  fn history_row(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
    let rgb = palette(cx);
    let revision = &self.file_history.revisions[index];
    let hash = revision.hash.clone();
    let check_hash = hash.clone();
    let selected = self.file_history.selection.paths.contains(&hash);
    let view = cx.entity().downgrade();
    let root = self.root.clone();
    let path = self.file_history.path.clone();
    div()
      .id(("file-revision", index))
      .h(px(34.))
      .flex()
      .items_center()
      .gap_3()
      .px_3()
      .pr(px(18.))
      .bg(rgb(if selected { Hover } else { PANEL }))
      .cursor_pointer()
      .child(
        gpui_component::checkbox::Checkbox::new(("revision-check", index))
          .checked(selected)
          .disabled(self.busy)
          .on_click(cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            if !this.busy {
              this.file_history.click(check_hash.clone(), true, false);
              cx.notify();
            }
          })),
      )
      .child(div().w(px(65.)).flex_shrink_0().child(format!("r{}", revision.number)))
      .child(div().w(px(90.)).flex_shrink_0().text_color(rgb(MUTED)).child(revision.hash[..10].to_string()))
      .child(div().w(px(65.)).flex_shrink_0().child(revision.action.clone()))
      .child(div().flex_1().min_w_0().text_ellipsis().overflow_hidden().child(revision.message.clone()))
      .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
        if !this.busy {
          let modifiers = event.modifiers();
          this.file_history.click(hash.clone(), modifiers.control || modifiers.platform, modifiers.shift);
          cx.notify();
        }
      }))
      .context_menu(move |menu, _, cx| {
        // Right-click deliberately preserves the two-row selection.
        let enabled = view.upgrade().is_some_and(|entity| {
          let this = entity.read(cx);
          !this.busy && this.connected && this.root == root && this.file_history.path == path && this.file_history.selected_pair().is_some()
        });
        let view = view.clone();
        let root = root.clone();
        let path = path.clone();
        menu.item(PopupMenuItem::new(t("Diff Selected")).disabled(!enabled).on_click(move |_, _, cx| {
          let _ = view.update(cx, |this, cx| {
            if this.root == root && this.file_history.path == path {
              this.diff_selected_history(cx);
            }
          });
        }))
      })
  }

  pub(super) fn render_file_history(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let rgb = palette(cx);
    let path = self.file_history.path.clone();
    let limit = self.file_history.limit;
    div()
      .flex_1()
      .min_h_0()
      .flex()
      .flex_col()
      .child(
        div()
          .flex()
          .items_center()
          .gap_2()
          .px_3()
          .py_2()
          .flex_shrink_0()
          .child(
            div()
              .flex_1()
              .min_w_0()
              .overflow_hidden()
              .text_ellipsis()
              .child(format!("{} — {}", t("File History"), path.clone().unwrap_or_default())),
          )
          .when(path.is_some(), |div| {
            div.child(self.button("refresh-file-history", "Refresh", !self.busy).on_click(cx.listener(move |this, _, _, cx| {
              if let Some(path) = this.file_history.path.clone() {
                this.open_file_history(path, limit, cx);
              }
            })))
          })
          .when(limit > 0 && self.file_history.revisions.len() >= limit, |div| {
            div.child(self.button("more-file-history", "Load more", !self.busy).on_click(cx.listener(move |this, _, _, cx| {
              if let Some(path) = this.file_history.path.clone() {
                this.open_file_history(path, limit.saturating_mul(2), cx);
              }
            })))
          }),
      )
      .child(
        div()
          .px_3()
          .pb_2()
          .text_size(px(12.))
          .text_color(rgb(MUTED))
          .child(t("Select two revisions with Ctrl/Shift or checkboxes, then right-click and choose Diff Selected.")),
      )
      .when(self.file_history.revisions.is_empty(), |d| {
        d.child(div().p_3().child(if self.file_history.message.is_empty() {
          t("Select a repository file to view its history.")
        } else {
          self.file_history.message.clone()
        }))
      })
      .child(
        div()
          .relative()
          .flex_1()
          .min_h_0()
          .overflow_hidden()
          .child(
            uniform_list(
              "file-history-rows",
              self.file_history.revisions.len(),
              cx.processor(|this, range: std::ops::Range<usize>, _, cx| range.map(|index| this.history_row(index, cx)).collect::<Vec<_>>()),
            )
            .size_full()
            .track_scroll(&self.file_history.scroll),
          )
          .child(gpui_component::scroll::Scrollbar::vertical(&self.file_history.scroll).mode(gpui_component::scroll::ScrollbarMode::Always)),
      )
  }
}

#[cfg(test)]
mod tests {
  use super::FileHistoryState;
  use crate::backend;
  #[test]
  fn selection_requires_exactly_two_revisions_and_orders_old_to_new() {
    let mut state = FileHistoryState {
      revisions: (1..=3)
        .rev()
        .map(|number| backend::FileRevision {
          hash: number.to_string(),
          number,
          action: String::new(),
          message: String::new(),
        })
        .collect(),
      ..Default::default()
    };
    assert!(state.selected_pair().is_none());
    state.click("3".into(), false, false);
    assert!(state.selected_pair().is_none());
    state.click("1".into(), true, false);
    let (older, newer) = state.selected_pair().unwrap();
    assert_eq!((older.number, newer.number), (1, 3));
    state.click("2".into(), true, false);
    assert!(state.selected_pair().is_none());
    state.click("3".into(), false, false);
    state.click("2".into(), false, true);
    assert!(state.selected_pair().is_some());
  }
}
