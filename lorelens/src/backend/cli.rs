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
        let bundled = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join(relative)));
        if let Some(path) = bundled.as_ref().filter(|path| path.is_file()) {
            return path.clone();
        }
        // Cargo runs keep the supplied CLI in the crate's asset directory.
        let development = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        if development.is_file() {
            return development;
        }
        // Keep a useful absolute path in the launch error when not yet bundled.
        bundled.unwrap_or(development)
    }
}

pub fn run_as(
    cli: &Path,
    root: &Path,
    args: &[String],
    json: bool,
    identity: Option<&str>,
) -> Result<String, String> {
    let mut command = Command::new(cli);
    command
        .current_dir(root)
        .args(["--no-pager", "--non-interactive"])
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // An explicit login URL and clone destination do not require a local repository.
    if is_repository(root) && args.first().is_none_or(|arg| arg != "clone") {
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
        format!(
            "Cannot start {}: {e}\nChoose lore.exe with Locate CLI, or set LORELENS_LORE_BIN.",
            cli.display()
        )
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
    let timeout = if args
        .first()
        .is_some_and(|arg| matches!(arg.as_str(), "clone" | "push"))
    {
        1800
    } else {
        120
    };
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if start.elapsed() < Duration::from_secs(timeout) => {
                thread::sleep(Duration::from_millis(40))
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(format!(
                    "Lore timed out after {timeout} seconds. Check the destination or refresh status before retrying."
                ));
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
        return Err(format!("{}\n{}", stdout.trim(), stderr.trim())
            .trim()
            .to_string());
    }
    Ok(if stderr.trim().is_empty() || json {
        stdout
    } else {
        format!("{stdout}\n{stderr}")
    })
}
