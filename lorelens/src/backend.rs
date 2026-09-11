mod binary_merge;
mod cli;
mod deduplicate;
mod filesystem;
mod history;
mod ignore;
mod obliterate;
mod reset;
pub use binary_merge::{BinarySide, is_binary_merge, select_binary_merge};
pub fn list_tree(root: &Path, expanded: &std::collections::HashSet<PathBuf>) -> Result<Vec<Entry>, String> {
  fn walk(root: &Path, dir: &Path, expanded: &std::collections::HashSet<PathBuf>, result: &mut Vec<Entry>) -> Result<(), String> {
    for entry in list_directory(root, dir)? {
      let descend = entry.directory && expanded.contains(&entry.path);
      let path = entry.path.clone();
      result.push(entry);
      if descend {
        walk(root, &path, expanded, result)?;
      }
    }
    Ok(())
  }
  let mut result = Vec::new();
  walk(root, root, expanded, &mut result)?;
  Ok(result)
}
pub use cli::{find_cli, run_as};
pub use deduplicate::{deduplicate_commands, deduplicate_files, duplicate_change_paths, duplicate_local_files};
pub use history::{FileRevision, file_history};
pub use obliterate::obliterate_args;
pub use reset::reset_commands;
pub fn cleanup_resolved_merge(cli: &Path, root: &Path, relative: &str, identity: Option<&str>) -> Result<(), String> {
  let output = run_as(cli, root, &["status".into(), "--scan".into()], true, identity)?;
  let events = output
    .lines()
    .filter(|line| !line.trim().is_empty())
    .map(serde_json::from_str::<Value>)
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;
  let normalized = relative.replace('\\', "/");
  let completed = events.iter().any(|event| event["tagName"] == "complete" && event["data"]["status"] == 0);
  let resolved = events.iter().any(|event| {
    let data = &event["data"];
    event["tagName"] == "repositoryStatusFile" && data["path"].as_str().is_some_and(|path| path.replace('\\', "/") == normalized) && data["flagConflictUnresolved"].as_bool() == Some(false)
  });
  if !completed || !resolved {
    return Err(crate::i18n::t("Conflict resolution was not confirmed. Merge backup files were kept."));
  }
  let root = root.canonicalize().map_err(|e| e.to_string())?;
  let file = root.join(relative).canonicalize().map_err(|e| e.to_string())?;
  if !file.starts_with(&root) || !file.is_file() {
    return Err("Invalid merge file path".into());
  }
  let mut backups = Vec::new();
  for suffix in ["~base", "~mine", "~theirs"] {
    let mut name = file.as_os_str().to_owned();
    name.push(suffix);
    let path = PathBuf::from(name);
    match fs::symlink_metadata(&path) {
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
      Err(error) => return Err(error.to_string()),
      Ok(metadata) => {
        if !metadata.is_file() || metadata.file_type().is_symlink() || !path.canonicalize().map_err(|e| e.to_string())?.starts_with(&root) {
          return Err(format!("Invalid merge backup path: {}", path.display()));
        }
        backups.push(path);
      }
    }
  }
  for path in backups {
    fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
  }
  Ok(())
}
pub fn is_repository(root: &Path) -> bool {
  root.join(".lore").is_dir() || root.join(".urc").is_dir()
}
pub use filesystem::{copy_entries, delete_entry, list_directory, preview, validate_text_files};

/// Create placeholders for selected reset targets that are missing from disk.
/// Lore needs a filesystem node to resolve staged deletions during reset.
pub fn prepare_reset_paths(root: &Path, commands: &[Vec<String>]) -> Result<Vec<PathBuf>, String> {
  let mut created = Vec::new();
  for args in commands.iter().filter(|args| args.first().is_some_and(|arg| arg == "reset")) {
    let Some(separator) = args.iter().position(|arg| arg == "--") else { continue };
    for argument in &args[separator + 1..] {
      let path = Path::new(argument);
      let path = if path.is_absolute() { path.to_path_buf() } else { root.join(path) };
      if path.exists() || !path.parent().is_some_and(Path::is_dir) {
        continue;
      }
      match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(_) => created.push(path),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
          for created_path in &created {
            let _ = fs::remove_file(created_path);
          }
          return Err(format!("Cannot prepare reset path {}: {error}", path.display()));
        }
      }
    }
  }
  Ok(created)
}

