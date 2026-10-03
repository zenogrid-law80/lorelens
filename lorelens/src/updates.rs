use super::*;
use backend::updates::{self, Release};
use gpui_component::checkbox::Checkbox;
use gpui_component::scroll::{Scrollbar, ScrollbarMode};
use std::collections::HashSet;

pub(super) const RELEASES_URL: &str = "https://github.com/zenogrid-law80/lorelens/releases";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const HISTORY_PAGE_SIZE: usize = 5;
const MAX_NOTE_CHARACTERS: usize = 12_000;
const MAX_NOTE_LINES: usize = 120;

pub(super) struct UpdatesView {
  parent: WeakEntity<Lens>,
  check_for_updates: bool,
  checking: bool,
  checked: bool,
  downloading: bool,
  installer_launched: bool,
  installer_completed: bool,
  releases: Vec<Release>,
  expanded: HashSet<String>,
  visible_releases: usize,
  check_error: Option<String>,
  install_error: Option<String>,
  preference_error: Option<String>,
  scroll: ScrollHandle,
}

impl UpdatesView {
  pub(super) fn new(parent: WeakEntity<Lens>, check_for_updates: bool, cx: &mut Context<Self>) -> Self {
    let mut view = Self {
      parent,
      check_for_updates,
      checking: false,
      checked: false,
      downloading: false,
      installer_launched: false,
      installer_completed: false,
      releases: Vec::new(),
      expanded: HashSet::new(),
      visible_releases: HISTORY_PAGE_SIZE,
      check_error: None,
      install_error: None,
      preference_error: None,
      scroll: ScrollHandle::default(),
    };
    if check_for_updates {
      view.check(cx);
    }
    view
  }

  pub(super) fn available_version(&self) -> Option<&str> {
    self.latest_update().map(|release| release.version.as_str())
  }

  fn latest_update(&self) -> Option<&Release> {
    self.releases.first().filter(|release| updates::is_newer(&release.version, CURRENT_VERSION))
  }

  pub(super) fn check(&mut self, cx: &mut Context<Self>) {
    if self.checking || self.downloading {
      return;
    }
    self.checking = true;
    self.check_error = None;
    let task = cx.background_executor().spawn(async move { updates::releases() });
    cx.spawn(async move |this, cx| {
      let result = task.await;
      let _ = this.update(cx, |this, cx| {
        this.checking = false;
        this.checked = true;
        match result {
          Ok(releases) => {
            this.expanded.retain(|version| releases.iter().any(|release| &release.version == version));
            this.releases = releases;
          }
          Err(error) => this.check_error = Some(error),
        }
        let _ = this.parent.update(cx, |_, cx| cx.notify());
        cx.notify();
      });
    })
    .detach();
    cx.notify();
  }

  fn set_automatic_checks(&mut self, enabled: bool, cx: &mut Context<Self>) {
    let saved = self
      .parent
      .update(cx, |parent, cx| {
        let previous = parent.settings.check_for_updates;
        parent.settings.check_for_updates = enabled;
        let saved = parent.save_settings();
        if !saved {
          parent.settings.check_for_updates = previous;
        }
        cx.notify();
        saved
      })
      .unwrap_or(false);
    if saved {
      self.check_for_updates = enabled;
      self.preference_error = None;
    } else {
      self.preference_error = Some(t("Could not save update preferences."));
    }
    cx.notify();
  }

  fn install(&mut self, cx: &mut Context<Self>) {
    if self.checking || self.downloading || self.installer_launched || self.installer_completed {
      return;
    }
    let Some(release) = self.latest_update().filter(|release| release.installer.is_some()).cloned() else {
      return;
    };
    self.downloading = true;
    self.installer_completed = false;
    self.install_error = None;
    let task = cx.background_executor().spawn(async move {
      let installer = updates::download_installer(&release)?;
      updates::launch_installer(&installer)
    });
    cx.spawn(async move |this, cx| {
      let result = task.await;
      let _ = this.update(cx, |this, cx| {
        this.downloading = false;
        match result {
          Ok(completion) => {
            this.installer_launched = true;
            let monitor = cx
              .background_executor()
              .spawn(async move { completion.recv().unwrap_or_else(|_| Err(t("Could not monitor the update installer. Try again."))) });
            cx.spawn(async move |this, cx| {
              let result = monitor.await;
              let _ = this.update(cx, |this, cx| {
                this.installer_launched = false;
                match result {
                  Ok(()) => this.installer_completed = true,
                  Err(error) => this.install_error = Some(error),
                }
                cx.notify();
              });
            })
            .detach();
          }
          Err(error) => this.install_error = Some(error),
        }
        cx.notify();
      });
    })
    .detach();
    cx.notify();
  }

