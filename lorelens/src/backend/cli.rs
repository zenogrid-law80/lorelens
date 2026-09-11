use super::*;

pub fn find_cli() -> PathBuf {
  if let Some(path) = std::env::var_os("LORELENS_LORE_BIN") {
    return path.into();
  }
  #[cfg(target_os = "windows")]
  let relative = "dist/lore.exe";
  #[cfg(target_os = "macos")]
  let relative = "dis/lore";
  #[cfg(not(any(target_os = "windows", target_os = "macos")))]
  return PathBuf::from("lore");

  #[cfg(any(target_os = "windows", target_os = "macos"))]
  {
    #[cfg(target_os = "windows")]
    for installed in [
      PathBuf::from(r"C:\Program Files (x86)\LoreLens\dist\lore.exe"),
      PathBuf::from(r"C:\Program Files\LoreLens\dist\lore.exe"),
    ] {
      if installed.is_file() {
        return installed;
      }
    }
    let bundled = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join(relative)));
    if let Some(path) = bundled.as_ref().filter(|path| path.is_file()) {
      return path.clone();
    }
    // Cargo runs keep the supplied CLI in the crate's asset directory.
    let development = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    if development.is_file() {
      return development;
    }
    PathBuf::from("lore")
  }
}

pub fn run_as(cli: &Path, root: &Path, args: &[String], json: bool, identity: Option<&str>) -> Result<String, String> {
  #[cfg(windows)]
  let canonical_args = canonicalize_change_paths(root, args)?;
  #[cfg(windows)]
  let args = canonical_args.as_slice();
  let mut command = Command::new(cli);
  command
    .current_dir(root)
    .args(["--no-pager", "--non-interactive"])
    .env("NO_COLOR", "1")
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  // An explicit login URL and clone destination do not require a local repository.
  if is_repository(root) && args.first().is_none_or(|arg| arg != "clone") && !args.iter().any(|arg| arg == "--repository") {
    command.arg("--repository").arg(root);
  }
  if json {
    command.arg("--json");
  }
  if let Some(identity) = identity {
    command.arg("--identity").arg(identity);
  }
  command.args(args);
  #[cfg(windows)]
  {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000);
  }
  let mut child = command.spawn().map_err(|e| {
    let message = format!("Cannot start {}: {e}\nChoose lore.exe with Locate CLI, or set LORELENS_LORE_BIN.", cli.display());
    #[cfg(windows)]
    if e.kind() == std::io::ErrorKind::NotFound {
      return format!("{message}\nLore CLI was not found. Install it in PowerShell:\nwinget install EpicGames.Lore\nThen restart LoreLens or choose lore.exe with Locate CLI.");
    }
    message
  })?;
  let stdout = child.stdout.take().unwrap();
  let stderr = child.stderr.take().unwrap();
  // Drain both pipes concurrently; commands with large diffs must never deadlock.
  let read = |mut pipe: Box<dyn Read + Send>| {
    let mut kept = Vec::new();
    let mut buffer = [0u8; 8192];
    let mut truncated = false;
    loop {
      let n = pipe.read(&mut buffer).map_err(|e| e.to_string())?;
      if n == 0 {
        break;
      }
      let remaining = (16 * 1024 * 1024usize).saturating_sub(kept.len());
      kept.extend_from_slice(&buffer[..n.min(remaining)]);
      truncated |= n > remaining;
    }
    if truncated {
      return Err("Command output exceeded 16 MiB. Narrow the selected path.".into());
    }
    Ok::<String, String>(String::from_utf8_lossy(&kept).into_owned())
  };
  let out = thread::spawn(move || read(Box::new(stdout)));
  let err = thread::spawn(move || read(Box::new(stderr)));
  let start = Instant::now();
  let timeout = if args.first().is_some_and(|arg| matches!(arg.as_str(), "clone" | "push")) { 1800 } else { 120 };
  let result = loop {
    match child.try_wait() {
      Ok(Some(status)) => break Ok(status),
      Ok(None) if start.elapsed() < Duration::from_secs(timeout) => thread::sleep(Duration::from_millis(40)),
      Ok(None) => {
        let _ = child.kill();
        let _ = child.wait();
        break Err(format!("Lore timed out after {timeout} seconds. Check the destination or refresh status before retrying."));
      }
      Err(e) => {
        let _ = child.kill();
        let _ = child.wait();
        break Err(e.to_string());
      }
    }
  };
  let stdout = out.join().map_err(|_| "Output reader failed")??;
  let stderr = err.join().map_err(|_| "Error reader failed")??;
  if !result?.success() {
    let error = format!("{}\n{}", stdout.trim(), stderr.trim()).trim().to_string();
    if args.first().is_some_and(|arg| arg == "file")
      && args.get(1).is_some_and(|arg| arg == "obliterate")
      && args.iter().any(|arg| arg.starts_with("--path="))
      && let Some(address) = obliterate_address_from_error(&error)
    {
      let retry = vec!["file".into(), "obliterate".into(), "--address".into(), address.to_owned()];
      let command = format!("lore file obliterate --address {address}");
      return match run_as(cli, root, &retry, json, identity) {
        Ok(output) => Ok(format!("{error}\n\nFallback: {command}\n{output}")),
        Err(retry_error) => Err(format!("{error}\n\nFallback failed: {command}\n{retry_error}")),
      };
    }
    return Err(error);
  }
  Ok(if stderr.trim().is_empty() || json { stdout } else { format!("{stdout}\n{stderr}") })
}

