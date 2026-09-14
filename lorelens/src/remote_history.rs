use super::*;

const REMOTE_HISTORY_PAGE_SIZE: usize = 100;
const REMOTE_HISTORY_MAX_COMMITS: usize = 10000;

fn remote_history_limit(page: usize) -> usize {
  page.saturating_add(1).saturating_mul(REMOTE_HISTORY_PAGE_SIZE).saturating_add(1).min(REMOTE_HISTORY_MAX_COMMITS)
}

fn remote_history_page_range(page: usize, total: usize) -> std::ops::Range<usize> {
  let start = page.saturating_mul(REMOTE_HISTORY_PAGE_SIZE).min(total);
  start..start.saturating_add(REMOTE_HISTORY_PAGE_SIZE).min(total)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum HistoryTarget {
  Head,
  Local(String),
  Remote(String),
}

pub(super) struct RemoteHistoryState {
  pub visible: bool,
  pub limit: usize,
  page: usize,
  // Browsing another branch never checks it out.
  pub target: HistoryTarget,
  pub result: Result<backend::RemoteHistory, String>,
  filter: Entity<TextInput>,
  branch_filter: Entity<TextInput>,
  rows: Vec<usize>,
  selected: Option<String>,
  files: Option<Result<Vec<backend::RevisionFile>, String>>,
  files_loading: bool,
  selected_file: Option<String>,
  local_expanded: bool,
  remote_expanded: bool,
  collapsed_folders: std::collections::HashSet<String>,
  scroll: UniformListScrollHandle,
}

impl RemoteHistoryState {
  pub fn new(cx: &mut Context<Lens>) -> Self {
    let filter = cx.new(|cx| TextInput::new("Text, hash or author", cx));
    let branch_filter = cx.new(|cx| TextInput::new("Find a branch", cx));
    cx.observe(&filter, |this, _, cx| {
      this.remote_history.scroll.scroll_to_item(0, ScrollStrategy::Top);
      cx.notify();
    })
    .detach();
    cx.observe(&branch_filter, |_, _, cx| cx.notify()).detach();
    Self {
      visible: false,
      limit: remote_history_limit(0),
      page: 0,
      target: HistoryTarget::Head,
      result: Err("Refresh to load history.".into()),
      filter,
      branch_filter,
      rows: Vec::new(),
      selected: None,
      files: None,
      files_loading: false,
      selected_file: None,
      local_expanded: true,
      remote_expanded: true,
      collapsed_folders: Default::default(),
      scroll: UniformListScrollHandle::default(),
    }
  }

  fn selected_commit(&self) -> Option<&backend::RemoteCommit> {
    self.result.as_ref().ok()?.commits.iter().find(|commit| Some(&commit.hash) == self.selected.as_ref())
  }

  fn comparison(&self) -> Option<backend::RevisionComparison> {
    let file = self.files.as_ref()?.as_ref().ok()?.iter().find(|file| Some(&file.path) == self.selected_file.as_ref())?;
    backend::RevisionComparison::modified_file(self.selected_commit()?, file).ok()
  }

  pub(super) fn matches_comparison(&self, comparison: &backend::RevisionComparison) -> bool {
    self.comparison().as_ref() == Some(comparison)
  }
}

fn matches_commit(commit: &backend::RemoteCommit, query: &str) -> bool {
  [commit.message.as_str(), commit.hash.as_str(), commit.author.as_str(), &commit.number.to_string()]
    .iter()
    .any(|value| value.to_lowercase().contains(query))
}

fn commit_date(timestamp: Option<i64>) -> String {
  timestamp
    .and_then(chrono::DateTime::from_timestamp_millis)
    .map(|date| date.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
    .unwrap_or_else(|| "—".into())
}

fn commit_message(commit: &backend::RemoteCommit) -> String {
  let first = commit.message.lines().next().unwrap_or_default();
  if first.is_empty() { t("(No commit message)") } else { first.into() }
}

impl Lens {
  pub(super) fn set_remote_history(&mut self, result: Result<backend::RemoteHistory, String>, cx: &mut Context<Self>) {
    if let Ok(history) = &result {
      let branch_changed = self.remote_history.result.as_ref().is_ok_and(|previous| previous.branch != history.branch);
      let last_page = history.commits.len().saturating_sub(1) / REMOTE_HISTORY_PAGE_SIZE;
      if branch_changed {
        self.remote_history.page = 0;
      } else {
        self.remote_history.page = self.remote_history.page.min(last_page);
      }
      self.remote_history.limit = remote_history_limit(self.remote_history.page);
    }
    let selection = result.as_ref().ok().and_then(|history| {
      let range = remote_history_page_range(self.remote_history.page, history.commits.len());
      history.commits[range.clone()]
        .iter()
        .find(|commit| Some(&commit.hash) == self.remote_history.selected.as_ref())
        .or_else(|| history.commits.get(range.start))
        .map(|commit| commit.hash.clone())
    });
    self.remote_history.result = result;
    if selection != self.remote_history.selected {
      self.remote_history.selected = None;
      self.remote_history.files = None;
      self.remote_history.files_loading = false;
      self.remote_history.selected_file = None;
    }
    if self.remote_history.visible
      && let Some(hash) = selection
    {
      self.select_remote_commit(hash, cx);
    }
  }

  pub(super) fn activate_remote_history(&mut self, cx: &mut Context<Self>) {
    self.remote_history.visible = true;
    let hash = self
      .remote_history
      .selected
      .clone()
      .or_else(|| self.remote_history.result.as_ref().ok()?.commits.first().map(|commit| commit.hash.clone()));
    if self.connected
      && let Some(hash) = hash
    {
      self.select_remote_commit(hash, cx);
    }
    cx.notify();
  }

  fn browse_history_target(&mut self, target: HistoryTarget, cx: &mut Context<Self>) {
    if self.busy || !self.connected || self.remote_history.target == target {
      return;
    }
    self.remote_history.target = target;
    self.remote_history.page = 0;
    self.remote_history.limit = remote_history_limit(0);
    self.remote_history.selected = None;
    self.remote_history.files = None;
    self.remote_history.files_loading = false;
    self.remote_history.selected_file = None;
    self.remote_history.result = Err("Loading history…".into());
    self.remote_history.scroll.scroll_to_item(0, ScrollStrategy::Top);
    self.refresh(cx);
  }

  fn browse_remote_history_page(&mut self, page: usize, cx: &mut Context<Self>) {
    if self.busy || !self.connected || page == self.remote_history.page {
      return;
    }
    self.remote_history.page = page;
    self.remote_history.limit = remote_history_limit(page);
    self.remote_history.selected = None;
    self.remote_history.files = None;
    self.remote_history.files_loading = false;
    self.remote_history.selected_file = None;
    self.remote_history.scroll.scroll_to_item(0, ScrollStrategy::Top);
    self.refresh(cx);
  }

  fn select_remote_commit(&mut self, hash: String, cx: &mut Context<Self>) {
    if !self.remote_history.result.as_ref().is_ok_and(|history| history.commits.iter().any(|commit| commit.hash == hash)) {
      return;
    }
    if self.remote_history.selected.as_ref() == Some(&hash) && (self.remote_history.files_loading || self.remote_history.files.as_ref().is_some_and(Result::is_ok)) {
      return;
    }
    self.remote_history.selected = Some(hash.clone());
    self.remote_history.files = None;
    self.remote_history.files_loading = true;
    self.remote_history.selected_file = None;
    self.remote_history.collapsed_folders.clear();
    let root = self.root.clone();
    let expected_root = root.clone();
    let target = self.remote_history.target.clone();
    let cli = self.cli.clone();
    let identity = self.settings.identity.clone();
    let revision = hash.clone();
    let task = cx.background_executor().spawn(async move { backend::revision_files(&cli, &root, &revision, identity.as_deref()) });
    cx.spawn(async move |this, cx| {
      let result = task.await;
      let _ = this.update(cx, |this, cx| {
        if this.root != expected_root || this.remote_history.target != target || this.remote_history.selected.as_ref() != Some(&hash) {
          return;
        }
        this.remote_history.files = Some(result);
        this.remote_history.files_loading = false;
        cx.notify();
      });
    })
    .detach();
    cx.notify();
  }

  pub(super) fn diff_remote_file(&mut self, cx: &mut Context<Self>) {
    if self.busy || !self.connected {
      return;
    }
    let Some(comparison) = self.remote_history.comparison() else { return };
    let tool = self.settings.external_tool.clone();
    let custom_arguments = (tool == "custom").then(|| self.settings.custom_tool_arguments.clone());
    let configured = if tool == "custom" {
      Some(&self.settings.custom_tool_path)
    } else {
      self.settings.tool_paths.get(&tool)
    };
    let Some(executable) = external_tools::resolve(&tool, configured) else {
      self.choose_tool(tool, Some(DiffRetry::RemoteHistory { root: self.root.clone(), comparison }), cx);
      return;
    };
    let display_name = if tool == "custom" { self.settings.custom_tool_name.clone() } else { tool.clone() };
    self.busy = true;
    self.notice = tf("Opening {tool} diff for {path}…", &[("tool", display_name), ("path", comparison.path.clone())]);
    self.log(format!("Diff: {:?} {} -> {}", comparison.path, comparison.source, comparison.target));
    let root = self.root.clone();
    let expected_root = root.clone();
    let cli = self.cli.clone();
    let identity = self.settings.identity.clone();
    let task = cx.background_executor().spawn(async move {
      external_tools::diff_commit_file(
        &cli,
        &root,
        &comparison,
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

  fn create_remote_patch(&mut self, cx: &mut Context<Self>) {
    if self.busy || !self.connected {
      return;
    }
    let Some(comparison) = self.remote_history.comparison() else { return };
    self.busy = true;
    self.error = false;
    self.notice = "Generating patch…".into();
    self.log(format!("Patch: {:?} {} -> {}", comparison.path, comparison.source, comparison.target));
    let root = self.root.clone();
    let expected_root = root.clone();
    let expected_comparison = comparison.clone();
    let cli = self.cli.clone();
    let identity = self.settings.identity.clone();
    let task = cx.background_executor().spawn(async move { backend::revision_patch(&cli, &root, &comparison, identity.as_deref()) });
    cx.spawn(async move |this, cx| {
      let result = task.await;
      let _ = this.update(cx, |this, cx| {
        this.busy = false;
        if this.root != expected_root || !this.remote_history.matches_comparison(&expected_comparison) {
          cx.notify();
          return;
        }
        match result {
          Ok(patch) => {
            let name = std::path::Path::new(&expected_comparison.path).file_name().unwrap_or_default().to_string_lossy();
            let suggested_name = format!("{name}-{}.patch", expected_comparison.target.chars().take(10).collect::<String>());
            this.save_remote_patch(patch, suggested_name, cx);
          }
          Err(error) => {
            this.log(error.clone());
            this.notice = t(&error);
            this.error = true;
          }
        }
        cx.notify();
      });
    })
    .detach();
    cx.notify();
  }

  fn save_remote_patch(&mut self, patch: String, suggested_name: String, cx: &mut Context<Self>) {
    let prompt = cx.prompt_for_new_path(&self.root, Some(&suggested_name));
    self.busy = true;
    self.notice = "Choose where to save the patch.".into();
    cx.spawn(async move |this, cx| {
      let result = match prompt.await {
        Ok(Ok(Some(path))) => {
          let destination = path.clone();
          cx.background_executor()
            .spawn(async move { backend::save_revision_patch(&destination, &patch).map(|()| Some(path)) })
            .await
        }
        Ok(Ok(None)) => Ok(None),
        _ => Err(t("Could not open the patch save dialog.")),
      };
      let _ = this.update(cx, |this, cx| {
        this.busy = false;
        match result {
          Ok(Some(path)) => {
            this.notice = tf("Patch saved: {path}", &[("path", path.display().to_string())]);
            this.log(this.notice.clone());
            this.error = false;
          }
          Ok(None) => {
            this.notice = "Patch save cancelled".into();
            this.error = false;
          }
          Err(error) => {
            this.log(error.clone());
            this.notice = tf("Could not save patch: {error}", &[("error", error)]);
            this.error = true;
          }
        }
        cx.notify();
      });
    })
    .detach();
  }

  fn remote_branch_tree(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let rgb = palette(cx);
    let query = self.remote_history.branch_filter.read(cx).content.to_lowercase();
    let local_branches = self.local_branches.iter().filter(|branch| branch.to_lowercase().contains(&query)).cloned().collect::<Vec<_>>();
    let remote_branches = self.remote_branches.iter().filter(|branch| branch.to_lowercase().contains(&query)).cloned().collect::<Vec<_>>();
    div()
      .size_full()
      .min_h_0()
      .flex()
      .flex_col()
      .bg(rgb(PANEL))
      .child(div().h(px(36.)).p_1().flex_shrink_0().child(self.remote_history.branch_filter.clone()))
      .child(
        div()
          .id("history-branch-tree")
          .flex_1()
          .min_h_0()
          .overflow_y_scroll()
          .py_1()
          .px_1()
          .child(
            div()
              .id("history-head")
              .flex()
              .items_center()
              .gap_2()
              .h(px(28.))
              .px_2()
              .rounded_sm()
              .cursor_pointer()
              .when(self.remote_history.target == HistoryTarget::Head, |row| row.bg(rgb(Hover)).text_color(rgb(Accent)))
              .child(Icon::new(IconName::GitBranch).size(px(13.)))
              .child(
                div()
                  .min_w_0()
                  .text_ellipsis()
                  .overflow_hidden()
                  .child(tf("HEAD · {branch}", &[("branch", self.status.branch.clone())])),
              )
              .on_click(cx.listener(|this, _, _, cx| this.browse_history_target(HistoryTarget::Head, cx))),
          )
          .child(
            div()
              .id("history-locals")
              .flex()
              .items_center()
              .gap_2()
              .h(px(28.))
              .px_2()
              .cursor_pointer()
              .text_color(rgb(MUTED))
              .child(Icon::new(if self.remote_history.local_expanded { IconName::ChevronDown } else { IconName::ChevronRight }).size(px(12.)))
              .child(t("Local"))
              .on_click(cx.listener(|this, _, _, cx| {
                this.remote_history.local_expanded = !this.remote_history.local_expanded;
                cx.notify();
              })),
          )
          .when(self.remote_history.local_expanded, |tree| {
            tree
              .when(local_branches.is_empty(), |tree| tree.child(div().px_3().py_1().text_color(rgb(MUTED)).child(t("No local branches"))))
              .children(local_branches.into_iter().enumerate().map(|(index, branch)| {
                let selected = self.remote_history.target == HistoryTarget::Local(branch.clone());
                div()
                  .id(("history-local-branch", index))
                  .h(px(27.))
                  .pl_5()
                  .pr_2()
                  .flex()
                  .items_center()
                  .gap_2()
                  .rounded_sm()
                  .cursor_pointer()
                  .when(selected, |row| row.bg(rgb(Hover)).text_color(rgb(Accent)))
                  .hover(move |style| style.bg(rgb(Hover)))
                  .child(Icon::new(IconName::GitBranch).size(px(12.)))
                  .child(div().min_w_0().text_ellipsis().overflow_hidden().child(branch.clone()))
                  .on_click(cx.listener(move |this, _, _, cx| this.browse_history_target(HistoryTarget::Local(branch.clone()), cx)))
              }))
          })
          .child(
            div()
              .id("history-remotes")
              .flex()
              .items_center()
              .gap_2()
              .h(px(28.))
              .px_2()
              .cursor_pointer()
              .text_color(rgb(MUTED))
              .child(Icon::new(if self.remote_history.remote_expanded { IconName::ChevronDown } else { IconName::ChevronRight }).size(px(12.)))
              .child(t("Remote"))
              .on_click(cx.listener(|this, _, _, cx| {
                this.remote_history.remote_expanded = !this.remote_history.remote_expanded;
                cx.notify();
              })),
          )
          .when(self.remote_history.remote_expanded, |tree| {
            tree
              .when(remote_branches.is_empty(), |tree| tree.child(div().px_3().py_1().text_color(rgb(MUTED)).child(t("No remote branches"))))
              .children(remote_branches.into_iter().enumerate().map(|(index, branch)| {
                let selected = self.remote_history.target == HistoryTarget::Remote(branch.clone());
                div()
                  .id(("history-remote-branch", index))
                  .h(px(27.))
                  .pl_5()
                  .pr_2()
                  .flex()
                  .items_center()
                  .gap_2()
                  .rounded_sm()
                  .cursor_pointer()
                  .when(selected, |row| row.bg(rgb(Hover)).text_color(rgb(Accent)))
                  .hover(move |style| style.bg(rgb(Hover)))
                  .child(Icon::new(IconName::GitBranch).size(px(12.)))
                  .child(div().min_w_0().text_ellipsis().overflow_hidden().child(branch.clone()))
                  .on_click(cx.listener(move |this, _, _, cx| this.browse_history_target(HistoryTarget::Remote(branch.clone()), cx)))
              }))
          }),
      )
  }

  fn remote_commit_row(&self, row: usize, window_active: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
    let rgb = palette(cx);
    let history = self.remote_history.result.as_ref().unwrap();
    let index = self.remote_history.rows[row];
    let commit = &history.commits[index];
    let hash = commit.hash.clone();
    let selected = self.remote_history.selected.as_ref() == Some(&hash);
    let (selected_background, selected_foreground) = selected_row_palette(cx, window_active);
    div()
      .id(("remote-commit", row))
      .w_full()
      .min_w_0()
      .flex()
      .items_center()
      .h(px(29.))
      .px_3()
      .gap_2()
      .border_b_1()
      .border_color(rgb(Divider))
      .cursor_pointer()
      .bg(if selected { selected_background } else { rgb(PANEL) })
      .when(selected, |row| row.text_color(selected_foreground).font_weight(FontWeight::BOLD))
      .when(!selected, |row| row.hover(move |style| style.bg(rgb(Hover))))
      .child(
        div()
          .w(px(68.))
          .flex_shrink_0()
          .text_size(px(11.))
          .text_color(if selected { selected_foreground } else { rgb(MUTED) })
          .child(format!("r{}", commit.number)),
      )
      .child(
        div()
          .w(px(90.))
          .flex_shrink_0()
          .min_w_0()
          .text_ellipsis()
          .overflow_hidden()
          .text_size(px(11.))
          .text_color(if selected { selected_foreground } else { rgb(MUTED) })
          .child(commit.hash.chars().take(10).collect::<String>()),
      )
      .child(
        div()
          .flex_1()
          .min_w_0()
          .flex()
          .items_center()
          .gap_1()
          .child(div().flex_1().min_w_0().text_ellipsis().overflow_hidden().child(commit_message(commit)))
          .when(commit.parents.len() > 1, |cell| {
            cell.child(Icon::new(IconName::GitMerge).size(px(13.)).text_color(if selected { selected_foreground } else { rgb(Accent) }))
          }),
      )
      .child(
        div()
          .w(px(100.))
          .flex_shrink_0()
          .min_w_0()
          .text_ellipsis()
          .overflow_hidden()
          .text_size(px(11.))
          .text_color(if selected { selected_foreground } else { rgb(MUTED) })
          .child(if commit.author.is_empty() { "—".into() } else { commit.author.clone() }),
      )
      .child(
        div()
          .w(px(120.))
          .flex_shrink_0()
          .min_w_0()
          .text_ellipsis()
          .overflow_hidden()
          .text_size(px(11.))
          .text_color(if selected { selected_foreground } else { rgb(MUTED) })
          .child(commit_date(commit.timestamp)),
      )
      .on_click(cx.listener(move |this, _, _, cx| this.select_remote_commit(hash.clone(), cx)))
  }

  fn remote_revision_details(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let rgb = palette(cx);
    let selected = self.remote_history.selected_commit().filter(|_| self.connected);
    let mut files = div().id("remote-revision-files").flex_1().min_h_0().overflow_y_scroll().p_2();
    if selected.is_none() {
      files = files.child(div().p_2().text_color(rgb(MUTED)).child(t("Select a commit to view details.")));
    } else if self.remote_history.files_loading {
      files = files.child(div().p_2().text_color(rgb(MUTED)).child(t("Loading changed files…")));
    } else {
      match &self.remote_history.files {
        Some(Ok(changes)) => {
          if changes.is_empty() {
            files = files.child(div().p_2().text_color(rgb(MUTED)).child(t("No changed files in this revision.")));
          }
          let mut folders = std::collections::BTreeMap::<String, Vec<&backend::RevisionFile>>::new();
          for file in changes {
            let folder = file.path.rsplit_once('/').map_or("", |(folder, _)| folder);
            folders.entry(folder.into()).or_default().push(file);
          }
          for (index, (folder, changes)) in folders.into_iter().enumerate() {
            let collapsed = self.remote_history.collapsed_folders.contains(&folder);
            let toggle_folder = folder.clone();
            files = files.child(
              div()
                .id(("revision-folder", index))
                .flex()
                .items_center()
                .gap_1()
                .h(px(26.))
                .cursor_pointer()
                .child(Icon::new(if collapsed { IconName::ChevronRight } else { IconName::ChevronDown }).size(px(12.)))
                .child(Icon::new(IconName::Folder).size(px(13.)))
                .child(
                  div()
                    .min_w_0()
                    .text_ellipsis()
                    .overflow_hidden()
                    .child(if folder.is_empty() { t("Repository root") } else { folder.clone() }),
                )
                .child(div().flex_shrink_0().text_color(rgb(MUTED)).child(changes.len().to_string()))
                .on_click(cx.listener(move |this, _, _, cx| {
                  if !this.remote_history.collapsed_folders.remove(&toggle_folder) {
                    this.remote_history.collapsed_folders.insert(toggle_folder.clone());
                  }
                  cx.notify();
                })),
            );
            if !collapsed {
              for file in changes {
                let action = file.action.to_lowercase();
                let (marker, color) = match action.as_str() {
                  "add" | "create" => ("A", Success),
                  "delete" | "remove" => ("D", Danger),
                  "move" | "rename" => ("R", Accent),
                  _ if file.is_modified() => ("M", Accent),
                  "copy" => ("C", Success),
                  _ => ("?", MUTED),
                };
                let label = if file.from_path.is_empty() {
                  file.path.rsplit('/').next().unwrap_or(&file.path).to_owned()
                } else {
                  format!("{} → {}", file.from_path, file.path)
                };
                let path = file.path.clone();
                let select_path = file.path.clone();
                let file_selected = self.remote_history.selected_file.as_ref() == Some(&file.path);
                files = files.child(
                  div()
                    .id(SharedString::from(format!("revision-file-{}", file.path)))
                    .h(px(25.))
                    .pl_5()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .when(file_selected, |row| row.bg(rgb(Hover)))
                    .hover(move |style| style.bg(rgb(Hover)))
                    .child(div().w(px(12.)).flex_shrink_0().text_color(rgb(color)).child(marker))
                    .child(div().min_w_0().text_ellipsis().overflow_hidden().child(label))
                    .tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(path.clone()).build(window, cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                      if !this.busy {
                        this.remote_history.selected_file = Some(select_path.clone());
                        cx.notify();
                      }
                    })),
                );
              }
            }
          }
        }
        Some(Err(error)) => {
          files = files
            .child(div().text_color(rgb(Danger)).child(tf("Changed files unavailable: {error}", &[("error", t(error))])))
            .child(
              div()
                .id("retry-revision-files")
                .mt_2()
                .h(px(28.))
                .px_2()
                .flex()
                .items_center()
                .rounded_sm()
                .border_1()
                .border_color(rgb(BORDER))
                .cursor_pointer()
                .hover(move |style| style.bg(rgb(Hover)))
                .child(t("Retry"))
                .on_click(cx.listener(|this, _, _, cx| {
                  if let Some(hash) = this.remote_history.selected.clone() {
                    this.select_remote_commit(hash, cx);
                  }
                })),
            );
        }
        None => {}
      }
    }
    let mut details = div().id("remote-revision-message").h(px(135.)).flex_shrink_0().overflow_y_scroll().p_3().flex().flex_col().gap_2();
    if let Some(commit) = selected {
      let author = if commit.author.is_empty() { "—" } else { &commit.author };
      let parents = commit.parents.iter().map(|parent| parent.chars().take(10).collect::<String>()).collect::<Vec<_>>().join(", ");
      let metadata = format!("r{} · {}\n{}\n{}", commit.number, commit_date(commit.timestamp), author, commit.hash);
      details = details
        .child(div().font_weight(FontWeight::SEMIBOLD).child(commit_message(commit)))
        .child(div().text_color(rgb(MUTED)).child(metadata))
        .when(!parents.is_empty(), |details| {
          details.child(div().text_color(rgb(MUTED)).child(tf("Parents: {parents}", &[("parents", parents)])))
        });
    }
    let can_compare = !self.busy && self.connected && self.remote_history.comparison().is_some();
    let file_panel = div()
      .flex_1()
      .min_w_0()
      .min_h_0()
      .flex()
      .flex_col()
      .child(
        div()
          .flex()
          .items_center()
          .gap_1()
          .px_2()
          .py_1()
          .flex_shrink_0()
          .border_b_1()
          .border_color(rgb(BORDER))
          .child(
            div()
              .id("diff-remote-file")
              .h(px(28.))
              .px_2()
              .flex()
              .items_center()
              .rounded_sm()
              .border_1()
              .border_color(rgb(BORDER))
              .text_color(rgb(if can_compare { TEXT } else { MUTED }))
              .when(can_compare, |button| {
                button
                  .cursor_pointer()
                  .hover(move |style| style.bg(rgb(Hover)))
                  .on_click(cx.listener(|this, _, _, cx| this.diff_remote_file(cx)))
              })
              .child(t("Diff")),
          )
          .child(
            div()
              .id("patch-remote-file")
              .h(px(28.))
              .px_2()
              .flex()
              .items_center()
              .rounded_sm()
              .border_1()
              .border_color(rgb(BORDER))
              .text_color(rgb(if can_compare { TEXT } else { MUTED }))
              .when(can_compare, |button| {
                button
                  .cursor_pointer()
                  .hover(move |style| style.bg(rgb(Hover)))
                  .on_click(cx.listener(|this, _, _, cx| this.create_remote_patch(cx)))
              })
              .child(t("Create patch")),
          ),
      )
      .child(
        div()
          .px_2()
          .text_size(px(10.))
          .text_color(rgb(MUTED))
          .flex_shrink_0()
          .child(t("M files: first parent → selected commit")),
      )
      .child(files);
    div().size_full().min_h_0().flex().flex_col().child(file_panel).child(details.border_t_1().border_color(rgb(BORDER)))
  }

  pub(super) fn render_remote_history(&mut self, window_active: bool, cx: &mut Context<Self>) -> impl IntoElement {
    let rgb = palette(cx);
    let query = self.remote_history.filter.read(cx).content.to_lowercase();
    let loaded_total = self.remote_history.result.as_ref().map_or(0, |history| history.commits.len());
    let page_range = remote_history_page_range(self.remote_history.page, loaded_total);
    self.remote_history.rows = self
      .remote_history
      .result
      .as_ref()
      .ok()
      .filter(|_| self.connected)
      .map(|history| {
        history
          .commits
          .iter()
          .enumerate()
          .skip(page_range.start)
          .take(page_range.len())
          .filter(|(_, commit)| matches_commit(commit, &query))
          .map(|(index, _)| index)
          .collect()
      })
      .unwrap_or_default();
    let count = self.remote_history.rows.len();
    let page_count = page_range.len();
    let page_number = self.remote_history.page + 1;
    let has_previous = self.remote_history.page > 0;
    let has_next = page_range.end < loaded_total && page_range.end < REMOTE_HISTORY_MAX_COMMITS;
    let branch = self.remote_history.result.as_ref().map_or_else(|_| self.status.branch.clone(), |history| history.branch.clone());
    let message = if !self.connected {
      t("Open a connected Lore repository to view history.")
    } else {
      match &self.remote_history.result {
        Ok(_) if loaded_total > 0 => t("No matching commits."),
        Ok(_) => t("No history for this branch."),
        Err(error) => tf("History unavailable: {error}", &[("error", t(error))]),
      }
    };
    let list = div()
      .size_full()
      .min_w_0()
      .min_h_0()
      .flex()
      .flex_col()
      .child(
        div()
          .h(px(36.))
          .px_1()
          .flex()
          .items_center()
          .gap_2()
          .flex_shrink_0()
          .border_b_1()
          .border_color(rgb(BORDER))
          .child(div().flex_1().min_w_0().child(self.remote_history.filter.clone()))
          .child(
            div()
              .text_color(rgb(MUTED))
              .child(format!("{branch} · {} · {count}/{page_count}", tf("Page {page}", &[("page", page_number.to_string())]))),
          )
          .child(
            self
              .button("previous-remote-history", "Previous", !self.busy && self.connected && has_previous)
              .on_click(cx.listener(|this, _, _, cx| {
                this.browse_remote_history_page(this.remote_history.page.saturating_sub(1), cx);
              })),
          )
          .child(
            self
              .button("next-remote-history", "Next", !self.busy && self.connected && has_next)
              .on_click(cx.listener(|this, _, _, cx| {
                this.browse_remote_history_page(this.remote_history.page.saturating_add(1), cx);
              })),
          )
          .child(
            self
              .button("refresh-remote-history", "Refresh", !self.busy && self.connected)
              .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
          ),
      )
      .child(
        div()
          .h(px(32.))
          .pl_3()
          .pr(px(24.))
          .flex()
          .items_center()
          .gap_2()
          .text_size(px(11.))
          .text_color(rgb(MUTED))
          .flex_shrink_0()
          .border_b_1()
          .border_color(rgb(BORDER))
          .bg(rgb(Header))
          .child(div().w(px(68.)).flex_shrink_0().child(t("Revision")))
          .child(div().w(px(90.)).flex_shrink_0().child(t("Hash")))
          .child(div().flex_1().min_w_0().child(t("Commit message")))
          .child(div().w(px(100.)).flex_shrink_0().child(t("Author")))
          .child(div().w(px(120.)).flex_shrink_0().child(t("Date"))),
      )
      .when(count == 0, |list| list.child(div().p_3().text_color(rgb(MUTED)).child(message)))
      .when(count > 0, |list| {
        list.child(
          div()
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(
              uniform_list(
                "remote-history-rows",
                count,
                cx.processor(move |this, range: std::ops::Range<usize>, _, cx| range.map(|row| this.remote_commit_row(row, window_active, cx)).collect::<Vec<_>>()),
              )
              .size_full()
              .pr(px(12.))
              .track_scroll(&self.remote_history.scroll),
            )
            .child(gpui_component::scroll::Scrollbar::vertical(&self.remote_history.scroll).mode(gpui_component::scroll::ScrollbarMode::Always)),
        )
      });
    div()
      .flex_1()
      .min_h_0()
      .min_w_0()
      .text_size(px(12.))
      .flex()
      .child(div().w(px(170.)).h_full().flex_shrink_0().border_r_1().border_color(rgb(BORDER)).child(self.remote_branch_tree(cx)))
      .child(div().flex_1().min_w_0().h_full().child(list))
      .child(
        div()
          .w(px(280.))
          .h_full()
          .flex_shrink_0()
          .border_l_1()
          .border_color(rgb(BORDER))
          .child(self.remote_revision_details(cx)),
      )
  }
}

#[cfg(test)]
mod tests {
  use super::{commit_date, matches_commit, remote_history_limit, remote_history_page_range};
  use crate::backend;

  #[test]
  fn search_matches_message_hash_author_and_revision() {
    let commit = backend::RemoteCommit {
      hash: "aB1234".into(),
      number: 42,
      author: "홍길동".into(),
      message: "Fix Preview\nDetails".into(),
      ..Default::default()
    };
    for query in ["", "preview", "ab12", "홍길동", "42", "details"] {
      assert!(matches_commit(&commit, query));
    }
    assert!(!matches_commit(&commit, "unknown"));
    assert_eq!(commit_date(None), "—");
    assert_eq!(commit_date(Some(i64::MAX)), "—");
  }

  #[test]
  fn remote_history_pages_show_one_hundred_commits_and_keep_a_next_page_sentinel() {
    assert_eq!(remote_history_limit(0), 101);
    assert_eq!(remote_history_limit(1), 201);
    assert_eq!(remote_history_page_range(0, 201), 0..100);
    assert_eq!(remote_history_page_range(1, 201), 100..200);
    assert_eq!(remote_history_page_range(2, 201), 200..201);
    assert_eq!(remote_history_limit(usize::MAX), 10000);
  }
}
