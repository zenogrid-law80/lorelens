use std::collections::{BTreeMap, HashSet};

use crate::backend::Change;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PendingTreeRow {
  Folder { path: String, name: String, depth: usize, change_index: Option<usize> },
  Change { index: usize, name: String, depth: usize },
}

#[derive(Default)]
struct PendingTreeFolder {
  name: String,
  path: String,
  folders: BTreeMap<String, PendingTreeFolder>,
  changes: Vec<(String, usize)>,
}

pub(super) fn pending_tree_rows(changes: &[Change], visible: &[usize], collapsed: &HashSet<String>) -> Vec<PendingTreeRow> {
  let mut root = PendingTreeFolder::default();
  let mut changed_paths = HashSet::new();
  for &index in visible {
    let Some(change) = changes.get(index) else { continue };
    let components: Vec<_> = change.path.split(['/', '\\']).filter(|component| !component.is_empty()).collect();
    let Some((name, folders)) = components.split_last() else {
      root.changes.push((change.path.clone(), index));
      continue;
    };
    changed_paths.insert(components.join("/"));
    let mut parent = &mut root;
    let mut folder_path = String::new();
    for folder in folders {
      if !folder_path.is_empty() {
        folder_path.push('/');
      }
      folder_path.push_str(folder);
      parent = parent.folders.entry((*folder).into()).or_insert_with(|| PendingTreeFolder {
        name: (*folder).into(),
        path: folder_path.clone(),
        ..Default::default()
      });
    }
    parent.changes.push(((*name).into(), index));
  }
  let mut rows = Vec::new();
  flatten_pending_folder_contents(&root, 0, &changed_paths, collapsed, &mut rows);
  rows
}

fn flatten_pending_folder_contents(folder: &PendingTreeFolder, depth: usize, changed_paths: &HashSet<String>, collapsed: &HashSet<String>, rows: &mut Vec<PendingTreeRow>) {
  let mut items: Vec<_> = folder
    .folders
    .keys()
    .cloned()
    .map(|name| (name, None))
    .chain(
      folder
        .changes
        .iter()
        .filter(|(name, _)| !folder.folders.contains_key(name))
        .map(|(name, index)| (name.clone(), Some(*index))),
    )
    .collect();
  items.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
  for (name, change_index) in items {
    if let Some(index) = change_index {
      rows.push(PendingTreeRow::Change { index, name, depth });
    } else if let Some(child) = folder.folders.get(&name) {
      let change_index = folder.changes.iter().find_map(|(change_name, index)| (change_name == &name).then_some(*index));
      flatten_pending_folder(child, change_index, depth, changed_paths, collapsed, rows);
    }
  }
}

fn flatten_pending_folder(folder: &PendingTreeFolder, change_index: Option<usize>, depth: usize, changed_paths: &HashSet<String>, collapsed: &HashSet<String>, rows: &mut Vec<PendingTreeRow>) {
  let collapse_path = folder.path.clone();
  let mut name = folder.name.clone();
  let mut contents = folder;
  while change_index.is_none() && contents.changes.is_empty() && !changed_paths.contains(&contents.path) && contents.folders.len() == 1 {
    let child = contents.folders.values().next().expect("single child folder");
    name.push('/');
    name.push_str(&child.name);
    contents = child;
  }
  rows.push(PendingTreeRow::Folder {
    path: collapse_path.clone(),
    name,
    depth,
    change_index,
  });
  if !collapsed.contains(&collapse_path) {
    flatten_pending_folder_contents(contents, depth + 1, changed_paths, collapsed, rows);
  }
}

pub(super) fn pending_folder_paths(changes: &[Change], folder: &str) -> Vec<String> {
  let folder: Vec<_> = folder.split(['/', '\\']).filter(|component| !component.is_empty()).collect();
  let mut paths: Vec<_> = changes
    .iter()
    .filter(|change| {
      let path: Vec<_> = change.path.split(['/', '\\']).filter(|component| !component.is_empty()).collect();
      path.len() > folder.len() && path.starts_with(&folder) && !matches!(change.node_type.to_ascii_lowercase().as_str(), "directory" | "folder")
    })
    .map(|change| change.path.clone())
    .collect();
  paths.sort();
  paths.dedup();
  paths
}

#[derive(Default)]
pub(super) struct SelectionState {
  pub current: Option<String>,
  pub paths: HashSet<String>,
  pub anchor: Option<String>,
}

impl SelectionState {
  pub fn select_all(&mut self, visible: &[String]) {
    self.paths = visible.iter().cloned().collect();
    self.current = self.current.take().filter(|p| self.paths.contains(p)).or_else(|| visible.first().cloned());
    self.anchor = self.current.clone();
  }

