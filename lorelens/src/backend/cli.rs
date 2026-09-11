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
}