pub fn run_branch_switch_skipping_unavailable(cli: &Path, root: &Path, args: &[String], identity: Option<&str>) -> Result<String, String> {
  run_branch_switch_skipping_unavailable_with(root, || run_as(cli, root, args, false, identity))
}

fn run_branch_switch_skipping_unavailable_with<F>(root: &Path, mut run: F) -> Result<String, String>
where
  F: FnMut() -> Result<String, String>,
{
  const MAX_SKIPPED_FILES: usize = 64;
  let mut skipped = Vec::new();
  loop {
    match run() {
      Ok(output) if skipped.is_empty() => return Ok(output),
      Ok(output) => {
        let paths = skipped.iter().map(|path| format!("  {path}")).collect::<Vec<_>>().join("\n");
        return Ok(format!(
          "{}\n\nLoreLens completed the branch switch with {} unavailable file(s) excluded:\n{}\nThe paths were added to the local view filter. Remove them from {} after the missing content is restored.",
          output.trim_end(),
          skipped.len(),
          paths,
          repository_view_path(root).display()
        ));
      }
      Err(error) => {
        let Some(path) = unavailable_sync_path(&error) else {
          return Err(with_skipped_paths(error, root, &skipped));
        };
        if skipped.contains(&path) {
          return Err(with_skipped_paths(format!("{error}\n\nLore still attempted to materialize the excluded path {path}."), root, &skipped));
        }
        if skipped.len() >= MAX_SKIPPED_FILES {
          return Err(with_skipped_paths(
            format!("{error}\n\nBranch switch stopped after excluding {MAX_SKIPPED_FILES} unavailable files."),
            root,
            &skipped,
          ));
        }
        append_view_exclusion(root, &path).map_err(|view_error| format!("{error}\n\nCould not exclude unavailable path {path}: {view_error}"))?;
        skipped.push(path);
      }
    }
  }
}

fn unavailable_sync_path(error: &str) -> Option<String> {
  if !error.contains("Address not found:") {
    return None;
  }
  error.lines().find_map(|line| {
    let raw = line.split_once(" - Failed to sync file ").map(|(_, path)| path)?;
    normalize_repository_relative_path(raw)
  })
}

fn normalize_repository_relative_path(raw: &str) -> Option<String> {
  let path = raw.trim().replace('\\', "/");
  if path.is_empty() || path.starts_with('/') || path.get(1..2) == Some(":") {
    return None;
  }
  let components = path.split('/').collect::<Vec<_>>();
  if components.iter().any(|component| component.is_empty() || matches!(*component, "." | ".."))
    || components
      .first()
      .is_some_and(|component| component.eq_ignore_ascii_case(".lore") || component.eq_ignore_ascii_case(".urc"))
  {
    return None;
  }
  Some(components.join("/"))
}

fn repository_view_path(root: &Path) -> PathBuf {
  let metadata = if root.join(".lore").is_dir() { ".lore" } else { ".urc" };
  root.join(metadata).join("view")
}

fn view_exclusion(path: &str) -> String {
  let mut pattern = String::from("/");
  for character in path.chars() {
    if matches!(character, '*' | '?' | '[' | ']' | '\\') {
      pattern.push('\\');
    }
    pattern.push(character);
  }
  pattern
}