  pub fn click(&mut self, path: String, visible: &[String], additive: bool, range: bool) {
    if range {
      let start = self.anchor.as_ref().and_then(|p| visible.iter().position(|v| v == p));
      let end = visible.iter().position(|p| p == &path);
      if !additive {
        self.paths.clear();
      }
      if let (Some(start), Some(end)) = (start, end) {
        self.paths.extend(visible[start.min(end)..=start.max(end)].iter().cloned());
      } else {
        self.paths.insert(path.clone());
        self.anchor = Some(path.clone());
      }
    } else {
      if !additive {
        self.paths.clear();
      }
      if !self.paths.remove(&path) {
        self.paths.insert(path.clone());
      }
      self.anchor = Some(path.clone());
    }
    self.current = if self.paths.contains(&path) {
      Some(path)
    } else {
      visible.iter().find(|p| self.paths.contains(*p)).cloned()
    };
  }

  pub fn select(&mut self, path: String) {
    self.current = Some(path.clone());
    self.paths.clear();
    self.paths.insert(path.clone());
    self.anchor = Some(path);
  }
  pub fn clear(&mut self) {
    *self = Self::default();
  }
}

#[derive(Default)]
pub(super) struct PreviewState {
  pub path: Option<String>,
  pub content: String,
  generation: u64,
  loading: bool,
}

impl PreviewState {
  pub fn begin(&mut self) -> u64 {
    self.generation = self.generation.wrapping_add(1);
    self.loading = true;
    self.generation
  }

  pub fn accepts(&self, generation: u64) -> bool {
    self.loading && self.generation == generation
  }