pub fn cleanup_reset_paths(paths: &[PathBuf]) {
  for path in paths {
    let _ = fs::remove_file(path);
  }
}

use serde_json::Value;
use std::{
  fs,
  io::Read,
  path::{Path, PathBuf},
  process::{Command, Stdio},
  thread,
  time::{Duration, Instant},
};

#[derive(Clone, Debug, Default)]
pub struct Change {
  pub path: String,
  pub node_type: String,
  pub action: String,
  pub staged: bool,
  pub conflict: bool,
}

impl Change {
  pub fn file_marker(&self) -> &'static str {
    if self.conflict {
      "!"
    } else if self.staged {
      "S"
    } else {
      "M"
    }
  }
}

#[derive(Default, Debug)]
pub struct Status {
  pub branch: String,
  pub revision: String,
  pub changes: Vec<Change>,
}

pub fn parse_status(output: &str) -> Result<Status, String> {
  let mut status = Status::default();
  let mut found = false;
  for line in output.lines().filter(|s| !s.trim().is_empty()) {
    let event: Value = serde_json::from_str(line).map_err(|e| format!("Invalid Lore JSON: {e}"))?;
    let data = &event["data"];
    match event["tagName"].as_str().unwrap_or("") {
      "repositoryStatusRevision" => {
        found = true;
        status.branch = data["branchName"].as_str().unwrap_or("unknown").into();
        status.revision = data["revisionNumber"].to_string();
      }
      "repositoryStatusFile" => {
        let path = data["path"].as_str().ok_or("Status file has no path")?;
        status.changes.push(Change {
          path: path.into(),
          node_type: data["type"].as_str().unwrap_or("").to_owned(),
          action: data["action"].as_str().unwrap_or("changed").into(),
          staged: data["flagStaged"].as_bool().unwrap_or(false),
          conflict: data["flagConflictUnresolved"].as_bool().unwrap_or(false),
        });
      }
      "complete" if data["status"].as_i64().unwrap_or(0) != 0 => return Err(data.to_string()),
      _ => {}
    }
  }
  if !found {
    return Err("Lore did not report a repository. Open a Lore repository, or check the CLI version.".into());
  }
  status.changes.sort_by(|a, b| a.path.cmp(&b.path));
  Ok(status)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalCommit {
  pub hash: String,
  pub number: String,
  pub message: String,
}

impl LocalCommit {
  pub fn label(&self) -> String {
    let short: String = self.hash.chars().take(8).collect();
    let message = self.message.split_whitespace().collect::<Vec<_>>().join(" ");
    crate::i18n::tf(
      "{message} · {hash} · Revision {number}",
      &[
        ("message", if message.is_empty() { crate::i18n::t("(No commit message)") } else { message }),
        ("hash", short),
        ("number", self.number.clone()),
      ],
    )
  }
}

pub fn discard_local_commits(cli: &Path, root: &Path, identity: Option<&str>, branch: &str, expected: &[LocalCommit]) -> Result<String, String> {
  let output = run_as(cli, root, &["status".into(), "--scan".into()], true, identity)?;
  let status = parse_status(&output)?;
  if status.branch != branch || !status.changes.is_empty() {
    return Err("Branch changed or working changes exist. Refresh and try again.".into());
  }
  if expected.is_empty() || pending_push(cli, root, &output, identity)? != expected {
    return Err("Local commits changed. Refresh and reopen the dialog.".into());
  }
  let events = output
    .lines()
    .filter(|line| !line.trim().is_empty())
    .map(serde_json::from_str::<Value>)
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;
  let data = &events.iter().find(|event| event["tagName"] == "repositoryStatusRevision").ok_or("Missing repository status")?["data"];
  if data["remoteBranchExist"] != true && data["remoteBranchExist"] != 1 {
    return Err("No remote branch to return to.".into());
  }
  let remote = data["revisionRemote"].as_str().filter(|s| !s.is_empty()).ok_or("Missing remote revision")?;
  let local = data["revisionLocal"].as_str().ok_or("Missing local revision")?;
  let history = run_as(cli, root, &["history".into(), "10000".into(), "--revision".into(), local.into()], true, identity)?;
  if !parse_revision_history(&history)?.iter().any(|commit| commit.hash == remote) {
    return Err("Remote revision is not an ancestor of the local commits. Sync must be resolved first.".into());
  }
  run_as(cli, root, &["branch".into(), "reset".into(), "--branch".into(), branch.into(), remote.into()], false, identity)
}

pub fn pending_push(cli: &Path, root: &Path, status: &str, identity: Option<&str>) -> Result<Vec<LocalCommit>, String> {
  pending_commits(cli, root, status, identity, false)
}

pub fn pending_pull(cli: &Path, root: &Path, status: &str, identity: Option<&str>) -> Result<Vec<LocalCommit>, String> {
  pending_commits(cli, root, status, identity, true)
}

fn pending_commits(cli: &Path, root: &Path, status: &str, identity: Option<&str>, incoming: bool) -> Result<Vec<LocalCommit>, String> {
  let events: Vec<Value> = status
    .lines()
    .filter(|line| !line.trim().is_empty())
    .map(serde_json::from_str)
    .collect::<Result<_, _>>()
    .map_err(|e| e.to_string())?;
  let data = &events.iter().find(|event| event["tagName"] == "repositoryStatusRevision").ok_or("Missing repository status")?["data"];
  let flag = |name: &str| data[name].as_bool().unwrap_or_else(|| data[name].as_u64() == Some(1));
  if !flag("remoteAuthorized") {
    return Err("Remote status unavailable; check authentication.".into());
  }
  if incoming && !matches!(data["remoteBranchExist"], Value::Bool(_) | Value::Number(_)) {
    return Err("Remote branch status unavailable.".into());
  }
  if incoming && (!flag("remoteBranchExist") || data["revisionLocal"].as_str().is_some_and(|local| Some(local) == data["revisionRemote"].as_str())) {
    return Ok(Vec::new());
  }
  if !incoming && !flag("isLocalAhead") {
    return Ok(Vec::new());
  }
  let read_history = |revision: &str| -> Result<Vec<LocalCommit>, String> {
    let output = run_as(cli, root, &["history".into(), "10000".into(), "--revision".into(), revision.into()], true, identity)?;
    parse_revision_history(&output)
  };
  let local = read_history(data["revisionLocal"].as_str().ok_or("Missing local revision")?)?;
  let remote = if flag("remoteBranchExist") {
    read_history(data["revisionRemote"].as_str().ok_or("Missing remote revision")?)?
  } else {
    Vec::new()
  };
  if local.len() >= 10000 || remote.len() >= 10000 {
    return Err("History exceeds 10000 revisions; pending commit count unavailable.".into());
  }
  Ok(if incoming { unique_commits(remote, local) } else { unique_commits(local, remote) })
}

fn unique_commits(source: Vec<LocalCommit>, known: Vec<LocalCommit>) -> Vec<LocalCommit> {
  let known: std::collections::HashSet<_> = known.into_iter().map(|commit| commit.hash).collect();
  source.into_iter().filter(|commit| !known.contains(&commit.hash)).collect()
}

#[cfg(test)]
mod incoming_tests {
  use super::*;

  #[test]
  fn compares_histories_in_both_directions_including_divergence() {
    let commits = |hashes: &[&str]| {
      hashes
        .iter()
        .map(|hash| LocalCommit {
          hash: (*hash).into(),
          number: String::new(),
          message: String::new(),
        })
        .collect::<Vec<_>>()
    };
    assert_eq!(unique_commits(commits(&["remote", "shared"]), commits(&["local", "shared"])), commits(&["remote"]));
    assert!(unique_commits(commits(&["shared"]), commits(&["local", "shared"])).is_empty());
    assert_eq!(unique_commits(commits(&["new", "shared"]), commits(&["shared"])), commits(&["new"]));
  }

  #[test]
  fn incoming_status_handles_equal_missing_and_unauthorized_remotes() {
    let query = |data: Value| {
      let output = serde_json::json!({"tagName": "repositoryStatusRevision", "data": data}).to_string();
      pending_pull(Path::new("missing-cli"), Path::new("."), &output, None)
    };
    assert!(
      query(serde_json::json!({"remoteAuthorized": true, "remoteBranchExist": true, "revisionLocal": "same", "revisionRemote": "same"}))
        .unwrap()
        .is_empty()
    );
    assert!(query(serde_json::json!({"remoteAuthorized": 1, "remoteBranchExist": 0})).unwrap().is_empty());
    assert!(query(serde_json::json!({"remoteAuthorized": false})).is_err());
    assert!(query(serde_json::json!({"remoteAuthorized": true})).is_err());
  }
}

fn parse_revision_history(output: &str) -> Result<Vec<LocalCommit>, String> {
  let mut revisions = Vec::new();
  let mut complete = false;
  for line in output.lines().filter(|line| !line.trim().is_empty()) {
    let event: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    match event["tagName"].as_str() {
      Some("revisionHistoryEntry") => revisions.push(LocalCommit {
        hash: event["data"]["revision"].as_str().ok_or("Missing revision hash")?.into(),
        number: event["data"]["revisionNumber"].as_u64().ok_or("Missing revision number")?.to_string(),
        message: String::new(),
      }),
      Some("metadata") if event["data"]["key"] == "message" => {
        if let Some(commit) = revisions.last_mut() {
          let value = &event["data"]["value"];
          commit.message = value.as_str().or_else(|| value["value"].as_str()).unwrap_or_default().to_string();
        }
      }
      Some("complete") => {
        if event["data"]["status"].as_i64() != Some(0) {
          return Err("History query failed".into());
        }
        complete = true;
      }
      _ => {}
    }
  }
  if !complete || revisions.is_empty() {
    return Err("Incomplete revision history".into());
  }
  Ok(revisions)
}

pub fn parse_lock_status(output: &str) -> Result<bool, String> {
  let mut count = None;
  let mut complete = false;
  for line in output.lines().filter(|line| !line.trim().is_empty()) {
    let event: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    match event["tagName"].as_str() {
      Some("lockFileStatusBegin") => count = event["data"]["count"].as_u64(),
      Some("complete") => {
        if event["data"]["status"].as_i64() != Some(0) {
          return Err(event["data"].to_string());
        }
        complete = true;
      }
      _ => {}
    }
  }
  if !complete {
    return Err("Lock status did not complete.".into());
  }
  count.map(|count| count > 0).ok_or_else(|| "Missing lock status.".into())
}

pub fn parse_locked_paths(output: &str) -> Result<std::collections::HashSet<String>, String> {
  let mut paths = std::collections::HashSet::new();
  let mut complete = false;
  let mut found = false;
  for line in output.lines().filter(|line| !line.trim().is_empty()) {
    let event: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    match event["tagName"].as_str() {
      Some("lockFileQueryBegin") => found = true,
      Some("lockFileQuery") => {
        let path = event["data"]["path"].as_str().ok_or("Lock has no path")?;
        paths.insert(path.replace('\\', "/"));
      }
      Some("complete") => {
        if event["data"]["status"].as_i64() != Some(0) {
          return Err(event["data"].to_string());
        }
        complete = true;
      }
      _ => {}
    }
  }
  if !found || !complete {
    return Err("Incomplete lock query".into());
  }
  Ok(paths)
}
//
// pub fn run(cli: &Path, root: &Path, args: &[String], json: bool) -> Result<String, String> {
//     run_as(cli, root, args, json, None)
// }

pub fn clone_repository(cli: &Path, url: &str, destination: &Path, identity: Option<&str>) -> Result<String, String> {
  if url.trim().is_empty() || !destination.is_absolute() {
    return Err("Repository URL and absolute destination path are required.".into());
  }
  let parent = destination.parent().filter(|parent| parent.is_dir()).ok_or("Destination parent folder must exist.")?;
  if destination.exists() && (!destination.is_dir() || fs::read_dir(destination).map_err(|e| e.to_string())?.next().is_some()) {
    return Err("Destination must be a new or empty folder.".into());
  }
  let output = run_as(cli, parent, &["clone".into(), "--".into(), url.into(), destination.to_string_lossy().into_owned()], false, identity)?;
  if !is_repository(destination) {
    return Err(format!("Clone completed but no repository was found at {}.\n{output}", destination.display()));
  }
  Ok(output)
}

pub fn create_repository(cli: &Path, url: &str, destination: &Path, identity: Option<&str>) -> Result<String, String> {
  if !url.contains("://") || url.ends_with("://") || url.chars().any(char::is_whitespace) {
    return Err("Enter a repository URL including its scheme and repository name.".into());
  }
  if !destination.is_absolute() {
    return Err("Enter an absolute destination path.".into());
  }
  let parent = destination.parent().filter(|path| path.is_dir()).ok_or("Destination parent folder must exist.")?;
  if destination.exists() && (!destination.is_dir() || fs::read_dir(destination).map_err(|e| e.to_string())?.next().is_some()) {
    return Err("Destination must be a new or empty folder.".into());
  }
  let output = run_as(
    cli,
    parent,
    &[
      "--repository".into(),
      destination.to_string_lossy().into_owned(),
      "repository".into(),
      "create".into(),
      "--".into(),
      url.into(),
    ],
    false,
    identity,
  )?;
  if !is_repository(destination) {
    return Err(format!("Create completed but no repository was found at {}.\n{output}", destination.display()));
  }
  Ok(output)
}

pub fn move_entry(root: &Path, source: &Path, destination: &Path) -> Result<(), String> {
  let root = root.canonicalize().map_err(|e| e.to_string())?;
  let source = source.canonicalize().map_err(|e| e.to_string())?;
  let name = destination.file_name().ok_or("Destination must include the new file or folder name.")?;
  let parent = destination
    .parent()
    .ok_or("Destination parent is required.")?
    .canonicalize()
    .map_err(|e| format!("Destination parent: {e}"))?;
  let destination = parent.join(name);
  for path in [&source, &destination] {
    let relative = path.strip_prefix(&root).map_err(|_| "Move must stay inside the repository.")?;
    if relative.as_os_str().is_empty()
      || relative.components().any(|part| {
        let name = part.as_os_str().to_string_lossy();
        name.eq_ignore_ascii_case(".lore") || name.eq_ignore_ascii_case(".git")
      })
    {
      return Err("Cannot move repository root or metadata.".into());
    }
  }
  if fs::symlink_metadata(&destination).is_ok() {
    return Err("Destination already exists.".into());
  }
  if source.is_dir() && destination.starts_with(&source) {
    return Err("Cannot move a folder inside itself.".into());
  }
  fs::rename(&source, &destination).map_err(|e| format!("Move failed: {e}"))
}

pub fn parse_account(output: &str) -> Result<(String, String), String> {
  let mut account = None;
  let mut complete = false;
  for line in output.lines().filter(|line| !line.trim().is_empty()) {
    let event: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    match event["tagName"].as_str() {
      Some("authUserInfo") => {
        let id = event["data"]["id"].as_str().filter(|id| !id.is_empty()).ok_or("Missing account ID")?;
        let name = event["data"]["name"].as_str().filter(|name| !name.is_empty()).unwrap_or(id);
        account = Some((id.into(), name.into()));
      }
      Some("complete") => {
        if event["data"]["status"].as_i64() != Some(0) {
          return Err("Authentication failed".into());
        }
        complete = true;
      }
      _ => {}
    }
  }
  if !complete {
    return Err("Authentication did not complete".into());
  }
  account.ok_or_else(|| "No authenticated account returned".into())
}

pub fn has_valid_login(output: &str, identity: Option<&str>, now_ms: u64) -> Result<bool, String> {
  let mut complete = false;
  let mut valid = false;
  for line in output.lines().filter(|line| !line.trim().is_empty()) {
    let event: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    match event["tagName"].as_str() {
      Some("authIdentity") => {
        let data = &event["data"];
        valid |= data["userId"].as_str().is_some_and(|id| !id.is_empty() && identity.is_none_or(|selected| selected == id)) && data["expires"].as_u64().is_some_and(|expires| expires > now_ms);
      }
      Some("complete") => {
        if event["data"]["status"].as_i64() != Some(0) {
          return Err("Login state check failed".into());
        }
        complete = true;
      }
      _ => {}
    }
  }
  if !complete {
    return Err("Login state check did not complete".into());
  }
  Ok(valid)
}

pub fn update_repository_identity(root: &Path, identity: &str) -> Result<(), String> {
  use std::io::Write;
  let path = root.join(".lore/config.toml");
  let original = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
  let mut document = original.parse::<toml_edit::DocumentMut>().map_err(|e| e.to_string())?;
  if document.get("identity").and_then(|item| item.as_str()) == Some(identity) {
    return Ok(());
  }
  let mut value = toml_edit::Value::from(identity);
  if let Some(old) = document.get("identity").and_then(|item| item.as_value()) {
    *value.decor_mut() = old.decor().clone();
  }
  document["identity"] = toml_edit::Item::Value(value);
  let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(|e| e.to_string())?;
  temporary.write_all(document.to_string().as_bytes()).map_err(|e| e.to_string())?;
  temporary.as_file().sync_all().map_err(|e| e.to_string())?;
  if fs::read_to_string(&path).map_err(|e| e.to_string())? != original {
    return Err("Repository config changed while updating identity; retry login.".into());
  }
  temporary.persist(&path).map_err(|e| e.to_string())?;
  Ok(())
}

pub fn repository_remote_url(root: &Path) -> Option<String> {
  for metadata in [".lore", ".urc"] {
    let path = root.join(metadata).join("config.toml");
    let Ok(text) = fs::read_to_string(path) else {
      continue;
    };
    let Ok(config) = text.parse::<toml_edit::DocumentMut>() else {
      continue;
    };
    if let Some(url) = config.get("remote_url").and_then(|item| item.as_str()) {
      let url = url.trim();
      if !url.is_empty() {
        return Some(url.to_owned());
      }
    }
  }
  None
}

#[derive(Clone, Debug)]
pub struct Entry {
  pub path: PathBuf,
  pub directory: bool,
  pub size: u64,
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn startup_login_checks_identity_expiration_and_completion() {
    let output = concat!(
      "{\"tagName\":\"authIdentity\",\"data\":{\"userId\":\"alice\",\"expires\":2000}}\n",
      "{\"tagName\":\"complete\",\"data\":{\"status\":0}}"
    );
    assert!(has_valid_login(output, Some("alice"), 1000).unwrap());
    assert!(!has_valid_login(output, Some("bob"), 1000).unwrap());
    assert!(!has_valid_login(output, None, 2000).unwrap());
    assert!(!has_valid_login(r#"{"tagName":"complete","data":{"status":0}}"#, None, 0).unwrap());
    assert!(has_valid_login("", None, 0).is_err());
    assert!(has_valid_login(r#"{"tagName":"complete","data":{"status":45}}"#, None, 0).is_err());
  }

  #[test]
  fn repository_detection_requires_metadata_directory() {
    let root = tempfile::tempdir().unwrap();
    assert!(!is_repository(root.path()));
    for marker in [".lore", ".urc"] {
      let checkout = root.path().join(marker.trim_start_matches('.'));
      fs::create_dir(&checkout).unwrap();
      fs::create_dir(checkout.join(marker)).unwrap();
      assert!(is_repository(&checkout));
    }
    fs::write(root.path().join(".lore"), "not a repository").unwrap();
    assert!(!is_repository(root.path()));
  }

  #[test]
  fn move_preserves_contents_and_rejects_overwrite_and_self_nesting() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("한글 file.txt");
    let destination = root.path().join("renamed.txt");
    fs::write(&source, "content").unwrap();
    move_entry(root.path(), &source, &destination).unwrap();
    assert!(!source.exists());
    assert_eq!(fs::read_to_string(&destination).unwrap(), "content");
    fs::write(&source, "keep").unwrap();
    assert!(move_entry(root.path(), &source, &destination).is_err());
    assert_eq!(fs::read_to_string(&source).unwrap(), "keep");
    let folder = root.path().join("folder");
    fs::create_dir(&folder).unwrap();
    assert!(move_entry(root.path(), &folder, &folder.join("nested")).is_err());
    assert!(move_entry(root.path(), root.path(), &folder.join("root")).is_err());
  }
  #[test]
  fn clone_rejects_occupied_and_relative_destinations_before_launch() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("keep.txt"), "keep").unwrap();
    let cli = Path::new("does-not-exist");
    assert!(clone_repository(cli, "lores://example/repo", root.path(), None).unwrap_err().contains("empty folder"));
    assert!(clone_repository(cli, "lores://example/repo", Path::new("relative"), None).unwrap_err().contains("absolute"));
    assert_eq!(fs::read_to_string(root.path().join("keep.txt")).unwrap(), "keep");
  }
  #[test]
  fn create_rejects_invalid_inputs_without_touching_files() {
    let root = tempfile::tempdir().unwrap();
    let cli = Path::new("does-not-exist");
    fs::write(root.path().join("keep.txt"), "keep").unwrap();
    for (url, path, expected) in [
      ("invalid", root.path(), "URL"),
      ("lores://example/repo", Path::new("relative"), "absolute"),
      ("lores://example/repo", root.path(), "empty folder"),
    ] {
      assert!(create_repository(cli, url, path, None).unwrap_err().contains(expected));
    }
    assert_eq!(fs::read_to_string(root.path().join("keep.txt")).unwrap(), "keep");
    let missing = root.path().join("missing").join("repo");
    assert!(create_repository(cli, "lores://example/repo", &missing, None).unwrap_err().contains("parent folder"));
    assert!(!missing.exists());
  }
  #[test]
  fn repository_identity_preserves_other_config_and_rejects_invalid_toml() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".lore")).unwrap();
    let path = root.path().join(".lore/config.toml");
    fs::write(&path, "# repository\nidentity = \"old\" # account\n[remote]\nurl = \"lores://example\"\n").unwrap();
    update_repository_identity(root.path(), "new-user-id").unwrap();
    let updated = fs::read_to_string(&path).unwrap();
    assert!(updated.contains("identity = \"new-user-id\" # account"));
    assert!(updated.contains("# repository"));
    assert!(updated.contains("url = \"lores://example\""));
    update_repository_identity(root.path(), "new-user-id").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), updated);
    fs::write(&path, "invalid = [").unwrap();
    assert!(update_repository_identity(root.path(), "other").is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "invalid = [");
  }
  #[test]
  fn account_uses_returned_id_and_name_and_requires_success() {
    let info = "{\"tagName\":\"authUserInfo\",\"data\":{\"id\":\"uuid-123\",\"name\":\"law80\"}}";
    assert_eq!(
      parse_account(&format!("{info}\n{{\"tagName\":\"complete\",\"data\":{{\"status\":0}}}}")),
      Ok(("uuid-123".into(), "law80".into()))
    );
    assert!(parse_account(info).is_err());
    assert!(parse_account(&format!("{info}\n{{\"tagName\":\"complete\",\"data\":{{\"status\":1}}}}")).is_err());
  }
  #[test]
  fn lock_status_requires_success_and_explicit_count() {
    for (count, locked) in [(0, false), (1, true)] {
      let output = format!("{{\"tagName\":\"lockFileStatusBegin\",\"data\":{{\"count\":{count}}}}}\n{{\"tagName\":\"complete\",\"data\":{{\"status\":0}}}}");
      assert_eq!(parse_lock_status(&output), Ok(locked));
    }
    assert!(parse_lock_status("").is_err());
    assert!(parse_lock_status("{\"tagName\":\"complete\",\"data\":{\"status\":0}}").is_err());
    assert!(parse_lock_status("{\"tagName\":\"lockFileStatusBegin\",\"data\":{\"count\":0}}\n{\"tagName\":\"complete\",\"data\":{\"status\":1}}").is_err());
  }
  #[test]
  #[ignore = "Requires a built Lore CLI; creates an isolated repository under target"]
  fn real_lore_local_workflow() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("target")
      .join(format!("smoke-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&root).unwrap();
    let cli = find_cli();
    let invoke = |args: &[&str], json| run_as(&cli, &root, &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(), json, None).unwrap();
    invoke(&["repository", "create", "--offline", "lorelens-test"], false);
    fs::write(root.join("한글 sample.txt"), "first line\n").unwrap();
    let status = parse_status(&invoke(&["status", "--scan"], true)).unwrap();
    assert_eq!(status.changes[0].path, "한글 sample.txt");
    assert!(!status.changes[0].staged);
    invoke(&["stage", "--", "한글 sample.txt"], false);
    assert!(parse_status(&invoke(&["status"], true)).unwrap().changes[0].staged);
    invoke(&["unstage", "--", "한글 sample.txt"], false);
    assert!(!parse_status(&invoke(&["status", "--scan"], true)).unwrap().changes[0].staged);
    invoke(&["stage", "--", "한글 sample.txt"], false);
    invoke(&["commit", "--", "한글 commit message"], false);
    assert!(parse_status(&invoke(&["status", "--scan"], true)).unwrap().changes.is_empty());
    assert!(invoke(&["history", "50", "--oneline"], false).contains("한글 commit message"));
    assert!(invoke(&["branch", "list"], false).contains("main"));
    assert!(invoke(&["file", "history", "--", "한글 sample.txt", "50"], false).contains("한글 commit message"));
    fs::write(root.join("한글 sample.txt"), "second line\n").unwrap();
    assert!(invoke(&["diff", "--", "한글 sample.txt"], false).contains("second line"));
    let state = parse_status(&invoke(&["status", "--scan"], true)).unwrap();
    let revision = format!("{}@{}", state.branch, state.revision);
    let exported = root.join("base snapshot.txt");
    invoke(&["file", "write", "--path", "한글 sample.txt", "--revision", &revision, "--output", exported.to_str().unwrap()], false);
    assert_eq!(fs::read_to_string(exported).unwrap(), "first line\n");
  }
  #[test]
  fn reads_lore_tagged_events_and_flags() {
    let input = concat!(
      "{\"tagName\":\"repositoryStatusRevision\",\"data\":{\"branchName\":\"main\",\"revisionNumber\":12}}\n",
      "{\"tagName\":\"repositoryStatusFile\",\"data\":{\"path\":\"한글/a b.rs\",\"action\":\"modify\",\"flagStaged\":true,\"flagConflictUnresolved\":true,\"size\":42}}\n",
      "{\"tagName\":\"complete\",\"data\":{\"status\":0}}"
    );
    let status = parse_status(input).unwrap();
    assert_eq!(status.branch, "main");
    assert_eq!(status.changes[0].path, "한글/a b.rs");
    assert!(status.changes[0].staged && status.changes[0].conflict);
  }
  #[test]
  fn rejects_missing_repository_malformed_json_and_failed_completion() {
    assert!(parse_status("").is_err());
    assert!(parse_status("not json").is_err());
    assert!(parse_status("{\"tagName\":\"complete\",\"data\":{\"status\":1}}").is_err());
  }
  #[test]
  fn rejects_preview_outside_repository() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(preview(&root, &root.join("../Cargo.toml")).is_err());
    assert!(list_directory(&root, &root.join("..")).is_err());
  }

  #[test]
  fn directory_listing_applies_loreignore_and_reloads_it() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    for folder in ["cache", "src", "src/cache"] {
      fs::create_dir_all(root.join(folder)).unwrap();
    }
    for file in ["hidden.LOG", "keep.log", "root.txt", "src/root.txt", "src/a.log", "src/visible.rs"] {
      fs::write(root.join(file), "").unwrap();
    }
    fs::write(root.join(".loreignore"), "# comment\n*.log\n!keep.log\ncache/\n/root.txt\n").unwrap();
    let names = |directory: &Path| {
      list_directory(root, directory)
        .unwrap()
        .into_iter()
        .map(|entry| entry.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
    };
    assert_eq!(names(root), vec!["src", ".loreignore", "keep.log"]);
    assert_eq!(names(&root.join("src")), vec!["root.txt", "visible.rs"]);
    fs::write(root.join(".loreignore"), "src/**\n!src/visible.rs\n").unwrap();
    assert_eq!(names(&root.join("src")), vec!["visible.rs"]);
    fs::remove_file(root.join(".loreignore")).unwrap();
    assert!(names(root).contains(&"hidden.LOG".to_owned()));
  }
}