fn append_view_exclusion(root: &Path, path: &str) -> Result<(), String> {
  let view = repository_view_path(root);
  let parent = view.parent().ok_or_else(|| "Invalid repository view path.".to_string())?;
  if !parent.is_dir() {
    return Err(format!("Repository metadata directory not found: {}", parent.display()));
  }
  let canonical_root = root.canonicalize().map_err(|error| format!("Cannot resolve repository root {}: {error}", root.display()))?;
  let canonical_parent = parent.canonicalize().map_err(|error| format!("Cannot resolve repository metadata {}: {error}", parent.display()))?;
  if !canonical_parent.starts_with(&canonical_root) {
    return Err(format!("Repository metadata is outside the repository root: {}", parent.display()));
  }
  match fs::symlink_metadata(&view) {
    Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => return Err(format!("Refusing to overwrite non-regular view file: {}", view.display())),
    Ok(_) => {}
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
    Err(error) => return Err(format!("Cannot inspect {}: {error}", view.display())),
  }
  let original = match fs::read_to_string(&view) {
    Ok(contents) => contents,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
    Err(error) => return Err(format!("Cannot read {}: {error}", view.display())),
  };
  let pattern = view_exclusion(path);
  let mut updated = original.clone();
  if !updated.is_empty() && !updated.ends_with(['\n', '\r']) {
    updated.push('\n');
  }
  updated.push_str("# LoreLens: unavailable content skipped during branch switch\n");
  updated.push_str(&pattern);
  updated.push('\n');
  fs::write(&view, updated).map_err(|error| format!("Cannot update {}: {error}", view.display()))
}

fn with_skipped_paths(error: String, root: &Path, skipped: &[String]) -> String {
  if skipped.is_empty() {
    return error;
  }
  let paths = skipped.iter().map(|path| format!("  {path}")).collect::<Vec<_>>().join("\n");
  format!(
    "{error}\n\nLoreLens excluded these unavailable paths in {} before retrying:\n{paths}",
    repository_view_path(root).display()
  )
}

fn obliterate_address_from_error(error: &str) -> Option<&str> {
  let address = error.split_once("Address not found:")?.1.lines().next()?.trim();
  (!address.is_empty() && address.bytes().all(|byte| byte.is_ascii_hexdigit() || byte == b'-')).then_some(address)
}

#[cfg(windows)]
fn canonicalize_change_paths(root: &Path, args: &[String]) -> Result<Vec<String>, String> {
  let mut args = args.to_vec();
  let action = if args.first().is_some_and(|arg| arg == "file") { args.get(1) } else { args.first() };
  if !action.is_some_and(|arg| matches!(arg.as_str(), "unstage" | "reset")) {
    return Ok(args);
  }
  let Some(separator) = args.iter().position(|arg| arg == "--") else {
    return Ok(args);
  };
  let canonical_root = root
    .canonicalize()
    .or_else(|_| std::path::absolute(root))
    .map_err(|error| format!("Cannot resolve repository path {}: {error}", root.display()))?;
  for arg in &mut args[separator + 1..] {
    let absolute = std::path::absolute(root.join(&*arg)).map_err(|error| format!("Cannot resolve path {arg}: {error}"))?;
    let mut ancestor = absolute.as_path();
    let mut missing = Vec::new();
    let mut canonical = loop {
      match ancestor.canonicalize() {
        Ok(path) => break path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
          let Some(name) = ancestor.file_name() else {
            return Err(format!("Cannot canonicalize path {arg}: {error}"));
          };
          missing.push(name);
          ancestor = ancestor.parent().ok_or_else(|| format!("Cannot canonicalize path {arg}: {error}"))?;
        }
        Err(error) => return Err(format!("Cannot canonicalize path {arg}: {error}")),
      }
    };
    // Deleted files (and deleted parent directories) have no canonical path.
    // Resolve their nearest existing ancestor, then append each missing component.
    for name in missing.into_iter().rev() {
      canonical.push(name);
    }
    let normalized = canonical.strip_prefix(&canonical_root).unwrap_or(canonical.as_path());
    let text = normalized.to_str().ok_or_else(|| format!("Path is not valid UTF-8: {arg}"))?;
    *arg = if text.is_empty() {
      ".".into()
    } else if normalized.is_absolute() {
      if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
      } else {
        text.strip_prefix(r"\\?\").unwrap_or(text).to_owned()
      }
    } else {
      text.to_owned()
    };
  }
  Ok(args)
}

