use std::{ffi::OsString, fs, path::Path, process::{Command, Stdio}};

pub const TOOLS: [&str; 3] = ["idea", "p4merge", "TortoiseGitMerge"];

pub fn is_executable(path: &Path) -> bool {
    if !path.is_file() { return false; }
    #[cfg(windows)]
    { path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("exe") || ext.eq_ignore_ascii_case("com")) }
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; path.metadata().is_ok_and(|m| m.permissions().mode() & 0o111 != 0) }
    #[cfg(not(any(windows, unix)))]
    { true }
}

pub fn resolve(tool: &str, configured: Option<&std::path::PathBuf>) -> Option<std::path::PathBuf> {
    if let Some(path) = configured {
        return (path.is_absolute() && is_executable(path)).then(|| path.clone());
    }
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).filter(|dir| dir.is_absolute()).find_map(|dir| {
        #[cfg(windows)]
        let names = [format!("{tool}.exe"), format!("{tool}.com")];
        #[cfg(not(windows))]
        let names = [tool.to_string()];
        names.into_iter().map(|name| dir.join(name)).find(|path| is_executable(path))
    })
}

pub fn arguments(tool: &str, merge: bool, base: &Path, theirs: &Path, yours: &Path, result: &Path) -> Result<Vec<OsString>, String> {
    let path = |p: &Path| p.as_os_str().to_owned();
    Ok(match tool {
        "idea" if merge => vec!["merge".into(), path(theirs), path(yours), path(base), path(result)],
        "idea" => vec!["diff".into(), path(base), path(yours)],
        "p4merge" => vec![path(base), path(theirs), path(yours), path(result)],
        "TortoiseGitMerge" => [("/base:", base), ("/mine:", yours), ("/theirs:", theirs), ("/merged:", result)]
            .into_iter().map(|(prefix, p)| { let mut arg = OsString::from(prefix); arg.push(p); arg }).collect(),
        _ => return Err(format!("Unknown external tool: {tool}")),
    })
}

pub fn diff(cli: &Path, root: &Path, relative: &str, revision: &str, identity: Option<&str>, tool: &str, executable: &Path) -> Result<(), String> {
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
    crate::backend::run_as(cli, root, &[
        "file".into(), "write".into(), "--path".into(), relative.into(),
        "--revision".into(), revision.into(), "--output".into(), base.to_string_lossy().into_owned(),
    ], false, identity)?;
    fs::copy(&base, &theirs).map_err(|e| e.to_string())?;
    let local = root.join(relative);
    if local.exists() { fs::copy(local, &yours).map_err(|e| e.to_string())?; }
    else { fs::write(&yours, []).map_err(|e| e.to_string())?; }
    fs::copy(&yours, &result).map_err(|e| e.to_string())?;
    let mut command = Command::new(executable);
    command.current_dir(root).args(arguments(tool, false, &base, &theirs, &yours, &result)?)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|e| format!("Cannot start {tool}: {e}. Choose its executable using Locate executable in the tools menu."))?;
    // IDE launchers can exit before an existing IDE opens the files. Retain snapshots.
    let _ = temporary.keep();
    std::thread::spawn(move || { let _ = child.wait(); });
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
        let b = Path::new("base 한글.txt"); let t = Path::new("theirs.txt");
        let y = Path::new("local file.txt"); let r = Path::new("result.txt");
        for tool in ["idea"] {
            assert_eq!(arguments(tool, false, b,t,y,r).unwrap(), vec![OsString::from("diff"), b.into(), y.into()]);
            assert_eq!(arguments(tool, true, b,t,y,r).unwrap(), vec![OsString::from("merge"), t.into(), y.into(), b.into(), r.into()]);
        }
        for merge in [false, true] {
            assert_eq!(arguments("p4merge", merge,b,t,y,r).unwrap(), vec![b.as_os_str(),t.as_os_str(),y.as_os_str(),r.as_os_str()]);
            assert_eq!(arguments("TortoiseGitMerge", merge,b,t,y,r).unwrap(), vec!["/base:base 한글.txt", "/mine:local file.txt", "/theirs:theirs.txt", "/merged:result.txt"]);
        }
        assert!(arguments("unknown", false,b,t,y,r).is_err());
    }
}
