use super::*;

impl Lens {
  pub(super) fn bookmark_toggle(&self, cx: &mut Context<Self>) -> Button {
    let view = cx.entity().downgrade();
    let path = self.selection.current.as_ref().map(|path| self.root.join(path));
    let root = self.root.clone();
    let bookmarked = path.as_ref().is_some_and(|path| self.settings.is_bookmarked(&root, path));
    Button::new("bookmark-toggle")
      .label(if bookmarked { "★" } else { "☆" })
      .small()
      .disabled(self.busy || path.is_none())
      .on_click(move |_, _, cx| {
        let Some(path) = path.clone() else {
          return;
        };
        let _ = view.update(cx, |this, cx| {
          let added = this.settings.toggle_bookmark(&root, &path);
          if this.save_settings() {
            this.error = false;
            this.notice = tf(if added { "Bookmark added: {path}" } else { "Bookmark removed: {path}" }, &[("path", path.display().to_string())]);
          }
          cx.notify();
        });
      })
  }

  pub(super) fn bookmark_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
    let view = cx.entity().downgrade();
    Button::new("bookmark-list").label("▾").small().dropdown_menu(move |mut menu, _, cx| {
      let (bookmarks, root, ready) = view
        .update(cx, |this, cx| {
          if this.settings.prune_bookmarks() {
            this.save_settings();
            cx.notify();
          }
          (this.settings.bookmarks_for(&this.root), this.root.clone(), !this.busy)
        })
        .unwrap_or_default();
      for path in &bookmarks {
        let bookmark_view = view.clone();
        let target = root.join(path);
        let label = path.display().to_string();
        menu = menu.item(PopupMenuItem::new(label).disabled(!ready).on_click(move |_, _, cx| {
          let _ = bookmark_view.update(cx, |this, cx| this.open_bookmark(target.clone(), cx));
        }));
      }
      if bookmarks.is_empty() {
        menu = menu.label(t("No bookmarks"));
      }
      menu
    })
  }

  pub(super) fn app_menu(&self, kind: &'static str, toolbar: bool, cx: &mut Context<Self>) -> impl IntoElement {
    let view = cx.entity().downgrade();
    let ready = !self.busy;
    let file = ready && self.connected && self.selection.current.is_some();
    let commit = ready && self.connected && self.status.changes.iter().any(|c| c.staged);
    let theme = self.settings.theme.clone();
    let show_log = self.show_log;
    let account = self.logged_in_account.clone();
    let label = if toolbar {
      format!("{} ▾", self.root.file_name().unwrap_or_default().to_string_lossy())
    } else if kind == "Account" {
      format!("{}: {} ▾", t("Account"), t(&self.logged_in_account))
    } else {
      format!("{} ▾", t(kind))
    };
    let button = Button::new(if toolbar { "repository-selector" } else { kind })
      .label(label)
      .when(toolbar, |button| button.icon(IconName::Database).h(px(36.)).min_w(px(140.)))
      .when(!toolbar && kind != "Account", |button| button.ghost());
    button.dropdown_menu(move |mut menu, window, cx| {
      let items: Vec<(&str, &str, bool)> = match kind {
        "Repository" => vec![
          ("Open repository…", "open", ready),
          ("Clone repository…", "clone", ready),
          ("Create repository…", "create", ready),
          ("Sparse workspace…", "sparse", ready),
        ],
        "Changes" => vec![
          ("Stage", "stage", file),
          ("Unstage", "unstage", file),
          ("Commit staged", "commit", commit),
          ("File history", "history", file),
          ("Pending push", "pending", ready),
        ],
        "View" => vec![("Theme…", "theme", true), ("Command log", "log", true)],
        _ => vec![("Login…", "login", ready), ("Logout", "logout", ready)],
      };
      for (label, action, enabled) in items {
        let view = view.clone();
        let label = t(label);
        let checked = kind == "View" && (theme == action || (action == "log" && show_log));
        menu = menu.item(
          PopupMenuItem::new(if checked { format!("✓ {label}") } else { label })
            .disabled(!enabled)
            .on_click(move |_, window, cx| {
              let _ = view.update(cx, |this, cx| {
                match action {
                  "open" => this.choose(false, cx),
                  "clone" => this.clone_dialog(window, cx),
                  "create" => this.create_repository_dialog(window, cx),
                  "sparse" => this.sparse_workspace_dialog(window, cx),
                  "stage" | "unstage" | "history" => this.file_command(action, window, cx),
                  "commit" => this.commit_staged(cx),
                  "login" => this.login_dialog(window, cx),
                  "logout" => this.logout_dialog(window, cx),
                  "theme" => this.theme_dialog(window, cx),
                  "System" | "Light" | "Dark" => {
                    this.settings.theme = action.into();
                    defer_theme(action, window, cx);
                    this.save_settings();
                  }
                  "log" => {
                    this.show_log = !this.show_log;
                    this.settings.show_command_log = this.show_log;
                    this.save_settings();
                  }
                  "pending" => {
                    this.output_title = "Pending push commits".into();
                    this.output = match &this.pending_push {
                      Ok(items) if items.is_empty() => t("No commits pending push."),
                      Ok(items) => tf(
                        "{count} commits pending push\n\n{commits}",
                        &[
                          ("count", items.len().to_string()),
                          ("commits", items.iter().map(backend::LocalCommit::label).collect::<Vec<_>>().join("\n")),
                        ],
                      ),
                      Err(error) => error.clone(),
                    };
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
        menu = menu.submenu(t("Language"), window, cx, move |mut menu, _, cx| {
          let current = language_view.upgrade().map(|entity| entity.read(cx).settings.language.clone()).unwrap_or_default();
          for (code, name) in i18n::LOCALES {
            let view = language_view.clone();
            menu = menu.item(PopupMenuItem::new(if current == code { format!("✓ {name}") } else { name.into() }).on_click(move |_, window, cx| {
              let _ = view.update(cx, |this, cx| {
                this.settings.language = code.into();
                i18n::set_locale(code);
                window.set_window_title(&t("LoreLens — Desktop repository client"));
                this.save_settings();
                this.filter.update(cx, |_, cx| cx.notify());
                this.message.update(cx, |_, cx| cx.notify());
                window.refresh();
                cx.notify();
              });
            }));
          }
          menu
        });
      }
      if kind == "Repository" {
        let (bookmarks, root) = view
          .update(cx, |this, cx| {
            if this.settings.prune_bookmarks() {
              this.save_settings();
              cx.notify();
            }
            (this.settings.bookmarks_for(&this.root), this.root.clone())
          })
          .unwrap_or_default();
        let bookmark_view = view.clone();
        menu = menu.separator().submenu(t("Bookmarks"), window, cx, move |mut menu, _, _| {
          for path in &bookmarks {
            let view = bookmark_view.clone();
            let target = root.join(path);
            let label = path.display().to_string();
            menu = menu.item(PopupMenuItem::new(label).disabled(!ready).on_click(move |_, _, cx| {
              let _ = view.update(cx, |this, cx| this.open_bookmark(target.clone(), cx));
            }));
          }
          if bookmarks.is_empty() {
            menu = menu.label(t("No bookmarks"));
          }
          menu
        });
        let recent = view
          .update(cx, |this, cx| {
            if this.settings.prune_recent() {
              this.save_settings();
              cx.notify();
            }
            this.settings.recent.clone()
          })
          .unwrap_or_default();
        menu = menu.separator().label(t("Recent repositories"));
        for (index, path) in recent.iter().enumerate() {
          let view = view.clone();
          let path = path.clone();
          let label = path.display().to_string();
          let remote_url = backend::repository_remote_url(&path);
          let item = PopupMenuItem::element(move |_, _| {
            div().id(("recent-repository", index)).w_full().child(label.clone()).when_some(remote_url.clone(), |row, url| {
              row.tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(url.clone()).build(window, cx))
            })
          });
          menu = menu.item(item.disabled(!ready).on_click(move |_, _, cx| {
            let _ = view.update(cx, |this, cx| {
              if this.busy {
                return;
              }
              if matches!(path.try_exists(), Ok(false)) {
                this.settings.recent.retain(|entry| entry != &path);
                if this.save_settings() {
                  this.error = false;
                  this.notice = tf("Removed missing repository from recent list: {path}", &[("path", path.display().to_string())]);
                }
                cx.notify();
                return;
              }
              this.open_repository(path.clone(), cx)
            });
          }));
        }
        if recent.is_empty() {
          menu = menu.label(t("No recent repositories"));
        }
      }
      if kind == "Account" {
        menu = menu.separator().label(tf("Signed in: {account}", &[("account", t(&account))]));
      }
      menu
    })
  }
}
