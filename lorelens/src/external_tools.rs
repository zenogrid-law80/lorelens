use std::{
  ffi::OsString,
  fs,
  path::Path,
  process::{Command, Stdio},
};

pub const TOOLS: [&str; 4] = ["idea", "p4merge", "TortoiseGitMerge", "WinMergeU"];

pub struct Tool<'a> {
  pub name: &'a str,
  pub custom_arguments: Option<&'a str>,
  pub executable: &'a Path,
}

pub fn is_executable(path: &Path) -> bool {
  if !path.is_file() {
    return false;
  }
  #[cfg(windows)]
  {
    path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("exe") || ext.eq_ignore_ascii_case("com"))
  }
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    path.metadata().is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
  }
  #[cfg(not(any(windows, unix)))]
  {
    true
  }
}

pub fn resolve(tool: &str, configured: Option<&std::path::PathBuf>) -> Option<std::path::PathBuf> {
  if let Some(path) = configured {
    return (path.is_absolute() && is_executable(path)).then(|| path.clone());
  }
  let from_path = std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).filter(|dir| dir.is_absolute()).find_map(|dir| executable_in(&dir, tool)));
  from_path.or_else(|| default_install_dirs(tool).into_iter().find_map(|dir| executable_in(&dir, tool)))
}

fn executable_in(dir: &Path, tool: &str) -> Option<std::path::PathBuf> {
  #[cfg(windows)]
  let names = [format!("{tool}.exe"), format!("{tool}.com")];
  #[cfg(not(windows))]
  let names = [tool.to_string()];
  names.into_iter().map(|name| dir.join(name)).find(|path| is_executable(path))
}

#[cfg(windows)]
fn default_install_dirs(tool: &str) -> Vec<std::path::PathBuf> {
  let location = match tool.to_ascii_lowercase().as_str() {
    "idea" => ("LOCALAPPDATA", "Programs/RustRover/bin"),
    "winmergeu" => ("LOCALAPPDATA", "Programs/WinMerge"),
    "p4merge" => ("ProgramFiles", "Perforce"),
    "tortoisegitmerge" => ("ProgramFiles", "TortoiseGit/bin"),
    _ => return Vec::new(),
  };
  std::env::var_os(location.0).map(|root| vec![std::path::PathBuf::from(root).join(location.1)]).unwrap_or_default()
}

#[cfg(not(windows))]
fn default_install_dirs(_tool: &str) -> Vec<std::path::PathBuf> {
  Vec::new()
}

pub fn suggested_executable(tool: &str) -> Option<std::path::PathBuf> {
  resolve(tool, None).or_else(|| {
    default_install_dirs(tool).into_iter().next().map(|dir| {
      #[cfg(windows)]
      return dir.join(format!("{tool}.exe"));
      #[cfg(not(windows))]
      return dir.join(tool);
    })
  })
}

fn split_arguments(template: &str) -> Result<Vec<String>, String> {
  let mut arguments = Vec::new();
  let mut current = String::new();
  let mut quote = None;
  for character in template.chars() {
    match (quote, character) {
      (Some(expected), value) if value == expected => quote = None,
      (None, '"' | '\'') => quote = Some(character),
      (None, value) if value.is_whitespace() => {
        if !current.is_empty() {
          arguments.push(std::mem::take(&mut current));
        }
      }
      _ => current.push(character),
    }
  }
  if quote.is_some() {
    return Err("Custom arguments contain an unclosed quote.".into());
  }
  if !current.is_empty() {
    arguments.push(current);
  }
  Ok(arguments)
}

pub fn arguments(tool: &str, custom: Option<&str>, merge: bool, base: &Path, theirs: &Path, yours: &Path, result: &Path) -> Result<Vec<OsString>, String> {
  let path = |p: &Path| p.as_os_str().to_owned();
  Ok(match tool {
    "idea" if merge => vec!["merge".into(), path(theirs), path(yours), path(base), path(result)],
    "idea" => vec!["diff".into(), path(base), path(yours)],
    "p4merge" => vec![path(base), path(theirs), path(yours), path(result)],
    "TortoiseGitMerge" => [("/base:", base), ("/mine:", yours), ("/theirs:", theirs), ("/merged:", result)]
      .into_iter()
      .map(|(prefix, p)| {
        let mut arg = OsString::from(prefix);
        arg.push(p);
        arg
      })
      .collect(),
    "WinMergeU" if merge => vec![
      "/e".into(),
      "/u".into(),
      "/wl".into(),
      "/wm".into(),
      "/wr".into(),
      path(base),
      path(yours),
      path(theirs),
      "/o".into(),
      path(result),
    ],
    "WinMergeU" => vec!["/e".into(), "/u".into(), path(base), path(yours)],
    "custom" => split_arguments(custom.unwrap_or_default())?
      .into_iter()
      .map(|argument| match argument.as_str() {
        "{base}" => path(base),
        "{theirs}" => path(theirs),
        "{yours}" => path(yours),
        "{result}" => path(result),
        _ => argument.into(),
      })
      .collect(),
    _ => return Err(format!("Unknown external tool: {tool}")),
  })
}