  fn history(&self, cx: &mut Context<Self>) -> Div {
    let colors = palette(cx);
    let mut history = div().flex().flex_col().gap_2();
    for (index, release) in self.releases.iter().take(self.visible_releases).enumerate() {
      let expanded = self.expanded.contains(&release.version);
      let toggle = cx.entity().downgrade();
      let version = release.version.clone();
      let website = release.url.clone();
      let date = chrono::DateTime::parse_from_rfc3339(&release.published_at)
        .map(|date| date.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|_| release.published_at.chars().take(10).collect());
      let row = div()
        .id(("update-release-row", index))
        .flex()
        .items_center()
        .gap_3()
        .p_3()
        .cursor_pointer()
        .hover(|style| style.bg(colors(Hover)))
        .on_click(move |_, _, cx| {
          let _ = toggle.update(cx, |this, cx| {
            if !this.expanded.remove(&version) {
              this.expanded.insert(version.clone());
            }
            cx.notify();
          });
        })
        .child(div().font_weight(FontWeight::SEMIBOLD).child(format!("v{}", release.version)))
        .when(index == 0 && updates::is_newer(&release.version, CURRENT_VERSION), |row| {
          row.child(div().rounded_sm().px_2().text_xs().bg(colors(Selected)).text_color(colors(Accent)).child(t("New")))
        })
        .when(release.version == CURRENT_VERSION, |row| row.child(div().text_xs().text_color(colors(MUTED)).child(t("Installed"))))
        .child(div().flex_1().min_w_0().text_sm().text_color(colors(MUTED)).child(tf("Published {date}", &[("date", date)])))
        .child(Icon::new(if expanded { IconName::ChevronDown } else { IconName::ChevronRight }).size(px(14.)));
      let mut card = div()
        .flex()
        .flex_col()
        .w_full()
        .min_w_0()
        .border_1()
        .border_color(colors(BORDER))
        .rounded_md()
        .overflow_hidden()
        .child(row);
      if expanded {
        let mut notes = div().flex().flex_col().min_w_0().w_full().p_3().pt_0().gap_1().text_sm();
        if release.notes.trim().is_empty() {
          notes = notes.child(div().text_color(colors(MUTED)).child(t("No release notes.")));
        } else {
          let bounded: String = release.notes.chars().take(MAX_NOTE_CHARACTERS).collect();
          for line in bounded.lines().take(MAX_NOTE_LINES) {
            notes = notes.child(div().w_full().min_h(px(18.)).child(line.to_owned()));
          }
          if release.notes.chars().count() > MAX_NOTE_CHARACTERS || bounded.lines().count() > MAX_NOTE_LINES {
            notes = notes.child(div().text_color(colors(MUTED)).child(t("Additional release notes are available on the release page.")));
          }
        }
        notes = notes.child(
          Button::new(("update-release-website", index))
            .border_0()
            .ghost()
            .small()
            .icon(IconName::ExternalLink)
            .label(t("View on website"))
            .on_click(move |_, _, cx| {
              cx.open_url(&website);
            }),
        );
        card = card.child(notes);
      }
      history = history.child(card);
    }
    if self.visible_releases < self.releases.len() {
      let more = cx.entity().downgrade();
      history = history.child(Button::new("update-history-more").border_0().small().label(t("Load more")).w_full().on_click(move |_, _, cx| {
        let _ = more.update(cx, |this, cx| {
          this.visible_releases = (this.visible_releases + HISTORY_PAGE_SIZE).min(this.releases.len());
          cx.notify();
        });
      }));
    }
    if self.releases.is_empty() {
      let text = if self.checking {
        t("Checking for updates…")
      } else if self.checked && self.check_error.is_none() {
        t("No releases found. Try again or open the official releases page.")
      } else {
        t("Release history is unavailable. Check for updates to load it.")
      };
      history = history.child(div().p_3().text_sm().text_color(colors(MUTED)).child(text));
    }
    history
  }
}

