use super::{Change, delete_entry, parse_status, run_as};
use std::{
  collections::{BTreeMap, HashSet},
  fs,
  path::{Component, Path},
};

const COMMIT_MESSAGE: &str = "Remove unavailable local server binaries";

/// Return each path that Lore reported more than once in Changes.
pub fn duplicate_change_paths(changes: &[Change]) -> Vec<String> {
  let mut counts = BTreeMap::new();
  for change in changes {
    *counts.entry(change.path.clone()).or_insert(0usize) += 1;
  }
  counts.into_iter().filter_map(|(path, count)| (count >= 2).then_some(path)).collect()
}

/// Find selected local files whose duplicate Change entries contain both a
/// staged deletion and an unstaged keep operation.
pub fn duplicate_local_files(root: &Path, changes: &[Change], selected_paths: &[String]) -> Result<Vec<String>, String> {
  let selected: HashSet<_> = selected_paths.iter().map(String::as_str).collect();
  let mut states = BTreeMap::<&str, (bool, bool)>::new();
  for change in changes.iter().filter(|change| selected.contains(change.path.as_str())) {
    let state = states.entry(&change.path).or_default();
    state.0 |= change.staged && matches!(change.action.to_ascii_lowercase().as_str(), "delete" | "remove");
    state.1 |= !change.staged && change.action.eq_ignore_ascii_case("keep");
  }
  let canonical_root = root.canonicalize().map_err(|error| format!("Cannot resolve repository path {}: {error}", root.display()))?;
  let mut files = Vec::new();
  for (relative, (staged_delete, unstaged_keep)) in states {
    if !staged_delete || !unstaged_keep {
      continue;
    }
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
      || relative_path.components().any(|component| {
        !matches!(component, Component::Normal(_)) || matches!(component, Component::Normal(name) if [".git", ".lore", ".urc"].iter().any(|metadata| name.eq_ignore_ascii_case(metadata)))
      })
    {
      return Err(format!("Invalid duplicate file path: {relative}"));
    }
    let path = root.join(relative_path);
    let metadata = match fs::symlink_metadata(&path) {
      Ok(metadata) => metadata,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
      Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
      continue;
    }
    let canonical = path.canonicalize().map_err(|error| format!("{}: {error}", path.display()))?;
    if !canonical.starts_with(&canonical_root) {
      return Err(format!("Duplicate file is outside the repository: {}", path.display()));
    }
    files.push(relative.to_owned());
  }
  Ok(files)
}

fn delete_local_files(root: &Path, relative_paths: &[String]) -> Result<(), String> {
  for relative in relative_paths {
    delete_entry(root, &root.join(relative)).map_err(|error| format!("Cannot delete local duplicate file {relative}: {error}"))?;
  }
  Ok(())
}

/// Build the exact direct-argument workflow shown in the confirmation dialog.
pub fn deduplicate_commands(paths: &[String]) -> Vec<Vec<String>> {
  let mut paths = paths.to_vec();
  paths.sort();
  paths.dedup();
  if paths.is_empty() {
    return Vec::new();
  }
  let mut stage = vec!["stage".into()];
  stage.extend(paths);
  vec![
    vec!["status".into(), "--scan".into()],
    stage,
    vec!["commit".into(), "--".into(), COMMIT_MESSAGE.into()],
    vec!["push".into()],
    vec!["status".into()],
    vec!["repository".into(), "verify".into()],
  ]
}

