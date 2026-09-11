use super::*;
use std::collections::HashSet;

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
