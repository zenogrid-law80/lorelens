use std::{
  fs,
  io::{Read, Write},
  path::{Path, PathBuf},
};

const MAX_SIZE: u64 = 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FolderState {
  Included,
  Excluded,
  Mixed,
}

pub struct FolderListing {
  pub revision: String,
  pub folders: Vec<String>,
}

pub fn list_folders(cli: &Path, root: &Path, parent: &str, revision: Option<&str>, identity: Option<&str>) -> Result<FolderListing, String> {
  if !parent.is_empty() && !valid_folder(parent) {
    return Err("Invalid repository folder".into());
  }
  let mut args = vec!["repository".into(), "dump".into(), "--max-depth".into(), if parent.is_empty() { "1" } else { "2" }.into()];
  if !parent.is_empty() {
    args.push(format!("--path={parent}"));
  }
  if let Some(revision) = revision {
    args.push(format!("--revision={revision}"));
  }
  let listing = parse_listing(&super::run_as(cli, root, &args, true, identity)?, parent)?;
  if revision.is_some_and(|expected| expected != listing.revision) {
    return Err("Repository listing returned a different revision".into());
  }
  Ok(listing)
}

fn valid_folder(path: &str) -> bool {
  !path.contains(['\\', '\n', '\r', '\0', ':']) && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

fn parse_listing(output: &str, parent: &str) -> Result<FolderListing, String> {
  let mut revision = None;
  let mut complete = false;
  let mut folders = std::collections::BTreeSet::new();
  for line in output.lines().filter(|line| !line.trim().is_empty()) {
    let event: serde_json::Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
    let data = &event["data"];
    match event["tagName"].as_str() {
      Some("repositoryDumpBegin") => {
        let hash = data["revision"]
          .as_str()
          .filter(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
          .ok_or("Invalid repository revision")?;
        revision = Some(hash.to_owned());
      }
      Some("repositoryStateDumpNode") => {
        let name = data["name"].as_str().ok_or("Missing repository node name")?;
        let Some(folder) = name.strip_suffix('/') else { continue };
        let child = if parent.is_empty() {
          folder
        } else {
          let basename = parent.rsplit('/').next().unwrap();
          if folder == basename {
            continue;
          }
          folder.strip_prefix(&format!("{basename}/")).ok_or("Unexpected repository folder path")?
        };
        if child.contains('/') || !valid_folder(child) {
          return Err("Invalid repository child folder".into());
        }
        folders.insert(if parent.is_empty() { child.into() } else { format!("{parent}/{child}") });
      }
      Some("complete") => {
        if data["status"].as_i64() != Some(0) {
          return Err("Repository folder query failed".into());
        }
        complete = true;
      }
      _ => {}
    }
  }
  if !complete {
    return Err("Incomplete repository folder listing".into());
  }
  Ok(FolderListing {
    revision: revision.ok_or("Missing repository revision")?,
    folders: folders.into_iter().collect(),
  })
}

/// Conservative subtree status: arbitrary glob rules are kept and shown as mixed,
/// rather than claiming every descendant has the same inclusion state.
pub fn folder_state(text: &str, folder: &str) -> FolderState {
  let folder = folder.to_lowercase();
  let mut state = FolderState::Included;
  for line in text.lines() {
    let mut rule = line.trim();
    if rule.is_empty() || rule.starts_with('#') {
      continue;
    }
    let mut include = false;
    while let Some(rest) = rule.strip_prefix('!') {
      include = !include;
      rule = rest;
    }
    let next = if include { FolderState::Included } else { FolderState::Excluded };
    if matches!(rule, "**" | "/**") {
      state = next;
      continue;
    }
    let Some(rule) = rule.strip_prefix('/') else {
      state = FolderState::Mixed;
      continue;
    };
    let rule = rule.strip_suffix("/**").unwrap_or(rule).trim_end_matches('/');
    let mut literal = String::new();
    let mut escaped = false;
    let mut glob = false;
    for ch in rule.chars() {
      if escaped {
        literal.push(ch);
        escaped = false;
      } else if ch == '\\' {
        escaped = true;
      } else if matches!(ch, '*' | '?' | '[' | ']' | '{' | '}') {
        glob = true;
        break;
      } else {
        literal.push(ch);
      }
    }
    if glob || escaped {
      state = FolderState::Mixed;
      continue;
    }
    let literal = literal.to_lowercase();
    if folder == literal || folder.starts_with(&format!("{literal}/")) {
      state = next;
    } else if (folder.is_empty() || literal.starts_with(&format!("{folder}/"))) && state != next {
      state = FolderState::Mixed;
    }
  }
  state
}

pub fn set_folder(changes: &mut Vec<(String, bool)>, folder: &str, included: bool) {
  let key = folder.to_lowercase();
  changes.retain(|(path, _)| !key.is_empty() && path.to_lowercase() != key && !path.to_lowercase().starts_with(&format!("{key}/")));
  changes.push((folder.into(), included));
}

pub fn selection_text(original: &str, changes: &[(String, bool)]) -> Result<String, String> {
  // Only compact recognized generated blocks. Authored rules/comments are
  // ordering barriers: moving an override across them could change its meaning.
  let lines: Vec<_> = original.split_inclusive('\n').collect();
  let mut text = String::new();
  let mut selections = Vec::new();
  let mut managed = false;
  let mut index = 0;
  while index < lines.len() {
    let line = lines[index].trim();
    if line == "# LoreLens: folder selections" {
      managed = true;
      index += 1;
      continue;
    }
    if managed {
      if line.is_empty() {
        index += 1;
        continue;
      }
      if matches!(line, "**" | "!/**") {
        set_folder(&mut selections, "", line == "!/**");
        index += 1;
        continue;
      }
      let mut next = index + 1;
      while next < lines.len() && lines[next].trim().is_empty() {
        next += 1;
      }
      if let Some((folder, included)) = lines.get(next).and_then(|second| generated_folder(line, second.trim())) {
        set_folder(&mut selections, &folder, included);
        index = next + 1;
        continue;
      }
      if selections.is_empty() {
        // Keep the ownership marker when the entire block is unrecognized.
        text.push_str("# LoreLens: folder selections\n");
      } else {
        text = render_selections(&text, &selections)?;
        selections.clear();
      }
      managed = false;
    }
    text.push_str(lines[index]);
    index += 1;
  }
  for (folder, included) in changes {
    if !folder.is_empty() && !valid_folder(folder) {
      return Err("Invalid repository folder".into());
    }
    set_folder(&mut selections, folder, *included);
  }
  render_selections(&text, &selections)
}

fn generated_folder(first: &str, second: &str) -> Option<(String, bool)> {
  let included = first.starts_with('!');
  let rule = if included { first.strip_prefix('!')? } else { first };
  let literal = rule.strip_prefix('/')?.strip_suffix('/')?;
  if second != format!("{first}**") {
    return None;
  }
  let mut path = String::new();
  let mut chars = literal.chars();
  while let Some(ch) = chars.next() {
    if ch == '\\' {
      let escaped = chars.next()?;
      if !matches!(escaped, '*' | '?' | '[' | ']' | '{' | '}' | '\\') {
        return None;
      }
      path.push(escaped);
    } else if matches!(ch, '*' | '?' | '[' | ']' | '{' | '}') {
      return None;
    } else {
      path.push(ch);
    }
  }
  valid_folder(&path).then_some((path, included))
}

fn render_selections(original: &str, changes: &[(String, bool)]) -> Result<String, String> {
  let mut text = original.to_owned();
  if changes.is_empty() {
    return Ok(text);
  }
  if !text.is_empty() && !text.ends_with(['\n', '\r']) {
    text.push('\n');
  }
  text.push_str("# LoreLens: folder selections\n");
  for (index, (folder, included)) in changes.iter().enumerate() {
    let key = folder.to_lowercase();
    let ancestor = changes[..index]
      .iter()
      .rev()
      .find(|(parent, _)| parent.is_empty() || key.starts_with(&format!("{}/", parent.to_lowercase())));
    if ancestor.is_some_and(|(_, parent_included)| included == parent_included) {
      continue;
    }
    if folder.is_empty() {
      text.push_str(if *included { "!/**\n" } else { "**\n" });
      continue;
    }
    if !valid_folder(folder) {
      return Err("Invalid repository folder".into());
    }
    let mut literal = String::new();
    for ch in folder.chars() {
      if matches!(ch, '*' | '?' | '[' | ']' | '{' | '}' | '\\') {
        literal.push('\\');
      }
      literal.push(ch);
    }
    let prefix = if *included { "!" } else { "" };
    text.push_str(&format!("{prefix}/{literal}/\n{prefix}/{literal}/**\n"));
  }
  Ok(text)
}

pub struct ViewFile {
  pub path: PathBuf,
  pub contents: Option<String>,
}

pub fn load(root: &Path) -> Result<ViewFile, String> {
  let root = root.canonicalize().map_err(|error| error.to_string())?;
  let metadata = root.join(if root.join(".lore").is_dir() { ".lore" } else { ".urc" });
  let resolved = metadata.canonicalize().map_err(|error| error.to_string())?;
  if !resolved.starts_with(&root) || resolved == root || !resolved.is_dir() {
    return Err(crate::i18n::t("Repository metadata must be inside the workspace."));
  }
  let path = resolved.join("view");
  let contents = match fs::symlink_metadata(&path) {
    Ok(info) => {
      if !info.is_file() || info.file_type().is_symlink() {
        return Err(crate::i18n::t("The sparse view must be a regular file."));
      }
      let mut text = String::new();
      fs::File::open(&path)
        .map_err(|error| error.to_string())?
        .take(MAX_SIZE + 1)
        .read_to_string(&mut text)
        .map_err(|error| error.to_string())?;
      if text.len() as u64 > MAX_SIZE {
        return Err(crate::i18n::t("Sparse view files are limited to 1 MiB."));
      }
      Some(text)
    }
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
    Err(error) => return Err(error.to_string()),
  };
  Ok(ViewFile { path, contents })
}

pub fn merge_selections(root: &Path, original: &ViewFile, changes: &[(String, bool)]) -> Result<(), String> {
  let latest = load(root)?;
  if latest.path != original.path {
    return Err(crate::i18n::t("The sparse view changed outside this editor. Reopen it before saving."));
  }
  let merged = selection_text(latest.contents.as_deref().unwrap_or_default(), changes)?;
  save(root, &latest, &merged)
}

pub fn save(root: &Path, original: &ViewFile, text: &str) -> Result<(), String> {
  if text.len() as u64 > MAX_SIZE || text.contains('\0') {
    return Err(crate::i18n::t("Sparse rules must be text without NUL characters and no larger than 1 MiB."));
  }
  let current = load(root)?;
  if current.path != original.path || current.contents != original.contents {
    return Err(crate::i18n::t("The sparse view changed outside this editor. Reopen it before saving."));
  }
  if text == original.contents.as_deref().unwrap_or_default() {
    return Ok(());
  }
  let mut temp = tempfile::NamedTempFile::new_in(current.path.parent().unwrap()).map_err(|error| error.to_string())?;
  temp.write_all(text.as_bytes()).map_err(|error| error.to_string())?;
  temp.as_file().sync_all().map_err(|error| error.to_string())?;
  temp.persist(&current.path).map_err(|error| error.to_string())?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn compacts_repeated_folder_history_and_is_idempotent() {
    let mut history = "# LoreLens: folder selections\n\n!/**\n\n".to_owned();
    for included in [false, true, false, true, false] {
      history = render_selections(&history, &[("SparseWorkspaceTest".into(), included)]).unwrap();
    }
    let compact = selection_text(&history, &[]).unwrap();
    assert_eq!(compact, "# LoreLens: folder selections\n!/**\n/SparseWorkspaceTest/\n/SparseWorkspaceTest/**\n");
    assert_eq!(selection_text(&compact, &[]).unwrap(), compact);
    let included = selection_text(&compact, &[("SparseWorkspaceTest".into(), true)]).unwrap();
    assert_eq!(included, "# LoreLens: folder selections\n!/**\n");
    assert_eq!(folder_state(&compact, "SparseWorkspaceTest/Sub"), FolderState::Excluded);
    assert_eq!(folder_state(&compact, "Other"), FolderState::Included);
  }

  #[test]
  fn compaction_preserves_authored_order_and_escaped_names() {
    let first = render_selections("# user comment\r\n*.tmp\r\n", &[("Assets".into(), false)]).unwrap();
    let custom = format!("{first}# hand edited\n!/Assets/keep.txt\n");
    let history = render_selections(&custom, &[("Assets/한글 [art]".into(), true)]).unwrap();
    let compact = selection_text(&history, &[]).unwrap();
    assert!(compact.starts_with(&custom));
    assert_eq!(selection_text(&compact, &[]).unwrap(), compact);
    assert!(compact.contains("/Assets/한글 \\[art\\]/"));
    let unknown = "# LoreLens: folder selections\n/Assets/*\n# custom\n!/Assets/keep.txt\n";
    assert_eq!(selection_text(unknown, &[]).unwrap(), unknown);
    let children = render_selections("", &[("Assets/Textures".into(), false), ("Assets".into(), true)]).unwrap();
    assert_eq!(selection_text(&children, &[]).unwrap(), "# LoreLens: folder selections\n!/Assets/\n!/Assets/**\n");
  }

  #[test]
  fn merges_selections_into_latest_rules_and_leaves_external_edits_intact() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".lore")).unwrap();
    let original = load(root.path()).unwrap();
    let external = "# external edit\r\n**\r\n!/Source/**\r\n";
    fs::write(&original.path, external).unwrap();
    merge_selections(root.path(), &original, &[("Assets".into(), true), ("Source".into(), false)]).unwrap();
    let merged = fs::read_to_string(&original.path).unwrap();
    assert!(merged.starts_with(external));
    assert_eq!(folder_state(&merged, "Assets/Textures"), FolderState::Included);
    assert_eq!(folder_state(&merged, "Source"), FolderState::Excluded);
    merge_selections(root.path(), &original, &[]).unwrap();
    assert_eq!(fs::read_to_string(&original.path).unwrap(), merged);
    fs::remove_file(&original.path).unwrap();
    merge_selections(root.path(), &original, &[]).unwrap();
    assert!(!original.path.exists());
  }

  #[test]
  fn folder_selections_preserve_rules_and_respect_parent_overrides() {
    let original = "# custom\r\n**\r\n!/Source/**\r\n";
    assert_eq!(folder_state(original, "Source"), FolderState::Included);
    assert_eq!(folder_state(original, "Assets"), FolderState::Excluded);
    assert_eq!(folder_state(original, ""), FolderState::Mixed);
    let mut changes = Vec::new();
    set_folder(&mut changes, "Assets/Textures", true);
    let text = selection_text(original, &changes).unwrap();
    assert!(text.starts_with(original));
    assert_eq!(folder_state(&text, "Assets"), FolderState::Mixed);
    assert_eq!(folder_state(&text, "Assets/Textures"), FolderState::Included);
    assert_eq!(folder_state(&text, "Assets/Other"), FolderState::Excluded);
    set_folder(&mut changes, "Assets", false);
    assert_eq!(changes, [("Assets".into(), false)]);
    assert_eq!(folder_state(&selection_text(original, &changes).unwrap(), "Assets/Textures"), FolderState::Excluded);
    set_folder(&mut changes, "", true);
    assert_eq!(changes, [(String::new(), true)]);
    assert_eq!(folder_state(&selection_text(original, &changes).unwrap(), "Assets"), FolderState::Included);
    assert_eq!(selection_text(original, &[]).unwrap(), original);
    assert_eq!(folder_state("*.tmp", "Source"), FolderState::Mixed);
    assert!(selection_text("", &[("../escape".into(), true)]).is_err());
    let literal = selection_text("**\n", &[("한글 [art]".into(), true)]).unwrap();
    assert_eq!(folder_state(&literal, "한글 [art]"), FolderState::Included);
    assert!(literal.contains("/한글 \\[art\\]/"));
  }

  #[test]
  fn parses_immediate_folders_and_rejects_incomplete_dump() {
    let hash = "a".repeat(64);
    let events = [
      serde_json::json!({"tagName":"repositoryDumpBegin","data":{"revision":hash}}),
      serde_json::json!({"tagName":"repositoryStateDumpNode","data":{"name":"Source/"}}),
      serde_json::json!({"tagName":"repositoryStateDumpNode","data":{"name":"Source/한글/"}}),
      serde_json::json!({"tagName":"repositoryStateDumpNode","data":{"name":"Source/file.rs"}}),
      serde_json::json!({"tagName":"complete","data":{"status":0}}),
    ];
    let output = events.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n");
    let listing = parse_listing(&output, "Project/Source").unwrap();
    assert_eq!(listing.folders, ["Project/Source/한글"]);
    assert_eq!(listing.revision, hash);
    assert!(parse_listing(&output.replace("\"status\":0", "\"status\":1"), "Project/Source").is_err());
    assert!(parse_listing(&events[..4].iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"), "Source").is_err());
  }

  #[test]
  #[ignore = "Requires a usable Lore CLI; creates an isolated offline repository"]
  fn real_sparse_listing_includes_unmaterialized_folders() {
    let root = tempfile::tempdir().unwrap();
    let cli = super::super::find_cli();
    let invoke = |args: &[&str]| super::super::run_as(&cli, root.path(), &args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>(), false, None).unwrap();
    invoke(&["repository", "create", "--offline", "sparse-tree-test"]);
    for folder in ["Source", "Assets/한글 [art]", "Assets/Other"] {
      fs::create_dir_all(root.path().join(folder)).unwrap();
      fs::write(root.path().join(folder).join("file.txt"), "content\n").unwrap();
      invoke(&["stage", "--", &format!("{folder}/file.txt")]);
    }
    invoke(&["commit", "--", "Sparse test"]);
    let view = load(root.path()).unwrap();
    save(root.path(), &view, "**\n!/Source/**\n").unwrap();
    // Remove only fixture files, proving the query does not depend on disk contents.
    fs::remove_file(root.path().join("Assets/한글 [art]/file.txt")).unwrap();
    fs::remove_dir(root.path().join("Assets/한글 [art]")).unwrap();
    let listing = list_folders(&cli, root.path(), "", None, None).unwrap();
    assert_eq!(listing.folders, ["Assets", "Source"]);
    let children = list_folders(&cli, root.path(), "Assets", Some(&listing.revision), None).unwrap();
    assert_eq!(children.folders, ["Assets/Other", "Assets/한글 [art]"]);
    assert!(!root.path().join("Assets/한글 [art]").exists());
    let original = load(root.path()).unwrap();
    let text = selection_text(original.contents.as_deref().unwrap(), &[("Assets/한글 [art]".into(), true)]).unwrap();
    save(root.path(), &original, &text).unwrap();
    invoke(&["status"]);
    assert_eq!(load(root.path()).unwrap().contents.as_deref(), Some(text.as_str()));
  }

  #[test]
  fn creates_and_edits_both_metadata_formats_without_touching_workspace_files() {
    for directory in [".lore", ".urc"] {
      let root = tempfile::tempdir().unwrap();
      fs::create_dir(root.path().join(directory)).unwrap();
      fs::write(root.path().join("keep.txt"), "local changes").unwrap();
      let original = load(root.path()).unwrap();
      assert!(original.contents.is_none());
      save(root.path(), &original, "**\n!/Source/**\n# comment\n").unwrap();
      let loaded = load(root.path()).unwrap();
      assert_eq!(loaded.contents.as_deref(), Some("**\n!/Source/**\n# comment\n"));
      save(root.path(), &loaded, "").unwrap();
      assert_eq!(fs::read_to_string(root.path().join("keep.txt")).unwrap(), "local changes");
    }
  }

  #[test]
  fn refuses_external_changes_and_non_regular_files() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".lore")).unwrap();
    let original = load(root.path()).unwrap();
    fs::write(&original.path, "external").unwrap();
    assert!(save(root.path(), &original, "edited").is_err());
    assert_eq!(fs::read_to_string(&original.path).unwrap(), "external");
    fs::remove_file(&original.path).unwrap();
    fs::create_dir(&original.path).unwrap();
    assert!(load(root.path()).is_err());
  }

  #[test]
  fn rejects_invalid_text_and_leaves_missing_view_absent_when_unchanged() {
    let root = tempfile::tempdir().unwrap();
    assert!(load(root.path()).is_err());
    fs::create_dir(root.path().join(".lore")).unwrap();
    let original = load(root.path()).unwrap();
    save(root.path(), &original, "").unwrap();
    assert!(!original.path.exists());
    assert!(save(root.path(), &original, "a\0b").is_err());
    assert!(save(root.path(), &original, &"a".repeat(MAX_SIZE as usize + 1)).is_err());
  }
}