/// Recheck the duplicate set, then stage, commit, push, refresh, and verify it.
/// Refuse to include unrelated staged work in the automatic commit.
pub fn deduplicate_files(cli: &Path, root: &Path, identity: Option<&str>, expected_branch: &str, expected_revision: &str, expected_paths: &[String]) -> Result<String, String> {
  let commands = deduplicate_commands(expected_paths);
  let Some(scan) = commands.first() else {
    return Err("No duplicate Change entries were found.".into());
  };
  let scan_output = run_as(cli, root, scan, true, identity).map_err(|error| format!("lore status --scan: {error}"))?;
  let status = parse_status(&scan_output)?;
  let duplicate_paths = duplicate_change_paths(&status.changes);
  if status.branch != expected_branch || status.revision != expected_revision || !expected_paths.iter().all(|path| duplicate_paths.contains(path)) {
    return Err("Duplicate Change entries changed. Refresh and try again.".into());
  }
  let expected: HashSet<_> = expected_paths.iter().map(String::as_str).collect();
  if status.changes.iter().any(|change| change.staged && !expected.contains(change.path.as_str())) {
    return Err("Other staged files must be unstaged first so they are not included in the automatic commit.".into());
  }

  let local_files = duplicate_local_files(root, &status.changes, expected_paths)?;
  delete_local_files(root, &local_files)?;
  let mut output = local_files.iter().map(|relative| format!("Deleted local file: {relative}")).collect::<Vec<_>>();
  let mut final_status = None;
  for args in &commands {
    let json = args.first().is_some_and(|arg| arg == "status");
    let command_output = run_as(cli, root, args, json, identity).map_err(|error| format!("lore {}: {error}", args.join(" ")))?;
    if json {
      final_status = Some(parse_status(&command_output)?);
    }
    output.push(command_output);
  }
  let final_status = final_status.ok_or("Lore did not return the final repository status.")?;
  if final_status.changes.iter().any(|change| expected.contains(change.path.as_str())) {
    return Err("Duplicate Change entries remain after the commit and push. Refresh and inspect the command log.".into());
  }
  let output = output.into_iter().filter(|text| !text.trim().is_empty()).collect::<Vec<_>>().join("\n");
  Ok(if output.is_empty() { "Deduplication completed successfully.".into() } else { output })
}

#[cfg(test)]
mod tests {
  use super::{deduplicate_commands, delete_local_files, duplicate_change_paths, duplicate_local_files};
  use crate::backend::Change;

  fn change(path: &str) -> Change {
    Change {
      path: path.into(),
      ..Change::default()
    }
  }

  #[test]
  fn finds_only_paths_reported_at_least_twice() {
    let paths = duplicate_change_paths(&[change("b.bin"), change("a.bin"), change("b.bin"), change("c.bin"), change("a.bin"), change("a.bin")]);
    assert_eq!(paths, ["a.bin", "b.bin"]);
  }

  #[test]
  fn builds_the_requested_deduplication_workflow() {
    let commands = deduplicate_commands(&["two.bin".into(), "one file.bin".into(), "two.bin".into()]);
    assert_eq!(commands[0], ["status", "--scan"]);
    assert_eq!(commands[1], ["stage", "one file.bin", "two.bin"]);
    assert_eq!(commands[2], ["commit", "--", "Remove unavailable local server binaries"]);
    assert_eq!(commands[3], ["push"]);
    assert_eq!(commands[4], ["status"]);
    assert_eq!(commands[5], ["repository", "verify"]);
    assert!(deduplicate_commands(&[]).is_empty());
  }

  #[test]
  fn selects_existing_files_with_staged_delete_and_unstaged_keep() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("duplicate.bin"), b"local").unwrap();
    std::fs::write(root.path().join("other.bin"), b"other").unwrap();
    let mut deleted = change("duplicate.bin");
    deleted.action = "delete".into();
    deleted.staged = true;
    let mut kept = change("duplicate.bin");
    kept.action = "keep".into();
    let mut unrelated = change("other.bin");
    unrelated.action = "keep".into();
    let changes = [deleted, kept, unrelated];
    let files = duplicate_local_files(root.path(), &changes, &["duplicate.bin".into(), "other.bin".into()]).unwrap();
    assert_eq!(files, ["duplicate.bin"]);
    delete_local_files(root.path(), &files).unwrap();
    assert!(!root.path().join("duplicate.bin").exists());
    assert!(root.path().join("other.bin").is_file());
  }
}
