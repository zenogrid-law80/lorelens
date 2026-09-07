use super::*;

impl Render for Lens {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rgb = palette(cx);
        let ready = !self.busy;
        let vcs = ready && self.connected;
        let staged = self.status.changes.iter().filter(|c| c.staged).count();

        let sidebar = self.render_file_browser(cx);
        let mut tabs = div()
            .flex()
            .gap_2()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(rgb(BORDER));
        for (id, label, tab) in [
            ("pending-tab", "Pending changes", Tab::Pending),
            ("history-tab", "Submitted revisions", Tab::History),
            ("preview-tab", "File preview", Tab::Files),
        ] {
            tabs = tabs.child(
                div()
                    .id(id)
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(rgb(if self.tab == tab { Selected } else { PANEL }))
                    .child(t(label))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.busy {
                            return;
                        }
                        this.tab = tab;
                        if tab == Tab::History && this.connected {
                            this.command(
                                vec!["history".into(), "50".into(), "--oneline".into()],
                                "Submitted revisions",
                                false,
                                false,
                                cx,
                            );
                        } else if tab == Tab::Files
                            && let Some(path) = this.selection.current.clone()
                        {
                            this.select(path, cx);
                        }
                        cx.notify();
                    })),
            );
        }
        let mut pending = div()
            .id("pending-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .bg(rgb(PANEL));
        for (i, change) in self.status.changes.iter().enumerate().take(2000) {
            pending = pending.child(self.change_row(i, change, cx));
        }
        if self.status.changes.is_empty() {
            pending = pending.child(div().p_6().text_color(rgb(MUTED)).child(t(
                if self.connected {
                    "No pending changes. Refresh to scan for edits."
                } else {
                    "Open a Lore repository and Refresh to view pending changes."
                },
            )));
        }
        let details = if self.show_log {
            self.logs.join("\n\n")
        } else {
            if self.output_title == "Welcome to LoreLens" || self.output_title == "Move completed" {
                t(&self.output)
            } else {
                self.output.clone()
            }
        };
        let lines = details
            .lines()
            .take(1500)
            .enumerate()
            .map(|(i, line)| {
                div()
                    .flex()
                    .gap_3()
                    .min_h(px(20.))
                    .child(
                        div()
                            .w(px(35.))
                            .flex_shrink_0()
                            .text_right()
                            .text_color(rgb(MUTED))
                            .child(format!("{}", i + 1)),
                    )
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
            .when(self.tab == Tab::Pending, |d| d.h(px(270.)).flex_shrink_0())
            .when(self.tab != Tab::Pending, |d| d.flex_1())
            .border_t_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_2()
                    .bg(rgb(PANEL))
                    .child(
                        div()
                            .id("details-tab")
                            .cursor_pointer()
                            .text_color(rgb(if self.show_log { MUTED } else { BLUE }))
                            .child(t("Details"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_log = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("log-tab")
                            .cursor_pointer()
                            .text_color(rgb(if self.show_log { BLUE } else { MUTED }))
                            .child(t("Command log"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_log = true;
                                cx.notify();
                            })),
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
                    .child(self.button("copy", "Copy", true).on_click(cx.listener(
                        |this, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(if this.show_log {
                                this.logs.join("\n\n")
                            } else {
                                this.output.clone()
                            }));
                        },
                    ))),
            )
            .child(
                div()
                    .id("detail-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .font_family("Consolas")
                    .text_size(px(12.))
                    .p_3()
                    .children(lines),
            );
        let right = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .child(tabs)
            .when(self.tab == Tab::Pending, |d| {
                d.child(
                    div()
                        .flex()
                        .px_4()
                        .py_2()
                        .gap_3()
                        .bg(rgb(Hover))
                        .text_size(px(11.))
                        .text_color(rgb(MUTED))
                        .child(div().w(px(18.)))
                        .child(div().flex_1().child(t("FILE / PATH")))
                        .child(div().w(px(90.)).child(t("ACTION")))
                        .child(div().w(px(125.)).child(t("STATE")))
                        .child(div().w(px(75.)).text_right().child(t("SIZE"))),
                )
                .child(pending)
                .child(
                    div()
                        .px_4()
                        .py_3()
                        .flex()
                        .items_center()
                        .gap_3()
                        .border_t_1()
                        .border_color(rgb(BORDER))
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(rgb(MUTED))
                                .child(tf("{count} staged", &[("count", staged.to_string())])),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(BORDER))
                                .child(self.message.clone()),
                        )
                        .child(
                            self.button("commit", "Commit staged", vcs && staged > 0)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.commit_staged(cx);
                                })),
                        ),
                )
            })
            .child(detail_panel);
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .font_family("Segoe UI")
            .text_size(px(13.))
            .child(
                TitleBar::new()
                    .bg(rgb(PANEL))
                    .border_color(rgb(BORDER))
                    .child(
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
                                    .text_size(px(13.))
                                    .px_2()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("LoreLens"),
                            )
                            .child(self.app_menu("Repository", false, cx))
                            .child(self.app_menu("Changes", false, cx))
                            .child(self.tool_menu(cx))
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
                    .child(
                        self.button("refresh", "Refresh", ready)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    )
                    .child(self.button("sync", "Sync", vcs).on_click(cx.listener(
                        |this, _, _, cx| {
                            if !this.busy && this.connected {
                                this.command(vec!["sync".into()], "Sync", false, true, cx);
                            }
                        },
                    )))
                    .child(self.button("push", "Push", vcs).on_click(cx.listener(
                        |this, _, _, cx| {
                            if !this.busy && this.connected {
                                this.command(vec!["push".into()], "Push", false, true, cx);
                            }
                        },
                    )))
                    .child(div().text_size(px(11.)).child(match &self.pending_push {
                        Ok(items) => format!("↑ {}", items.len()),
                        Err(_) => "↑ ?".into(),
                    }))
                    .child(div().flex_1())
                    .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(
                        if self.connected {
                            tf(
                                "Revision {revision}",
                                &[("revision", self.status.revision.to_string())],
                            )
                        } else {
                            t("Lore not connected")
                        },
                    )),
            )
            .child(div().flex().flex_1().min_h_0().child(sidebar).child(right))
            .child(
                div()
                    .h(px(30.))
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .text_size(px(11.))
                    .text_color(rgb(if self.error { Danger } else { MUTED }))
                    .child(if self.busy {
                        "◌"
                    } else if self.error {
                        "!"
                    } else {
                        "●"
                    })
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(t(&self.notice)),
                    )
                    .child("Rust + GPUI · LoreLens"),
            )
            .children(Root::render_dialog_layer(window, cx))
    }
}
