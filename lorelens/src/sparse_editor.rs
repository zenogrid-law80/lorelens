use super::*;
use backend::sparse::{self, FolderState};
use gpui_component::checkbox::Checkbox;
use gpui_component::scroll::ScrollableElement as _;
use std::collections::{BTreeMap, HashSet};

pub(super) struct SparseEditor {
  root: PathBuf,
  cli: PathBuf,
  identity: Option<String>,
  original: String,
  revision: Option<String>,
  folders: BTreeMap<String, Vec<String>>,
  expanded: HashSet<String>,
  loading: HashSet<String>,
  errors: BTreeMap<String, String>,
  changes: Vec<(String, bool)>,
}

impl SparseEditor {
  pub fn new(root: PathBuf, cli: PathBuf, identity: Option<String>, original: String, cx: &mut Context<Self>) -> Self {
    let mut editor = Self {
      root,
      cli,
      identity,
      original,
      revision: None,
      folders: BTreeMap::new(),
      expanded: HashSet::from([String::new()]),
      loading: HashSet::new(),
      errors: BTreeMap::new(),
      changes: Vec::new(),
    };
    editor.fetch(String::new(), cx);
    editor
  }

  fn fetch(&mut self, parent: String, cx: &mut Context<Self>) {
    if !self.loading.insert(parent.clone()) {
      return;
    }
    self.errors.remove(&parent);
    let root = self.root.clone();
    let cli = self.cli.clone();
    let identity = self.identity.clone();
    let revision = self.revision.clone();
    let query = parent.clone();
    let task = cx
      .background_executor()
      .spawn(async move { sparse::list_folders(&cli, &root, &query, revision.as_deref(), identity.as_deref()) });
    cx.spawn(async move |this, cx| {
      let result = task.await;
      let _ = this.update(cx, |this, cx| {
        this.loading.remove(&parent);
        match result {
          Ok(listing) => {
            this.revision = Some(listing.revision);
            this.folders.insert(parent, listing.folders);
          }
          Err(error) => {
            this.errors.insert(parent, error);
          }
        }
        cx.notify();
      });
    })
    .detach();
    cx.notify();
  }

  pub fn selections(&self) -> Result<Vec<(String, bool)>, String> {
    if self.revision.is_none() || !self.loading.is_empty() {
      return Err(t("Wait for the repository folders to finish loading."));
    }
    Ok(self.changes.clone())
  }

  fn rows(&self, parent: &str, depth: usize, result: &mut Vec<(String, usize)>) {
    if let Some(children) = self.folders.get(parent) {
      for folder in children {
        result.push((folder.clone(), depth));
        if self.expanded.contains(folder) {
          self.rows(folder, depth + 1, result);
        }
      }
    }
  }

  fn checkbox(&self, path: &str, label: String, text: &str, index: usize, cx: &Context<Self>) -> Checkbox {
    let state = sparse::folder_state(text, path);
    let label = if state == FolderState::Mixed {
      tf("{folder} (partial / custom rules)", &[("folder", label)])
    } else {
      label
    };
    let view = cx.entity().downgrade();
    let path = path.to_owned();
    Checkbox::new(("sparse-folder-check", index))
      .label(label)
      .checked(state == FolderState::Included)
      .disabled(self.revision.is_none())
      .on_click(move |included, _, cx| {
        let _ = view.update(cx, |this, cx| {
          sparse::set_folder(&mut this.changes, &path, *included);
          cx.notify();
        });
      })
  }
}

impl Render for SparseEditor {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let text = sparse::selection_text(&self.original, &self.changes).unwrap_or_else(|_| self.original.clone());
    let mut rows = Vec::new();
    self.rows("", 0, &mut rows);
    let mut tree = div().flex().flex_col().gap_1();
    for (index, (path, depth)) in rows.into_iter().enumerate() {
      let expanded = self.expanded.contains(&path);
      let loading = self.loading.contains(&path);
      let target = path.clone();
      let toggle = cx.entity().downgrade();
      tree = tree.child(
        div()
          .flex()
          .items_center()
          .gap_2()
          .pl(px(depth as f32 * 18.))
          .child(Button::new(("sparse-expand", index)).ghost().small().label(if expanded { "▾" } else { "▸" }).on_click(move |_, _, cx| {
            let _ = toggle.update(cx, |this, cx| {
              if !this.expanded.remove(&target) {
                this.expanded.insert(target.clone());
                if !this.folders.contains_key(&target) {
                  this.fetch(target.clone(), cx);
                }
              }
              cx.notify();
            });
          }))
          .child(self.checkbox(&path, path.rsplit('/').next().unwrap_or(&path).to_owned(), &text, index + 1, cx))
          .when(loading, |row| row.child(t("Loading folders…"))),
      );
      if expanded && self.folders.get(&path).is_some_and(Vec::is_empty) {
        tree = tree.child(div().pl(px((depth + 1) as f32 * 18. + 30.)).text_sm().child(t("No subfolders")));
      }
    }
    for (index, (path, error)) in self.errors.iter().enumerate() {
      let retry = cx.entity().downgrade();
      let path = path.clone();
      tree = tree
        .child(div().text_sm().text_color(palette(cx)(Danger)).child(error.clone()))
        .child(Button::new(("sparse-retry", index)).small().label(t("Retry")).on_click(move |_, _, cx| {
          let _ = retry.update(cx, |this, cx| this.fetch(path.clone(), cx));
        }));
    }
    let reset = cx.entity().downgrade();
    div()
      .flex()
      .flex_col()
      .gap_2()
      .child(t("Select repository folders, including folders not downloaded locally. Expanding a folder loads its children."))
      .child(div().flex().items_center().gap_3().child(self.checkbox("", t("All repository folders"), &text, 0, cx)).child(
        Button::new("sparse-reset-selection").small().label(t("Undo selections")).on_click(move |_, _, cx| {
          let _ = reset.update(cx, |this, cx| {
            this.changes.clear();
            cx.notify();
          });
        }),
      ))
      .child(t("Partial / custom rules are preserved until you select the folder. Selecting a folder overrides all rules inside it."))
      .when(self.loading.contains(""), |view| view.child(t("Loading folders…")))
      .when(self.folders.get("").is_some_and(Vec::is_empty), |view| view.child(t("No subfolders")))
      .child(div().id("sparse-tree-scroll").h(px(300.)).overflow_y_scrollbar().child(tree))
  }
}