impl Render for UpdatesView {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let colors = palette(cx);
    let available = self.latest_update();
    let status = if self.checking {
      t("Checking for updates…")
    } else if available.is_some() {
      t("Update available")
    } else if self.checked && self.check_error.is_none() && !self.releases.is_empty() {
      t("You’re up to date.")
    } else {
      t("Check for updates to see the latest release.")
    };
    let check = cx.entity().downgrade();
    let automatic = cx.entity().downgrade();
    let install = cx.entity().downgrade();
    let mut application = div()
      .flex()
      .flex_col()
      .gap_3()
      .p_4()
      .border_1()
      .border_color(colors(BORDER))
      .rounded_lg()
      .child(
        div()
          .flex()
          .items_center()
          .justify_between()
          .gap_3()
          .child(
            div().flex().flex_col().gap_1().child(div().font_weight(FontWeight::SEMIBOLD).child(t("LoreLens"))).child(
              div()
                .text_sm()
                .text_color(colors(MUTED))
                .child(tf("Current version: {version}", &[("version", CURRENT_VERSION.into())])),
            ),
          )
          .child(
            Button::new("update-check")
              .border_0()
              .small()
              .icon(IconName::RefreshCw)
              .label(if self.checking {
                t("Checking for updates…")
              } else if self.check_error.is_some() {
                t("Retry")
              } else {
                t("Check for updates")
              })
              .loading(self.checking)
              .disabled(self.checking || self.downloading)
              .on_click(move |_, _, cx| {
                let _ = check.update(cx, |this, cx| this.check(cx));
              }),
          ),
      )
      .child(div().text_sm().text_color(if available.is_some() { colors(Accent) } else { colors(MUTED) }).child(status));
    if let Some(release) = available {
      application = application.child(div().text_sm().child(tf("Latest version: {version}", &[("version", release.version.clone())])));
      if release.installer.is_some() {
        application = application.child(
          Button::new("update-install")
            .border_0()
            .primary()
            .small()
            .label(if self.downloading { t("Downloading and verifying update…") } else { t("Download and install") })
            .loading(self.downloading)
            .disabled(self.checking || self.downloading || self.installer_launched || self.installer_completed)
            .on_click(move |_, _, cx| {
              let _ = install.update(cx, |this, cx| this.install(cx));
            }),
        );
      } else {
        let website = release.url.clone();
        application = application.child(div().text_sm().text_color(colors(MUTED)).child(if cfg!(windows) {
          t("Windows installer is unavailable for this release. Download the update from the release page.")
        } else {
          t("Automatic installation is unavailable on this platform. Download the update from the release page.")
        }));
        application = application.child(
          Button::new("update-manual-download")
            .border_0()
            .small()
            .icon(IconName::ExternalLink)
            .label(t("View on website"))
            .on_click(move |_, _, cx| {
              cx.open_url(&website);
            }),
        );
      }
    }
    if self.downloading {
      application = application.child(
        gpui_component::progress::Progress::new("update-download-progress")
          .loading(true)
          .accessibility_label(t("Downloading and verifying update…")),
      );
    }
    if self.installer_launched {
      application = application.child(div().text_sm().text_color(colors(Success)).child(t("Installer opened. Complete setup, then restart LoreLens.")));
    }
    if self.installer_completed {
      application = application.child(div().text_sm().text_color(colors(Success)).child(t("Installation completed. Restart LoreLens to use the new version.")));
    }
    for (heading, error) in [
      ("Update check failed. Check your connection and try again.", self.check_error.as_ref()),
      ("Update installation could not start. Try again.", self.install_error.as_ref()),
    ] {
      if let Some(error) = error {
        application = application.child(div().flex().flex_col().gap_1().text_sm().text_color(colors(Danger)).child(t(heading)).child(error.clone()));
      }
    }
    application = application
      .child(
        div()
          .border_t_1()
          .border_color(colors(BORDER))
          .pt_3()
          .child(
            Checkbox::new("update-auto-check")
              .checked(self.check_for_updates)
              .label(t("Check for updates on startup"))
              .on_click(move |enabled, _, cx| {
                let _ = automatic.update(cx, |this, cx| this.set_automatic_checks(*enabled, cx));
              }),
          ),
      )
      .child(
        div()
          .text_sm()
          .text_color(colors(MUTED))
          .child(t("LoreLens checks for new versions when it starts. Installation always requires your approval.")),
      );
    if let Some(error) = &self.preference_error {
      application = application.child(div().text_sm().text_color(colors(Danger)).child(error.clone()));
    }
    let source = div()
      .p_4()
      .flex()
      .flex_col()
      .gap_1()
      .border_1()
      .border_color(colors(BORDER))
      .rounded_lg()
      .child(div().font_weight(FontWeight::SEMIBOLD).child(t("Update download source")))
      .child(div().text_sm().child(t("Official GitHub Releases")))
      .child(div().text_sm().text_color(colors(MUTED)).child(t("Updates are downloaded from the official LoreLens GitHub releases.")));
    let history = self.history(cx);
    // Constrain only the viewport: a max-height on the scrolled content hides
    // overflowing cards from the scroll handle instead of making them scroll.
    let height = (window.viewport_size().height - px(200.)).max(px(180.)).min(px(520.));
    let content = div().w_full().h_auto().flex_shrink_0().flex().flex_col().gap_4().pr(px(12.)).child(application).child(source).child(
      div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .border_1()
        .border_color(colors(BORDER))
        .rounded_lg()
        .child(
          div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(t("Release history")))
            .child(
              Button::new("update-website")
                .border_0()
                .ghost()
                .small()
                .icon(IconName::ExternalLink)
                .label(t("View on website"))
                .on_click(|_, _, cx| {
                  cx.open_url(RELEASES_URL);
                }),
            ),
        )
        .child(history),
    );
    div()
      .relative()
      .w_full()
      .h(height)
      .min_w_0()
      .flex_shrink_0()
      .overflow_hidden()
      .child(div().id("updates-scroll").size_full().flex().flex_col().overflow_y_scroll().track_scroll(&self.scroll).child(content))
      .child(Scrollbar::vertical(&self.scroll).mode(ScrollbarMode::Always))
  }
}

impl Lens {
  pub(super) fn updates_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let updates = self.updates.clone();
    window.open_dialog(cx, move |dialog, _, _| {
      dialog.title(t("Updates")).w(px(780.)).footer(dialog_footer("updates-close", t("Close"), false)).child(updates.clone())
    });
  }
}
