use super::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct RemoteHistory {
  pub branch: String,
  pub commits: Vec<RemoteCommit>,
}

#[derive(Clone, Debug, Default)]
pub struct RemoteCommit {
  pub hash: String,
  pub number: u64,
  pub message: String,
  pub author: String,
  pub timestamp: Option<i64>,
  pub parents: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevisionFile {
  pub path: String,
  pub from_path: String,
  pub action: String,
}

impl RevisionFile {
  pub fn is_modified(&self) -> bool {
    self.from_path.is_empty() && matches!(self.action.to_ascii_lowercase().as_str(), "keep" | "modify" | "edit")
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevisionComparison {
  pub path: String,
  pub source: String,
  pub target: String,
}

impl RevisionComparison {
  pub fn modified_file(commit: &RemoteCommit, file: &RevisionFile) -> Result<Self, String> {
    if !file.is_modified() {
      return Err("Select a modified (M) file.".into());
    }
    let comparison = Self {
      path: file.path.clone(),
      source: commit.parents.first().ok_or("This commit has no parent to compare.")?.clone(),
      target: commit.hash.clone(),
    };
    comparison.validate()?;
    Ok(comparison)
  }

  fn validate(&self) -> Result<(), String> {
    if !valid_revision(&self.source) || !valid_revision(&self.target) || self.source == self.target {
      return Err("Invalid comparison revisions".into());
    }
    let path = self.path.replace('\\', "/");
    if path.is_empty() || path.contains(['\0', '\n', '\r', '\t', ':']) || path.split('/').any(|part| matches!(part, "" | "." | ".." | ".lore" | ".urc")) {
      return Err("Invalid comparison file path".into());
    }
    Ok(())
  }

  fn diff_args(&self) -> Result<Vec<String>, String> {
    self.validate()?;
    Ok(vec![
      "diff".into(),
      "--source".into(),
      self.source.clone(),
      "--target".into(),
      self.target.clone(),
      "--".into(),
      self.path.clone(),
    ])
  }
}

pub fn revision_patch(cli: &Path, root: &Path, comparison: &RevisionComparison, identity: Option<&str>) -> Result<String, String> {
  let output = run_as(cli, root, &comparison.diff_args()?, true, identity)?;
  parse_revision_patch(&output, &comparison.path)
}

fn parse_revision_patch(output: &str, path: &str) -> Result<String, String> {
  let events = history_events(output)?;
  let patches = events.iter().filter(|event| event["tagName"] == "fileDiff").collect::<Vec<_>>();
  if patches.is_empty() {
    return Err("No text changes to save as a patch.".into());
  }
  if patches.len() != 1 || patches[0]["data"]["path"].as_str().map(|path| path.replace('\\', "/")) != Some(path.replace('\\', "/")) {
    return Err("Diff output does not match the selected file.".into());
  }
  let patch = patches[0]["data"]["patch"].as_str().ok_or("Missing patch content")?;
  if patch.trim() == "Binary files differ" {
    return Err("Binary files cannot be saved as a text patch. Use Diff instead.".into());
  }
  let mut sections = patch.splitn(3, '\n');
  let old = sections.next().unwrap_or_default();
  let new = sections.next().unwrap_or_default();
  let body = sections.next().unwrap_or_default();
  if !old.starts_with("--- ") || !new.starts_with("+++ ") || !body.starts_with("@@ ") {
    return Err("No text changes to save as a patch.".into());
  }
  // Lore's display labels include revision annotations. Use standard paths so
  // the saved patch can be applied from the repository root with -p1.
  let label = |prefix: &str| {
    let label = format!("{prefix}/{}", path.replace('\\', "/"));
    if label.contains([' ', '"', '\\']) { serde_json::to_string(&label).unwrap() } else { label }
  };
  Ok(format!("--- {}\n+++ {}\n{body}", label("a"), label("b")))
}

pub fn save_revision_patch(path: &Path, patch: &str) -> Result<(), String> {
  use std::io::Write;
  let parent = path.parent().filter(|parent| parent.is_dir()).ok_or("Patch destination folder is unavailable.")?;
  let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
  temporary.write_all(patch.as_bytes()).map_err(|error| error.to_string())?;
  temporary.as_file().sync_all().map_err(|error| error.to_string())?;
  temporary.persist(path).map_err(|error| error.to_string())?;
  Ok(())
}

fn valid_revision(hash: &str) -> bool {
  hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) && hash.bytes().any(|byte| byte != b'0')
}

fn history_events(output: &str) -> Result<Vec<Value>, String> {
  let events = output
    .lines()
    .filter(|line| !line.trim().is_empty())
    .map(serde_json::from_str::<Value>)
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| error.to_string())?;
  let completions = events.iter().filter(|event| event["tagName"] == "complete").collect::<Vec<_>>();
  if completions.is_empty() || completions.iter().any(|event| event["data"]["status"].as_i64() != Some(0)) {
    return Err("History query did not complete successfully.".into());
  }
  Ok(events)
}

fn metadata_value(value: &Value) -> &Value {
  value.get("data").or_else(|| value.get("value")).unwrap_or(value)
}

fn machine_identity(value: &str) -> bool {
  let compact = value.bytes().filter(|byte| *byte != b'-').collect::<Vec<_>>();
  matches!(compact.len(), 32 | 64) && compact.iter().all(u8::is_ascii_hexdigit)
}

fn parse_remote_commits(output: &str) -> Result<Vec<RemoteCommit>, String> {
  let events = history_events(output)?;
  let authors = events
    .iter()
    .filter(|event| event["tagName"] == "authUserInfo")
    .filter_map(|event| {
      let id = event["data"]["id"].as_str()?.trim();
      let name = event["data"]["name"].as_str()?.trim();
      (!id.is_empty() && !name.is_empty() && id != name).then(|| (id.to_owned(), name.to_owned()))
    })
    .collect::<HashMap<_, _>>();
  let mut commits: Vec<RemoteCommit> = Vec::new();
  for event in events {
    let data = &event["data"];
    match event["tagName"].as_str() {
      Some("revisionHistoryEntry") => {
        let hash = data["revision"].as_str().filter(|hash| valid_revision(hash)).ok_or("Invalid revision hash")?;
        commits.push(RemoteCommit {
          hash: hash.into(),
          number: data["revisionNumber"].as_u64().ok_or("Missing revision number")?,
          parents: data["parent"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|hash| valid_revision(hash))
            .map(str::to_owned)
            .collect(),
          ..Default::default()
        });
      }
      Some("metadata") => {
        if let Some(commit) = commits.last_mut() {
          let value = metadata_value(&data["value"]);
          match data["key"].as_str() {
            Some("message") => commit.message = value.as_str().unwrap_or_default().into(),
            Some("created-by") => commit.author = value.as_str().unwrap_or_default().into(),
            Some("committed-by") if commit.author.is_empty() => commit.author = value.as_str().unwrap_or_default().into(),
            Some("timestamp") => commit.timestamp = value.as_i64(),
            _ => {}
          }
        }
      }
      _ => {}
    }
  }
  for commit in &mut commits {
    if let Some(author) = authors.get(&commit.author) {
      commit.author.clone_from(author);
    } else if machine_identity(&commit.author) {
      commit.author.clear();
    }
  }
  let mut seen = HashSet::new();
  commits.retain(|commit| seen.insert(commit.hash.clone()));
  Ok(commits)
}

pub fn remote_branch_history(cli: &Path, root: &Path, branch: &str, limit: usize, identity: Option<&str>) -> Result<RemoteHistory, String> {
  let args = vec!["--remote".into(), "history".into(), limit.clamp(1, 10000).to_string(), "--branch".into(), branch.into()];
  Ok(RemoteHistory {
    branch: branch.into(),
    commits: parse_remote_commits(&run_as(cli, root, &args, true, identity)?)?,
  })
}

pub fn revision_files(cli: &Path, root: &Path, revision: &str, identity: Option<&str>) -> Result<Vec<RevisionFile>, String> {
  if !valid_revision(revision) {
    return Err("Invalid revision hash".into());
  }
  let args = vec!["revision".into(), "info".into(), "--delta".into(), "--".into(), revision.into()];
  parse_revision_files(&run_as(cli, root, &args, true, identity)?, revision)
}

fn parse_revision_files(output: &str, revision: &str) -> Result<Vec<RevisionFile>, String> {
  let events = history_events(output)?;
  if !events.iter().any(|event| event["tagName"] == "revisionInfo" && event["data"]["revision"] == revision) {
    return Err("Revision details do not match the selected commit.".into());
  }
  let mut files = Vec::new();
  for event in events.iter().filter(|event| event["tagName"] == "revisionInfoDelta") {
    let data = &event["data"];
    if data["flagFile"] == false || data["flagFile"] == 0 {
      continue;
    }
    files.push(RevisionFile {
      path: data["path"].as_str().ok_or("Missing changed file path")?.into(),
      from_path: data["fromPath"].as_str().unwrap_or_default().into(),
      action: data["action"].as_str().unwrap_or("modify").into(),
    });
  }
  files.sort_by(|left, right| left.path.cmp(&right.path));
  Ok(files)
}

// Resolve the remote tip from status rather than the local branch head, which
// can include commits that have not been pushed yet.
fn remote_history_target(output: &str) -> Result<(String, Option<String>), String> {
  let events = output
    .lines()
    .filter(|line| !line.trim().is_empty())
    .map(serde_json::from_str::<Value>)
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| error.to_string())?;
  if !events.iter().any(|event| event["tagName"] == "complete" && event["data"]["status"].as_i64() == Some(0))
    || events.iter().any(|event| event["tagName"] == "complete" && event["data"]["status"].as_i64() != Some(0))
  {
    return Err("Incomplete repository status".into());
  }
  let data = &events.iter().find(|event| event["tagName"] == "repositoryStatusRevision").ok_or("Missing repository status")?["data"];
  let branch = data["branchName"].as_str().filter(|name| !name.is_empty()).ok_or("Missing branch name")?.to_owned();
  if data["remoteAuthorized"] != true && data["remoteAuthorized"] != 1 {
    return Err("Remote status unavailable; check authentication.".into());
  }
  if data["remoteBranchExist"] == false || data["remoteBranchExist"] == 0 {
    return Ok((branch, None));
  }
  if data["remoteBranchExist"] != true && data["remoteBranchExist"] != 1 {
    return Err("Remote branch status unavailable.".into());
  }
  let revision = data["revisionRemote"]
    .as_str()
    .filter(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) && hash.bytes().any(|byte| byte != b'0'))
    .ok_or("Missing remote revision")?;
  Ok((branch, Some(revision.into())))
}