pub fn diff(cli: &Path, root: &Path, relative: &str, revision: &str, identity: Option<&str>, tool: Tool<'_>) -> Result<(), String> {
  let temporary = tempfile::Builder::new().prefix("lorelens-diff-").tempdir().map_err(|e| e.to_string())?;
  let name = Path::new(relative).file_name().ok_or("Select a file to compare")?;
  let file = |side: &str| -> Result<std::path::PathBuf, String> {
    let dir = temporary.path().join(side);
    fs::create_dir(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(name))
  };
  let base = file("base")?;
  let theirs = file("theirs")?;
  let yours = file("yours")?;
  let result = file("result")?;
  crate::backend::run_as(
    cli,
    root,
    &[
      "file".into(),
      "write".into(),
      "--path".into(),
      relative.into(),
      "--revision".into(),
      revision.into(),
      "--output".into(),
      base.to_string_lossy().into_owned(),
    ],
    false,
    identity,
  )?;
  fs::copy(&base, &theirs).map_err(|e| e.to_string())?;
  let local = root.join(relative);
  if local.exists() {
    fs::copy(local, &yours).map_err(|e| e.to_string())?;
  } else {
    fs::write(&yours, []).map_err(|e| e.to_string())?;
  }
  fs::copy(&yours, &result).map_err(|e| e.to_string())?;
  let mut command = Command::new(tool.executable);
  command
    .current_dir(root)
    .args(arguments(tool.name, tool.custom_arguments, false, &base, &theirs, &yours, &result)?)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
  #[cfg(windows)]
  {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000);
  }
  let mut child = command
    .spawn()
    .map_err(|e| format!("Cannot start {}: {e}. Choose its executable using Locate executable in the tools menu.", tool.name))?;
  // IDE launchers can exit before an existing IDE opens the files. Retain snapshots.
  let _ = temporary.keep();
  std::thread::spawn(move || {
    let _ = child.wait();
  });
  Ok(())
}

pub fn merge(root: &Path, relative: &str, tool: Tool<'_>) -> Result<(), String> {
  let root = root.canonicalize().map_err(|e| e.to_string())?;
  let result = root.join(relative);
  let side = |suffix: &str| {
    let mut path = result.as_os_str().to_owned();
    path.push(suffix);
    std::path::PathBuf::from(path)
  };
  let (base, mine, theirs) = (side("~base"), side("~mine"), side("~theirs"));
  let before = fs::metadata(&result).and_then(|m| m.modified()).map_err(|e| e.to_string())?;
  for path in [&result, &base, &mine, &theirs] {
    let resolved = path
      .canonicalize()
      .map_err(|_| crate::i18n::tf("Merge input is missing: {path}", &[("path", path.display().to_string())]))?;
    if !resolved.starts_with(&root) || !resolved.is_file() {
      return Err(crate::i18n::t("Merge inputs must be files inside the repository."));
    }
  }
  let mut command = Command::new(tool.executable);
  command
    .current_dir(&root)
    .args(arguments(tool.name, tool.custom_arguments, true, &base, &theirs, &mine, &result)?)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
  #[cfg(windows)]
  {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000);
  }
  let mut child = command.spawn().map_err(|e| e.to_string())?;
  let status = child.wait().map_err(|e| e.to_string())?;
  if !status.success() {
    return Err(crate::i18n::t("Merge tool did not complete successfully. Conflict was not resolved."));
  }
  let after = fs::metadata(&result).and_then(|m| m.modified()).map_err(|e| e.to_string())?;
  if before == after {
    return Err(crate::i18n::t("No saved merge result detected. Conflict was not resolved."));
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn configured_executable_validation() {
    let executable = std::env::current_exe().unwrap();
    assert_eq!(resolve("rider", Some(&executable)), Some(executable.clone()));
    assert!(resolve("rider", Some(&executable.with_file_name("missing-tool.exe"))).is_none());
    assert!(resolve("rider", Some(&executable.parent().unwrap().to_path_buf())).is_none());
    assert!(resolve("rider", Some(&"relative.exe".into())).is_none());
  }
  #[test]
  fn tool_arguments_preserve_order_and_paths() {
    let b = Path::new("base 한글.txt");
    let t = Path::new("theirs.txt");
    let y = Path::new("local file.txt");
    let r = Path::new("result.txt");
    for tool in ["idea"] {
      assert_eq!(arguments(tool, None, false, b, t, y, r).unwrap(), vec![OsString::from("diff"), b.into(), y.into()]);
      assert_eq!(arguments(tool, None, true, b, t, y, r).unwrap(), vec![OsString::from("merge"), t.into(), y.into(), b.into(), r.into()]);
    }
    for merge in [false, true] {
      assert_eq!(arguments("p4merge", None, merge, b, t, y, r).unwrap(), vec![b.as_os_str(), t.as_os_str(), y.as_os_str(), r.as_os_str()]);
      assert_eq!(
        arguments("TortoiseGitMerge", None, merge, b, t, y, r).unwrap(),
        vec!["/base:base 한글.txt", "/mine:local file.txt", "/theirs:theirs.txt", "/merged:result.txt"]
      );
    }
    assert_eq!(arguments("WinMergeU", None, false, b, t, y, r).unwrap(), vec!["/e", "/u", "base 한글.txt", "local file.txt"]);
    assert_eq!(
      arguments("WinMergeU", None, true, b, t, y, r).unwrap(),
      vec!["/e", "/u", "/wl", "/wm", "/wr", "base 한글.txt", "local file.txt", "theirs.txt", "/o", "result.txt"]
    );
    assert_eq!(
      arguments("custom", Some("--wait \"{base}\" '{yours}' {result}"), false, b, t, y, r).unwrap(),
      vec!["--wait", "base 한글.txt", "local file.txt", "result.txt"]
    );
    assert!(arguments("unknown", None, false, b, t, y, r).is_err());
  }
}