  pub fn finish(&mut self) {
    self.loading = false;
  }
  pub fn invalidate(&mut self) {
    self.generation = self.generation.wrapping_add(1);
    self.loading = false;
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn change(path: &str) -> Change {
    Change {
      path: path.into(),
      ..Default::default()
    }
  }

  #[test]
  fn pending_changes_are_grouped_by_directory_and_can_be_collapsed() {
    let changes = [change("README.md"), change("src/main.rs"), change("src/ui/menu.rs"), change("tests/app.rs")];
    let visible = [0, 1, 2, 3];
    assert_eq!(
      pending_tree_rows(&changes, &visible, &HashSet::new()),
      [
        PendingTreeRow::Change {
          index: 0,
          name: "README.md".into(),
          depth: 0
        },
        PendingTreeRow::Folder {
          path: "src".into(),
          name: "src".into(),
          depth: 0,
          change_index: None
        },
        PendingTreeRow::Change {
          index: 1,
          name: "main.rs".into(),
          depth: 1
        },
        PendingTreeRow::Folder {
          path: "src/ui".into(),
          name: "ui".into(),
          depth: 1,
          change_index: None
        },
        PendingTreeRow::Change {
          index: 2,
          name: "menu.rs".into(),
          depth: 2
        },
        PendingTreeRow::Folder {
          path: "tests".into(),
          name: "tests".into(),
          depth: 0,
          change_index: None
        },
        PendingTreeRow::Change {
          index: 3,
          name: "app.rs".into(),
          depth: 1
        },
      ]
    );
    assert_eq!(
      pending_tree_rows(&changes, &visible, &["src".into()].into()),
      [
        PendingTreeRow::Change {
          index: 0,
          name: "README.md".into(),
          depth: 0
        },
        PendingTreeRow::Folder {
          path: "src".into(),
          name: "src".into(),
          depth: 0,
          change_index: None
        },
        PendingTreeRow::Folder {
          path: "tests".into(),
          name: "tests".into(),
          depth: 0,
          change_index: None
        },
        PendingTreeRow::Change {
          index: 3,
          name: "app.rs".into(),
          depth: 1
        },
      ]
    );
  }

  #[test]
  fn pending_change_tree_compacts_single_folder_chains_until_a_branch() {
    let changes = [change("src/platform/linux/app.rs"), change("src/platform/windows/app.rs")];
    assert_eq!(
      pending_tree_rows(&changes, &[0, 1], &HashSet::new()),
      [
        PendingTreeRow::Folder {
          path: "src".into(),
          name: "src/platform".into(),
          depth: 0,
          change_index: None
        },
        PendingTreeRow::Folder {
          path: "src/platform/linux".into(),
          name: "linux".into(),
          depth: 1,
          change_index: None
        },
        PendingTreeRow::Change {
          index: 0,
          name: "app.rs".into(),
          depth: 2
        },
        PendingTreeRow::Folder {
          path: "src/platform/windows".into(),
          name: "windows".into(),
          depth: 1,
          change_index: None
        },
        PendingTreeRow::Change {
          index: 1,
          name: "app.rs".into(),
          depth: 2
        },
      ]
    );
    assert_eq!(
      pending_tree_rows(&changes, &[0, 1], &["src".into()].into()),
      [PendingTreeRow::Folder {
        path: "src".into(),
        name: "src/platform".into(),
        depth: 0,
        change_index: None
      }]
    );
  }

  #[test]
  fn pending_change_tree_merges_a_changed_folder_with_its_tree_row() {
    let mut folder = change("src");
    folder.node_type = "directory".into();
    let changes = [folder, change("src/main.rs")];
    assert_eq!(
      pending_tree_rows(&changes, &[0, 1], &HashSet::new()),
      [
        PendingTreeRow::Folder {
          path: "src".into(),
          name: "src".into(),
          depth: 0,
          change_index: Some(0)
        },
        PendingTreeRow::Change {
          index: 1,
          name: "main.rs".into(),
          depth: 1
        }
      ]
    );
  }

  #[test]
  fn pending_change_tree_uses_only_filtered_indices_and_accepts_windows_separators() {
    let changes = [change("src\\main.rs"), change("src/ui/menu.rs")];
    assert_eq!(
      pending_tree_rows(&changes, &[0], &HashSet::new()),
      [
        PendingTreeRow::Folder {
          path: "src".into(),
          name: "src".into(),
          depth: 0,
          change_index: None
        },
        PendingTreeRow::Change {
          index: 0,
          name: "main.rs".into(),
          depth: 1
        }
      ]
    );
  }

  #[test]
  fn pending_folder_selection_includes_all_descendants_without_prefix_collisions() {
    let mut directory = change("src/generated");
    directory.node_type = "directory".into();
    let changes = [
      change("src/main.rs"),
      change("src/main.rs"),
      change("src/ui/menu.rs"),
      directory,
      change("src2/other.rs"),
      change("src"),
    ];
    assert_eq!(pending_folder_paths(&changes, "src"), ["src/main.rs", "src/ui/menu.rs"]);
    assert_eq!(pending_folder_paths(&changes, "src\\ui"), ["src/ui/menu.rs"]);
  }

  #[test]
  fn large_selection_includes_paths_beyond_the_old_display_limit() {
    let visible: Vec<_> = (0..10_000).map(|i| format!("file-{i}")).collect();
    let mut state = SelectionState::default();
    state.select_all(&visible);
    assert_eq!(state.paths.len(), 10_000);
    assert!(visible.iter().all(|path| state.paths.contains(path)));
    state.click("file-9999".into(), &visible, true, false);
    assert_eq!(state.paths.len(), 9_999);
    assert!(!state.paths.contains("file-9999"));
    state.click("file-9999".into(), &visible, false, false);
    state.click("file-1999".into(), &visible, false, true);
    assert_eq!(state.paths.len(), 8_001);
    assert!(state.paths.contains("file-9999"));
    assert!(state.paths.contains("file-1999"));
  }

  #[test]
  fn multiple_selection_supports_toggle_and_ranges() {
    let visible: Vec<String> = ["a", "b", "c", "d"].map(String::from).to_vec();
    let mut state = SelectionState::default();
    state.click("b".into(), &visible, false, false);
    state.click("d".into(), &visible, true, false);
    assert_eq!(state.paths.len(), 2);
    state.click("d".into(), &visible, true, false);
    assert_eq!(state.current.as_deref(), Some("b"));
    state.click("b".into(), &visible, false, true);
    assert_eq!(state.paths, ["b", "c", "d"].map(String::from).into());
    state.click("a".into(), &visible, true, true);
    assert_eq!(state.paths.len(), 4);
    state.click("c".into(), &visible, false, false);
    assert_eq!(state.paths, [String::from("c")].into());
  }

  #[test]
  fn select_all_replaces_other_panel_selection_and_handles_empty_lists() {
    let mut state = SelectionState::default();
    state.select("outside".into());
    let visible = vec!["a".into(), "b".into()];
    state.select_all(&visible);
    assert_eq!(state.paths, visible.into_iter().collect());
    assert_eq!(state.current.as_deref(), Some("a"));
    state.select_all(&[]);
    assert!(state.paths.is_empty());
    assert!(state.current.is_none() && state.anchor.is_none());
  }

  #[test]
  fn stale_previews_are_discarded() {
    let mut state = PreviewState::default();
    let first = state.begin();
    let second = state.begin();
    assert!(!state.accepts(first));
    assert!(state.accepts(second));
    state.invalidate();
    assert!(!state.accepts(second));
    let third = state.begin();
    state.finish();
    assert!(!state.accepts(third));
  }
}