#[cfg(all(test, windows))]
mod path_tests {
  use super::*;

  #[test]
  fn extracts_only_safe_obliterate_addresses() {
    let error = "[Error] Address not found: e45ba4b295fde040daab934efea5887962778aae33470a76b3561f0f3572b56f-01a0851a2a9375c1ad8939756f1dbca1";
    assert_eq!(
      obliterate_address_from_error(error),
      Some("e45ba4b295fde040daab934efea5887962778aae33470a76b3561f0f3572b56f-01a0851a2a9375c1ad8939756f1dbca1")
    );
    assert!(obliterate_address_from_error("Address not found: ..\\evil").is_none());
  }

  #[test]
  fn canonicalizes_existing_and_deleted_change_paths() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir(root.join("Existing")).unwrap();
    fs::write(root.join("Existing/file.txt"), "contents").unwrap();
    for action in ["unstage", "reset"] {
      let args = [action, "--", "Existing/./file.txt", "Existing/missing/deleted.exe", "Existing/../gone.jar"].map(str::to_owned);
      let normalized = canonicalize_change_paths(root, &args).unwrap();
      assert_eq!(&normalized[..2], &args[..2]);
      for (arg, relative) in normalized[2..].iter().zip([r"Existing\file.txt", r"Existing\missing\deleted.exe", "gone.jar"]) {
        assert!(!arg.contains('/'));
        assert!(!arg.starts_with(r"\\?\"));
        assert_eq!(Path::new(arg), Path::new(relative));
      }
    }
  }

  #[test]
  fn leaves_other_commands_and_option_values_unchanged() {
    let args = ["branch", "reset", "--", "main"].map(str::to_owned);
    assert_eq!(canonicalize_change_paths(Path::new("missing-root"), &args).unwrap(), args);
    let args = ["reset", "--revision", "main"].map(str::to_owned);
    assert_eq!(canonicalize_change_paths(Path::new("missing-root"), &args).unwrap(), args);
  }

  #[test]
  fn extracts_only_safe_unavailable_sync_paths() {
    let error = "[Error] Failed to synchronize state during branch switch: Address not found: abc-123\n  at lore-revision/src/fs/realize.rs:992 - Failed to sync file Plugins/Marketplace/AnimGenExample/Content/Characters/UEFN_Mannequin/Animations/Civ/Civ_Loc_WalkStrafe_Neutral_Male_T1.uasset";
    assert_eq!(
      unavailable_sync_path(error).as_deref(),
      Some("Plugins/Marketplace/AnimGenExample/Content/Characters/UEFN_Mannequin/Animations/Civ/Civ_Loc_WalkStrafe_Neutral_Male_T1.uasset")
    );
    assert!(unavailable_sync_path("Address not found: abc\n at x - Failed to sync file ../outside").is_none());
    assert!(unavailable_sync_path("at x - Failed to sync file safe/file.uasset").is_none());
  }

  #[test]
  fn appends_an_exact_local_view_exclusion_without_replacing_existing_rules() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir(root.join(".lore")).unwrap();
    fs::write(root.join(".lore/view"), "**\n!Plugins/**").unwrap();
    append_view_exclusion(root, "Plugins/Test[1]/asset?.uasset").unwrap();
    assert_eq!(
      fs::read_to_string(root.join(".lore/view")).unwrap(),
      "**\n!Plugins/**\n# LoreLens: unavailable content skipped during branch switch\n/Plugins/Test\\[1\\]/asset\\?.uasset\n"
    );
  }

  #[test]
  fn retries_branch_switch_after_excluding_only_the_unavailable_file() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir(root.join(".lore")).unwrap();
    let mut attempts = 0;
    let output = run_branch_switch_skipping_unavailable_with(root, || {
      attempts += 1;
      if attempts == 1 {
        Err("[Error] Address not found: abc-123\n at realize.rs:992 - Failed to sync file Content/Broken.uasset".into())
      } else {
        Ok("Switched to branch feat-test".into())
      }
    })
    .unwrap();
    assert_eq!(attempts, 2);
    assert!(output.contains("Switched to branch feat-test"));
    assert!(output.contains("Content/Broken.uasset"));
    assert!(fs::read_to_string(root.join(".lore/view")).unwrap().ends_with("/Content/Broken.uasset\n"));
  }
}