pub fn remote_history(cli: &Path, root: &Path, status: &str, limit: usize, identity: Option<&str>) -> Result<RemoteHistory, String> {
  let (branch, revision) = remote_history_target(status)?;
  let commits = if let Some(revision) = revision {
    let args = vec!["history".into(), limit.clamp(1, 10000).to_string(), "--revision".into(), revision];
    parse_remote_commits(&run_as(cli, root, &args, true, identity)?)?
  } else {
    Vec::new()
  };
  Ok(RemoteHistory { branch, commits })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRevision {
  pub hash: String,
  pub number: u64,
  pub action: String,
  pub message: String,
}

pub fn file_history(cli: &Path, root: &Path, path: &str, limit: usize, identity: Option<&str>) -> Result<Vec<FileRevision>, String> {
  let args = vec!["file".into(), "history".into(), "--".into(), path.into(), limit.to_string()];
  parse_file_history(&run_as(cli, root, &args, true, identity)?)
}

fn parse_file_history(output: &str) -> Result<Vec<FileRevision>, String> {
  let mut revisions: Vec<FileRevision> = Vec::new();
  let mut complete = false;
  for line in output.lines().filter(|line| !line.trim().is_empty()) {
    let event: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
    let data = &event["data"];
    match event["tagName"].as_str() {
      Some("fileHistory") => {
        let hash = data["revision"]
          .as_str()
          .filter(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
          .ok_or("Invalid file revision hash")?;
        revisions.push(FileRevision {
          hash: hash.into(),
          number: data["revisionNumber"].as_u64().ok_or("Missing file revision number")?,
          action: data["action"].as_str().unwrap_or_default().into(),
          message: String::new(),
        });
      }
      Some("metadata") if data["key"] == "message" => {
        if let Some(revision) = revisions.last_mut() {
          let value = &data["value"];
          revision.message = value.as_str().or_else(|| value["data"].as_str()).or_else(|| value["value"].as_str()).unwrap_or_default().into();
        }
      }
      Some("complete") => {
        if data["status"].as_i64() != Some(0) {
          return Err("File history query failed".into());
        }
        complete = true;
      }
      _ => {}
    }
  }
  if !complete {
    return Err("Incomplete file history".into());
  }
  // Preserve Lore's newest-first traversal order, including branch ancestry.
  let mut seen = HashSet::new();
  revisions.retain(|revision| seen.insert(revision.hash.clone()));
  Ok(revisions)
}

#[cfg(test)]
fn revision_diff_args(path: &str, older: &FileRevision, newer: &FileRevision) -> Result<Vec<String>, String> {
  if older.hash == newer.hash {
    return Err("Select two different revisions.".into());
  }
  Ok(vec![
    "diff".into(),
    "--source".into(),
    older.hash.clone(),
    "--target".into(),
    newer.hash.clone(),
    "--".into(),
    path.into(),
  ])
}

#[cfg(test)]
fn revision_diff(cli: &Path, root: &Path, path: &str, older: &FileRevision, newer: &FileRevision, identity: Option<&str>) -> Result<String, String> {
  run_as(cli, root, &revision_diff_args(path, older, newer)?, false, identity)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn modified_file_comparison_pins_first_parent_and_rejects_other_actions() {
    let commit = RemoteCommit {
      hash: "c".repeat(64),
      parents: vec!["a".repeat(64), "b".repeat(64)],
      ..Default::default()
    };
    let file = RevisionFile {
      path: "--한글 file.txt".into(),
      from_path: String::new(),
      action: "keep".into(),
    };
    let comparison = RevisionComparison::modified_file(&commit, &file).unwrap();
    assert_eq!(
      comparison.diff_args().unwrap(),
      ["diff", "--source", &"a".repeat(64), "--target", &"c".repeat(64), "--", "--한글 file.txt"]
    );
    for action in ["add", "delete", "move", "copy", "unknown"] {
      assert!(
        RevisionComparison::modified_file(
          &commit,
          &RevisionFile {
            action: action.into(),
            ..file.clone()
          }
        )
        .is_err()
      );
    }
    assert!(RevisionComparison::modified_file(&RemoteCommit { parents: vec![], ..commit.clone() }, &file).is_err());
    for path in ["../outside", "/absolute", "C:\\file", ".lore/config.toml", "bad\nfile"] {
      assert!(RevisionComparison::modified_file(&commit, &RevisionFile { path: path.into(), ..file.clone() }).is_err());
    }
    assert!(RevisionComparison::modified_file(&commit, &RevisionFile { from_path: "old.txt".into(), ..file }).is_err());
  }

  fn patch_output(path: &str, patch: &str) -> String {
    format!(
      "{}\n{}",
      serde_json::json!({"tagName":"fileDiff","data":{"path":path,"patch":patch}}),
      serde_json::json!({"tagName":"complete","data":{"status":0}})
    )
  }

  #[test]
  fn patch_export_preserves_hunks_and_rejects_binary_mismatched_or_failed_output() {
    let body = "@@ -1,2 +1,2 @@\n \n-old\n+new\n\\ No newline at end of file\n";
    let output = patch_output("dir/한글 file.txt", &format!("--- old label\n+++ new label\n{body}"));
    let patch = parse_revision_patch(&output, "dir/한글 file.txt").unwrap();
    assert_eq!(patch, format!("--- \"a/dir/한글 file.txt\"\n+++ \"b/dir/한글 file.txt\"\n{body}"));
    assert!(parse_revision_patch(&output, "other.txt").is_err());
    assert!(parse_revision_patch(&output.replace("\"status\":0", "\"status\":1"), "dir/한글 file.txt").is_err());
    assert!(parse_revision_patch(output.lines().next().unwrap(), "dir/한글 file.txt").is_err());
    for content in ["Binary files differ\n", "", "--- old\n+++ new\n"] {
      assert!(parse_revision_patch(&patch_output("file", content), "file").is_err());
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("change.patch");
    save_revision_patch(&path, &patch).unwrap();
    assert_eq!(fs::read_to_string(path).unwrap(), patch);
  }

  #[test]
  #[ignore = "Requires a usable Lore CLI and Git; creates isolated offline repositories"]
  fn real_commit_patch_applies_without_including_working_changes() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let cli = find_cli();
    let invoke = |args: &[&str], json| run_as(&cli, root, &args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>(), json, None).unwrap();
    invoke(&["repository", "create", "--offline", "patch-test"], false);
    let path = "한글 file.txt";
    let mut hashes = Vec::new();
    for content in ["first\n\nlast\n", "second\n\nlast\n"] {
      fs::write(root.join(path), content).unwrap();
      invoke(&["stage", "--", path], false);
      invoke(&["commit", "--", "Patch test"], false);
      let status = invoke(&["status"], true);
      let events = history_events(&status).unwrap();
      hashes.push(
        events.iter().find(|event| event["tagName"] == "repositoryStatusRevision").unwrap()["data"]["revisionLocal"]
          .as_str()
          .unwrap()
          .to_owned(),
      );
    }
    fs::write(root.join(path), "uncommitted content\n").unwrap();
    let comparison = RevisionComparison {
      path: path.into(),
      source: hashes[0].clone(),
      target: hashes[1].clone(),
    };
    let patch = revision_patch(&cli, root, &comparison, None).unwrap();
    assert!(!patch.contains("uncommitted"));
    let destination = root.join("change.patch");
    save_revision_patch(&destination, &patch).unwrap();
    let apply_root = tempfile::tempdir().unwrap();
    fs::write(apply_root.path().join(path), "first\n\nlast\n").unwrap();
    let output = Command::new("git")
      .current_dir(apply_root.path())
      .args(["-c", "core.autocrlf=false", "apply"])
      .arg(&destination)
      .output()
      .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read_to_string(apply_root.path().join(path)).unwrap(), "second\n\nlast\n");
    assert_eq!(fs::read_to_string(root.join(path)).unwrap(), "uncommitted content\n");
  }

  #[test]
  fn remote_commits_keep_authorship_dates_and_merge_parents() {
    let hash = "c".repeat(64);
    let author_id = "138ea5e6-c1c3-4465-9dd7-778a8cf433e4";
    let output = [
      serde_json::json!({"tagName":"revisionHistoryEntry","data":{"revision":hash,"revisionNumber":7,"parent":["a".repeat(64),"b".repeat(64)]}}),
      serde_json::json!({"tagName":"metadata","data":{"key":"message","value":{"data":"Merge\nDetails"}}}),
      serde_json::json!({"tagName":"metadata","data":{"key":"committed-by","value":{"value":"committer"}}}),
      serde_json::json!({"tagName":"metadata","data":{"key":"created-by","value":author_id}}),
      serde_json::json!({"tagName":"metadata","data":{"key":"timestamp","value":{"tagName":"numeric","data":1700000000000i64}}}),
      serde_json::json!({"tagName":"complete","data":{"status":0}}),
      serde_json::json!({"tagName":"authUserInfo","data":{"id":author_id,"name":"law80@zenogrid.co.kr"}}),
      serde_json::json!({"tagName":"complete","data":{"status":0}}),
    ]
    .iter()
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join("\n");
    let commits = parse_remote_commits(&output).unwrap();
    assert_eq!(commits[0].author, "law80@zenogrid.co.kr");
    assert_eq!(commits[0].message, "Merge\nDetails");
    assert_eq!(commits[0].timestamp, Some(1700000000000));
    assert_eq!(commits[0].parents.len(), 2);
    let without_user = output.lines().filter(|line| !line.contains("authUserInfo")).collect::<Vec<_>>().join("\n");
    assert!(parse_remote_commits(&without_user).unwrap()[0].author.is_empty());
    assert!(parse_remote_commits(&output.replace(&hash, "bad hash")).is_err());
    assert!(parse_remote_commits(output.lines().next().unwrap()).is_err());
  }

  #[test]
  fn revision_files_validate_selection_and_preserve_renames() {
    let hash = "a".repeat(64);
    let output = [
      serde_json::json!({"tagName":"revisionInfo","data":{"revision":hash}}),
      serde_json::json!({"tagName":"revisionInfoDelta","data":{"path":"src","flagFile":false,"action":"modify"}}),
      serde_json::json!({"tagName":"revisionInfoDelta","data":{"path":"src/새 이름.rs","fromPath":"old.rs","flagFile":true,"action":"move"}}),
      serde_json::json!({"tagName":"complete","data":{"status":0}}),
    ]
    .iter()
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join("\n");
    assert_eq!(
      parse_revision_files(&output, &hash).unwrap(),
      vec![RevisionFile {
        path: "src/새 이름.rs".into(),
        from_path: "old.rs".into(),
        action: "move".into()
      }]
    );
    assert!(parse_revision_files(&output, &"b".repeat(64)).is_err());
    assert!(parse_revision_files(&output.replace("\"status\":0", "\"status\":1"), &hash).is_err());
    assert!(revision_files(Path::new("missing-cli"), Path::new("."), "--malformed", None).is_err());
  }

  fn remote_status(data: Value) -> String {
    format!(
      "{}\n{}",
      serde_json::json!({"tagName": "repositoryStatusRevision", "data": data}),
      serde_json::json!({"tagName": "complete", "data": {"status": 0}})
    )
  }

  #[test]
  fn remote_history_starts_at_remote_tip_even_when_local_is_ahead_or_diverged() {
    let remote = "b".repeat(64);
    for local in ["a".repeat(64), remote.clone()] {
      for flag in [serde_json::json!(true), serde_json::json!(1)] {
        let output = remote_status(serde_json::json!({
          "branchName": "한글 branch", "remoteAuthorized": flag, "remoteBranchExist": flag,
          "revisionLocal": local, "revisionRemote": remote
        }));
        assert_eq!(remote_history_target(&output).unwrap(), ("한글 branch".into(), Some(remote.clone())));
      }
    }
  }

  #[test]
  fn remote_history_distinguishes_unpushed_branches_from_unavailable_status() {
    let output = remote_status(serde_json::json!({"branchName": "new", "remoteAuthorized": true, "remoteBranchExist": false}));
    assert!(remote_history(Path::new("missing-cli"), Path::new("."), &output, 100, None).unwrap().commits.is_empty());
    for data in [
      serde_json::json!({"branchName": "main", "remoteAuthorized": false}),
      serde_json::json!({"branchName": "main", "remoteAuthorized": true}),
      serde_json::json!({"branchName": "main", "remoteAuthorized": true, "remoteBranchExist": true}),
      serde_json::json!({"branchName": "main", "remoteAuthorized": true, "remoteBranchExist": true, "revisionRemote": "0".repeat(64)}),
    ] {
      assert!(remote_history_target(&remote_status(data)).is_err());
    }
    assert!(remote_history_target("").is_err());
    assert!(remote_history_target("invalid json").is_err());
    assert!(remote_history_target(output.lines().next().unwrap()).is_err());
    assert!(remote_history_target(&output.replace("\"status\":0", "\"status\":1")).is_err());
  }

  #[test]
  fn revision_history_preserves_order_and_reads_typed_messages() {
    let output = [
      serde_json::json!({"tagName": "revisionHistoryEntry", "data": {"revision": "b".repeat(64), "revisionNumber": 4}}),
      serde_json::json!({"tagName": "metadata", "data": {"key": "message", "value": {"tagName": "string", "data": "원격 커밋"}}}),
      serde_json::json!({"tagName": "revisionHistoryEntry", "data": {"revision": "a".repeat(64), "revisionNumber": 3}}),
      serde_json::json!({"tagName": "metadata", "data": {"key": "message", "value": "previous"}}),
      serde_json::json!({"tagName": "complete", "data": {"status": 0}}),
    ]
    .iter()
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join("\n");
    let commits = parse_revision_history(&output).unwrap();
    assert_eq!(commits.iter().map(|commit| commit.number.as_str()).collect::<Vec<_>>(), ["4", "3"]);
    assert_eq!(commits[0].message, "원격 커밋");
    assert_eq!(commits[1].message, "previous");
    assert!(parse_revision_history(&output.replace("\"status\":0", "\"status\":1")).is_err());
  }

  #[test]
  fn parses_file_revisions_and_typed_messages() {
    let hash = "a".repeat(64);
    let output = format!(
      "{}\n{}\n{}",
      serde_json::json!({"tagName":"fileHistory","data":{"revision":hash,"revisionNumber":3,"action":"edit"}}),
      serde_json::json!({"tagName":"metadata","data":{"key":"message","value":{"tagName":"string","data":"한글 message"}}}),
      serde_json::json!({"tagName":"complete","data":{"status":0}})
    );
    let revisions = parse_file_history(&output).unwrap();
    assert_eq!(
      revisions,
      vec![FileRevision {
        hash,
        number: 3,
        action: "edit".into(),
        message: "한글 message".into()
      }]
    );
    assert!(parse_file_history("").is_err());
    assert!(parse_file_history("not json").is_err());
    assert!(parse_file_history(r#"{"tagName":"complete","data":{"status":1}}"#).is_err());
    assert!(parse_file_history(r#"{"tagName":"complete","data":{"status":0}}"#).unwrap().is_empty());
    assert!(parse_file_history(&output.replace(&"a".repeat(64), "invalid")).is_err());
  }

  #[test]
  fn diff_arguments_pin_both_revisions_and_protect_paths() {
    let older = FileRevision {
      hash: "a".repeat(64),
      number: 1,
      action: "add".into(),
      message: String::new(),
    };
    let newer = FileRevision {
      hash: "b".repeat(64),
      number: 3,
      ..older.clone()
    };
    let args = revision_diff_args("--한글 file.txt", &older, &newer).unwrap();
    assert_eq!(args, ["diff", "--source", &older.hash, "--target", &newer.hash, "--", "--한글 file.txt"]);
    assert!(revision_diff_args("file", &older, &older).is_err());
  }

  #[test]
  #[ignore = "Requires a usable Lore CLI; creates an isolated offline repository"]
  fn real_file_history_and_selected_revision_diff() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let cli = find_cli();
    let invoke = |args: &[&str]| run_as(&cli, root, &args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>(), false, None).unwrap();
    invoke(&["repository", "create", "--offline", "file-history-test"]);
    let path = "한글 sample.txt";
    for (content, message) in [("first line\n", "First version"), ("second line\n", "Second version")] {
      fs::write(root.join(path), content).unwrap();
      invoke(&["stage", "--", path]);
      invoke(&["commit", "--", message]);
    }
    fs::write(root.join("unrelated.txt"), "other").unwrap();
    invoke(&["stage", "--", "unrelated.txt"]);
    invoke(&["commit", "--", "Unrelated commit"]);
    let revisions = file_history(&cli, root, path, 100, None).unwrap();
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].message, "Second version");
    assert_eq!(revisions[1].message, "First version");
    fs::write(root.join(path), "uncommitted local content\n").unwrap();
    let diff = revision_diff(&cli, root, path, &revisions[1], &revisions[0], None).unwrap();
    assert!(diff.contains("-first line") && diff.contains("+second line"), "{diff}");
    assert!(!diff.contains("uncommitted local content"));
    assert_eq!(fs::read_to_string(root.join(path)).unwrap(), "uncommitted local content\n");
    fs::remove_file(root.join(path)).unwrap();
    invoke(&["stage", "--", path]);
    invoke(&["commit", "--", "Delete version"]);
    let revisions = file_history(&cli, root, path, 100, None).unwrap();
    assert_eq!(revisions.len(), 3);
    let diff = revision_diff(&cli, root, path, &revisions[1], &revisions[0], None).unwrap();
    assert!(diff.contains("-second line"), "{diff}");
    assert!(!root.join(path).exists());
  }
}
