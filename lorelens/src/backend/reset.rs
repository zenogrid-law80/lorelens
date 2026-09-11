use std::path::Path;

/// Restore only the confirmed paths. Lore rejects reset on staged nodes, so
/// unstage the same paths first. Force makes Lore process a confirmed deleted
/// path even when normal filesystem filtering would skip it; absolute paths
/// keep the operation independent of the CLI's current working directory.
pub fn reset_commands(root: &Path, paths: &[String]) -> Vec<Vec<String>> {
  if paths.is_empty() {
    return Vec::new();
  }
  let mut paths = paths.iter().map(|path| root.join(path).display().to_string()).collect::<Vec<_>>();
  paths.sort();
  paths.dedup();
  let mut unstage = vec!["unstage".into(), "--force".into(), "--".into()];
  unstage.extend(paths.iter().cloned());
  let mut reset = vec!["reset".into(), "--purge".into(), "--".into()];
  reset.extend(paths);
  vec![unstage, reset]
}

#[cfg(test)]
mod tests {
  use super::reset_commands;
  use crate::backend::{find_cli, parse_status, run_as};
  use std::{fs, path::Path};

  #[test]
  fn empty_selection_runs_nothing() {
    assert!(reset_commands(Path::new("C:\\repo"), &[]).is_empty());
  }

  #[test]
  #[ignore = "Requires a usable Lore CLI; creates an isolated repository under target"]
  fn real_lore_reset_restores_staged_deletions_and_preserves_other_changes() {
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    fs::create_dir_all(&target).unwrap();
    let workspace = tempfile::Builder::new().prefix("reset-regression-").tempdir_in(target).unwrap();
    let root = workspace.path();
    let cli = find_cli();
    let invoke = |args: &[&str], json| run_as(&cli, root, &args.iter().map(|arg| (*arg).into()).collect::<Vec<_>>(), json, None).unwrap_or_else(|error| panic!("{}: {error}", args.join(" ")));
    invoke(&["repository", "create", "--offline", "lorelens-reset-test"], false);
    let deleted = "GameDesign/LocalServer/kafka/libs/rocksdbjni-10.1.3.jar";
    let modified = "한글 modified file.txt";
    let unstaged = "unstaged.txt";
    let untouched = "keep-staged.txt";
    let untouched_deleted = "keep-deleted.txt";
    fs::create_dir_all(root.join(deleted).parent().unwrap()).unwrap();
    for path in [deleted, modified, unstaged, untouched, untouched_deleted] {
      fs::write(root.join(path), b"committed bytes\0\xff\n").unwrap();
    }
    invoke(&["stage", "--", deleted, modified, unstaged, untouched, untouched_deleted], false);
    invoke(&["commit", "--", "Reset regression baseline"], false);
    fs::remove_file(root.join(deleted)).unwrap();
    fs::remove_file(root.join(untouched_deleted)).unwrap();
    invoke(&["status", "--scan"], false);
    for path in [modified, unstaged, untouched, "--new file.txt", "untracked.txt"] {
      fs::write(root.join(path), "local changes\n").unwrap();
    }
    invoke(&["stage", "--", deleted, modified, untouched, untouched_deleted, "--new file.txt"], false);
    let before = parse_status(&invoke(&["status", "--scan"], true)).unwrap();
    assert!(
      before
        .changes
        .iter()
        .any(|change| change.path == deleted && change.staged && matches!(change.action.as_str(), "remove" | "delete"))
    );

    let paths = [deleted, modified, unstaged, "--new file.txt", "untracked.txt", deleted].map(str::to_owned);
    for args in reset_commands(root, &paths) {
      run_as(&cli, root, &args, false, None).unwrap_or_else(|error| panic!("{}: {error}", args.join(" ")));
    }
    for path in [deleted, modified, unstaged] {
      assert_eq!(fs::read(root.join(path)).unwrap(), b"committed bytes\0\xff\n", "{path}");
    }
    assert!(!root.join("--new file.txt").exists());
    assert!(!root.join("untracked.txt").exists());
    assert_eq!(fs::read_to_string(root.join(untouched)).unwrap(), "local changes\n");
    assert!(!root.join(untouched_deleted).exists());
    let after = parse_status(&invoke(&["status", "--scan"], true)).unwrap();
    assert!(after.changes.iter().all(|change| !paths.contains(&change.path)), "{after:?}");
    for path in [untouched, untouched_deleted] {
      assert!(after.changes.iter().any(|change| change.path == path && change.staged), "{after:?}");
    }
  }
}
