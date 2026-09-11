use super::*;

impl Render for Lens {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let rgb = palette(cx);
    let ready = !self.busy;
    let vcs = ready && self.connected;
    let sync_count = |commits: &Result<Vec<backend::LocalCommit>, String>| {
      if self.connected {
        commits.as_ref().map(|items| items.len().to_string()).unwrap_or_else(|_| "?".into())
      } else {
        "?".into()
      }
    };
    let sync_label = format!("{}  {}", t("Sync"), sync_count(&self.pending_pull));
    let push_label = format!("{}  {}", t("Push"), sync_count(&self.pending_push));
    let deduplicate = vcs && !backend::duplicate_change_paths(&self.status.changes).is_empty();
    let staged = self.status.changes.iter().filter(|c| c.staged).count();
    let has_logs = !self.logs.is_empty();

    let sidebar_visible = !self.root.as_os_str().is_empty();
    let sidebar = div().size_full().pr_1().flex().when(sidebar_visible, |d| d.child(self.render_file_browser(cx)));
    let mut tabs = div().flex().flex_shrink_0().gap_2().px_3().border_b_1().border_color(rgb(BORDER));
    for (id, label, tab) in [
      ("pending-tab", "Changes", Tab::Pending),
      ("history-tab", "History", Tab::History),
      ("preview-tab", "File preview", Tab::Files),
      ("unpushed-tab", "Unpushed local commits", Tab::Unpushed),
    ] {
      tabs = tabs.child(
        div()
          .id(id)
          .relative()
          .px_3()
          .py_3()
          .cursor_pointer()
          .text_color(rgb(if self.tab == tab { Accent } else { TEXT }))
          .when(self.tab == tab, |tab| tab.font_weight(FontWeight::SEMIBOLD))
          .hover(move |style| style.bg(rgb(Hover)))
          .child(t(label))
          .when(self.tab == tab, |tab| tab.child(div().absolute().bottom_0().left_0().w_full().h(px(3.)).rounded_t_md().bg(rgb(Accent))))
          .on_click(cx.listener(move |this, _, _, cx| {
            if this.busy {
              return;
            }
            this.tab = tab;
            if tab == Tab::Unpushed && this.connected {
              this.refresh(cx);
            } else if tab == Tab::History && this.connected {
              this.command(vec!["history".into(), "50".into(), "--oneline".into()], "Submitted revisions", false, false, cx);
            } else if tab == Tab::Files
              && let Some(path) = this.preview.path.clone().or_else(|| this.selection.current.clone())
            {
              this.select(path, cx);
            }
            cx.notify();
          })),
      );
    }
    let pending_query = self.pending_filter.read(cx).content.to_lowercase().to_string();
    self.pending_visible = self
      .status
      .changes
      .iter()
      .enumerate()
      .filter(|(_, change)| pending_query.is_empty() || change.path.to_lowercase().contains(&pending_query) || change.action.to_lowercase().contains(&pending_query))
      .map(|(index, _)| index)
      .collect();
    let visible_paths: Vec<_> = self.pending_visible.iter().map(|index| self.status.changes[*index].path.clone()).collect();
    let all_visible_selected = !visible_paths.is_empty() && visible_paths.iter().all(|path| self.selection.paths.contains(path));
    let visible_count = visible_paths.len();
    let mut pending = div()
      .id("pending-list")
      .track_focus(&self.pending_focus)
      .on_mouse_down(
        MouseButton::Left,
        cx.listener(|this, _, window, cx| {
          window.focus(&this.pending_focus, cx);
        }),
      )
      .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
        let modifiers = event.keystroke.modifiers;
        if event.keystroke.key == "a" && (modifiers.control || modifiers.platform) && !modifiers.alt && !modifiers.shift {
          cx.stop_propagation();
          if this.busy {
            return;
          }
          let visible: Vec<_> = this.pending_visible.iter().map(|index| this.status.changes[*index].path.clone()).collect();
          this.selection.select_all(&visible);
          this.preview.invalidate();
          this.notice = tf("{count} items selected", &[("count", this.selection.paths.len().to_string())]);
          cx.notify();
        }
      }))
      .flex_1()
      .min_h_0()
      .flex()
      .flex_col()
      .overflow_hidden()
      .bg(rgb(PANEL));
    if !self.pending_visible.is_empty() {
      pending = pending.child(
        uniform_list(
          "pending-rows",
          self.pending_visible.len(),
          cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
            range
              .map(|i| {
                let change_index = this.pending_visible[i];
                this.change_row(change_index, &this.status.changes[change_index], cx)
              })
              .collect::<Vec<_>>()
          }),
        )
        .flex_1()
        .min_h_0()
        .pr(px(12.))
        .track_scroll(&self.pending_scroll),
      );
    }
    if self.pending_visible.is_empty() {
      let (title, description) = if !self.connected {
        ("Open a repository", "Open a Lore repository and Refresh to view pending changes.")
      } else if self.busy {
        ("Working…", "Scanning for changes…")
      } else if self.error {
        ("Changes unavailable", "Refresh to scan for changes. See the command log for details.")
      } else if !self.status.changes.is_empty() {
        ("No matching changes.", "Try a different filter to find your changes.")
      } else {
        ("Working tree clean", "Changes will appear here when Lore detects modified, added, or deleted files.")
      };
      pending = pending.child(
        div()
          .id("pending-empty-state")
          .flex_1()
          .min_h_0()
          .overflow_y_scroll()
          .flex()
          .flex_col()
          .items_center()
          .justify_center()
          .gap_2()
          .p_4()
          .text_center()
          .child(Icon::new(IconName::FileText).size(px(36.)).text_color(rgb(MUTED)))
          .child(div().mt_1().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).child(t(title)))
          .when(title == "Working tree clean", |state| state.child(div().text_size(px(14.)).child(t("No pending changes"))))
          .child(div().max_w(px(360.)).text_size(px(13.)).text_color(rgb(MUTED)).child(t(description)))
          .child(
            div()
              .mt_2()
              .child(self.button("empty-refresh", "Refresh", ready).on_click(cx.listener(|this, _, _, cx| this.refresh(cx)))),
          ),
      );
    }
    let lines = self
      .logs
      .join("\n\n")
      .lines()
      .take(1500)
      .enumerate()
      .map(|(i, line)| {
        div()
          .flex()
          .gap_3()
          .min_h(px(20.))
          .child(div().w(px(35.)).flex_shrink_0().text_right().text_color(rgb(MUTED)).child(format!("{}", i + 1)))
          .child(
            div()
              .text_color(rgb(if line.starts_with('+') {
                Success
              } else if line.starts_with('-') {
                Danger
              } else {
                TEXT
              }))
              .child(line.to_string()),
          )
      })
      .collect::<Vec<_>>();
    let detail_panel = div()
      .flex()
      .flex_col()
      .min_h_0()
      .size_full()
      .rounded_lg()
      .overflow_hidden()
      .bg(rgb(BG))
      .border_1()
      .border_color(rgb(BORDER))
      .child(
        div()
          .flex()
          .items_center()
          .gap_3()
          .px_4()
          .h(px(44.))
          .flex_shrink_0()
          .border_b_1()
          .border_color(rgb(BORDER))
          .bg(rgb(PANEL))
          .child(
            div()
              .h_full()
              .flex()
              .items_center()
              .px_1()
              .border_b_2()
              .border_color(rgb(Accent))
              .font_weight(FontWeight::SEMIBOLD)
              .text_color(rgb(Accent))
              .child(t("Command log")),
          )
          .child(
            div()
              .flex_1()
              .min_w_0()
              .overflow_hidden()
              .text_ellipsis()
              .text_size(px(11.))
              .text_color(rgb(MUTED))
              .child(t(&self.output_title)),
          )
          .child(self.button("copy", "Copy", true).on_click(cx.listener(|this, _, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(this.logs.join("\n\n")));
          })))
          .child(self.button("clear-command-log", "Clear", has_logs).on_click(cx.listener(|this, _, _, cx| {
            this.logs.clear();
            cx.notify();
          }))),
      )
      .child(
        div()
          .id("detail-scroll")
          .flex_1()
          .min_h_0()
          .overflow_scroll()
          .font_family(gpui_component::Theme::global(cx).mono_font_family.clone())
          .text_size(px(12.))
          .p_3()
          .children(lines),
      );
    let upper_panel = div()
      .size_full()
      .min_w_0()
      .min_h_0()
      .flex()
      .flex_col()
      .rounded_lg()
      .border_1()
      .border_color(rgb(BORDER))
      .bg(rgb(PANEL))
      .overflow_hidden()
      .child(tabs)
      .when(self.tab == Tab::Files, |d| {
        d.child(div().px_3().py_2().bg(rgb(PANEL)).child(self.preview.path.clone().unwrap_or_else(|| t("File preview")))).child(
          div()
            .id("file-preview-content")
            .flex_1()
            .min_h_0()
            .overflow_scroll()
            .font_family(gpui_component::Theme::global(cx).mono_font_family.clone())
            .text_size(px(12.))
            .p_3()
            .children(self.preview.content.lines().map(|line| div().min_h(px(20.)).child(line.to_string())).collect::<Vec<_>>()),
        )
      })
      .when(self.tab == Tab::History, |d| {
        d.child(div().px_3().py_2().bg(rgb(PANEL)).child(t("History"))).child(
          div()
            .id("history-content")
            .flex_1()
            .min_h_0()
            .overflow_scroll()
            .font_family(gpui_component::Theme::global(cx).mono_font_family.clone())
            .text_size(px(12.))
            .p_3()
            .children(self.output.lines().map(|line| div().min_h(px(20.)).child(line.to_string())).collect::<Vec<_>>()),
        )
      })
      .when(self.tab == Tab::Pending, |d| {
        d.child(
          div()
            .px_3()
            .pt_3()
            .pb_2()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .bg(rgb(PANEL))
            .child(
              div()
                .flex()
                .items_center()
                .pl_3()
                .bg(rgb(BG))
                .overflow_hidden()
                .border_1()
                .border_color(rgb(BORDER))
                .rounded_md()
                .child(Icon::new(IconName::Search).size(px(16.)).text_color(rgb(MUTED)))
                .child(div().flex_1().min_w_0().child(self.pending_filter.clone())),
            )
            .child(
              div().flex().items_center().gap_2().text_size(px(12.)).child(
                gpui_component::checkbox::Checkbox::new("select-visible-changes")
                  .label(tf("{count} changed files", &[("count", visible_count.to_string())]))
                  .checked(all_visible_selected)
                  .disabled(self.busy || visible_paths.is_empty())
                  .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                    if this.busy {
                      return;
                    }
                    if *checked {
                      this.selection.select_all(&visible_paths);
                    } else {
                      for path in &visible_paths {
                        this.selection.paths.remove(path);
                      }
                      this.selection.current = this.selection.paths.iter().next().cloned();
                      this.selection.anchor = this.selection.current.clone();
                    }
                    this.preview.invalidate();
                    cx.notify();
                  })),
              ),
            ),
        )
        .child(
          div()
            .relative()
            .mx_2()
            .mb_2()
            .border_1()
            .border_color(rgb(BORDER))
            .rounded_lg()
            .overflow_hidden()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
              div()
                .flex()
                .items_center()
                .flex_shrink_0()
                .h(px(32.))
                .pl_3()
                .pr(px(24.))
                .gap_2()
                .bg(rgb(Header))
                .text_size(px(11.))
                .text_color(rgb(MUTED))
                .child(div().w(px(16.)).flex_shrink_0())
                .child(div().w(px(16.)).flex_shrink_0())
                .child(div().flex_1().min_w_0().child(t("FILE / PATH")))
                .child(div().w(px(90.)).flex_shrink_0().child(t("ACTION")))
                .child(div().w(px(110.)).flex_shrink_0().child(t("STATE"))),
            )
            .child(pending)
            .when(visible_count > 0, |list| {
              list.child(gpui_component::scroll::Scrollbar::vertical(&self.pending_scroll).mode(gpui_component::scroll::ScrollbarMode::Always))
            }),
        )
        .child(
          div()
            .mx_2()
            .mb_2()
            .p_3()
            .flex()
            .flex_wrap()
            .flex_shrink_0()
            .items_center()
            .gap_3()
            .rounded_lg()
            .bg(rgb(Surface))
            .border_1()
            .border_color(rgb(BORDER))
            .child(
              div().flex().items_center().gap_3().child(Icon::new(IconName::GitBranch).size(px(20.)).text_color(rgb(MUTED))).child(
                div()
                  .flex()
                  .flex_col()
                  .gap_1()
                  .child(tf("{count} files staged", &[("count", staged.to_string())]))
                  .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(t("Stage files to commit your changes."))),
              ),
            )
            .child(
              div()
                .flex_1()
                .min_w(px(160.))
                .overflow_hidden()
                .rounded_md()
                .border_1()
                .border_color(rgb(BORDER))
                .child(self.message.clone()),
            )
            .child(self.button("generate-message", "Generate message", vcs && staged > 0).on_click(cx.listener(|this, _, _, cx| {
              this.generate_commit_message(cx);
            })))
            .child(self.button("commit", "Commit staged", vcs && staged > 0).on_click(cx.listener(|this, _, _, cx| {
              this.commit_staged(cx);
            }))),
        )
      })
      .when(self.tab == Tab::Unpushed, |d| {
        let items = self.pending_push.clone().unwrap_or_default();
        let can_discard = vcs && !items.is_empty() && self.status.changes.is_empty();
        let message = if !self.connected {
          t("Open a Lore repository and Refresh to view unpushed commits.")
        } else {
          match &self.pending_push {
            Err(error) => t(error),
            Ok(items) if items.is_empty() => t("No unpushed local commits."),
            Ok(items) => tf("{count} unpushed local commits", &[("count", items.len().to_string())]),
          }
        };
        d.child(
          div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(PANEL))
            .child(
              div().p_3().flex().items_center().gap_2().child(div().flex_1().child(message)).child(
                self
                  .button("discard-unpushed", "Discard all unpushed commits", can_discard)
                  .on_click(cx.listener(|this, _, window, cx| this.local_commits_dialog(window, cx))),
              ),
            )
            .when(!self.status.changes.is_empty(), |d| {
              d.child(div().px_3().pb_2().child(t("Commits cannot be discarded while working changes exist.")))
            })
            .child(
              uniform_list("unpushed-commit-rows", items.len(), move |range, _, _| {
                range.map(|i| div().h(px(30.)).px_3().overflow_hidden().text_ellipsis().child(items[i].label())).collect::<Vec<_>>()
              })
              .flex_1()
              .min_h_0(),
            ),
        )
      });
    let right = if self.show_log && self.tab != Tab::Unpushed {
      v_resizable("content-command-log-split")
        .child(resizable_panel().size_range(px(320.)..Pixels::MAX).child(upper_panel))
        .child(resizable_panel().size(px(270.)).size_range(px(140.)..px(600.)).child(div().size_full().pt_1().child(detail_panel)))
        .into_any_element()
    } else {
      upper_panel.into_any_element()
    };
    div()
      .capture_key_down(cx.listener(Self::handle_shortcut))
      .size_full()
      .relative()
      .flex()
      .flex_col()
      .bg(rgb(BG))
      .text_color(rgb(TEXT))
      .font_family(gpui_component::Theme::global(cx).font_family.clone())
      .text_size(px(13.))
      .child(
        TitleBar::new().bg(rgb(Surface)).border_color(rgb(BORDER)).child(
          div()
            .h_full()
            .flex_1()
            .min_w_0()
            .pr_2()
            .flex()
            .items_center()
            .gap_2()
            .child(
              div()
                .id("lorelens-home-link")
                .text_size(px(18.))
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, |event, _, cx| {
                  cx.stop_propagation();
                  if event.click_count == 2 {
                    cx.open_url("https://lorelens.zenogrid.co.kr/");
                  }
                })
                .child(Icon::default().data(include_bytes!("../assets/lorelens-mark.svg")).size(px(28.)).text_color(rgb(Accent)))
                .child("LoreLens"),
            )
            .child(self.app_menu("Repository", false, cx))
            .child(self.app_menu("Changes", false, cx))
            .child(self.options_menu(cx))
            .child(self.app_menu("View", false, cx))
            .child(div().flex_1())
            .child(self.app_menu("Account", false, cx)),
        ),
      )
      .child(
        div()
          .flex()
          .gap_2()
          .items_center()
          .px_4()
          .py_3()
          .border_b_1()
          .border_color(rgb(BORDER))
          .child(self.app_menu("Repository", true, cx))
          .child(self.branch_menu(vcs, cx))
          .child(self.button("refresh", "Refresh", ready).h(px(36.)).on_click(cx.listener(|this, _, _, cx| this.refresh(cx))))
          .child(self.button("sync", "Sync", vcs).h(px(36.)).label(sync_label).on_click(cx.listener(|this, _, _, cx| {
            if !this.busy && this.connected {
              this.command(vec!["sync".into()], "Sync", false, true, cx);
            }
          })))
          .child(
            self
              .button("push", "Push", vcs)
              .h(px(36.))
              .text_color(rgb(Accent))
              .label(push_label)
              .on_click(cx.listener(|this, _, _, cx| {
                if !this.busy && this.connected {
                  this.command(vec!["push".into()], "Push", false, true, cx);
                }
              })),
          )
          .child(
            self
              .button("deduplicate-files", "Deduplicate Files", deduplicate)
              .h(px(36.))
              .on_click(cx.listener(|this, _, window, cx| this.deduplicate_files_dialog(window, cx))),
          )
          .child(div().flex_1())
          .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(if self.connected {
            tf("Revision {revision}", &[("revision", self.status.revision.to_string())])
          } else {
            t("Lore not connected")
          })),
      )
      .child(
        div()
          .h(px(38.))
          .flex_shrink_0()
          .flex()
          .items_center()
          .gap_2()
          .px_4()
          .border_b_1()
          .border_color(rgb(BORDER))
          .text_size(px(11.))
          .text_color(rgb(MUTED))
          .child(t("Path"))
          .child(
            div()
              .flex_1()
              .min_w_0()
              .overflow_hidden()
              .rounded_md()
              .border_1()
              .border_color(rgb(BORDER))
              .child(self.selected_path.clone()),
          )
          .child(div().flex().items_center().child(self.bookmark_toggle(cx)).child(self.bookmark_list(cx))),
      )
      .child(
        div().p_1().flex().flex_1().min_h_0().child(
          h_resizable("file-content-split")
            .child(resizable_panel().visible(sidebar_visible).size(px(320.)).size_range(px(220.)..px(600.)).child(sidebar))
            .child(resizable_panel().size_range(px(500.)..Pixels::MAX).child(right)),
        ),
      )
      .child(
        div()
          .h(px(36.))
          .flex_shrink_0()
          .px_4()
          .flex()
          .items_center()
          .gap_3()
          .border_t_1()
          .border_color(rgb(BORDER))
          .bg(rgb(Surface))
          .text_size(px(11.))
          .text_color(rgb(if self.error { Danger } else { MUTED }))
          .child(div().size(px(10.)).flex_shrink_0().rounded_full().bg(rgb(if self.busy && !self.silent_refresh {
            Warning
          } else if self.error {
            Danger
          } else {
            Success
          })))
          .child(div().flex_1().overflow_hidden().text_ellipsis().child(t(&self.notice)))
          .child(concat!("Rust + GPUI · LoreLens v", env!("CARGO_PKG_VERSION"))),
      )
      .children(Root::render_dialog_layer(window, cx))
      .when(self.busy && !self.silent_refresh, |view| {
        view.child(
          div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x00000080))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
              div()
                .w(px(420.))
                .max_w_full()
                .p_6()
                .rounded_lg()
                .bg(rgb(PANEL))
                .border_1()
                .border_color(rgb(BORDER))
                .flex()
                .flex_col()
                .gap_4()
                .child(t("Working…"))
                .child(div().text_size(px(12.)).overflow_hidden().child(t(&self.notice)))
                .child(div().relative().h(px(6.)).w_full().overflow_hidden().rounded_full().bg(rgb(BORDER)).child(
                  div().absolute().h_full().w(relative(0.3)).rounded_full().bg(rgb(Accent)).with_animation(
                    "command-progress",
                    Animation::new(std::time::Duration::from_millis(1200)).repeat(),
                    |bar, delta| bar.left(relative(0.7 * (1. - (2. * delta - 1.).abs()))),
                  ),
                )),
            ),
        )
      })
  }
}
