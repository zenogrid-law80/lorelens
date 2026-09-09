use super::*;

/// Only a single repository-relative file may be passed to Lore's destructive API.
/// Missing worktree files are allowed; Lore resolves the tracked/staged node.
pub fn obliterate_args(root: &Path, path: &str) -> Result<Vec<String>, String> {
    let invalid = || "Select a single file inside the repository, outside metadata folders.".to_string();
    let normalized = path.replace('\\', "/");
    if normalized.is_empty() || normalized.split('/').any(|part| {
        part.is_empty() || matches!(part, "." | "..") || part.contains(':')
            || [".git", ".lore", ".urc"].iter().any(|name| part.eq_ignore_ascii_case(name))
            || part.ends_with(['.', ' '])
    }) { return Err(invalid()); }
    let mut target = root.to_path_buf();
    let components: Vec<_> = normalized.split('/').collect();
    for (index, component) in components.iter().enumerate() {
        target.push(component);
        match fs::symlink_metadata(&target) {
            Ok(metadata) => {
                #[cfg(windows)]
                let reparse = {
                    use std::os::windows::fs::MetadataExt;
                    metadata.file_attributes() & 0x400 != 0
                };
                #[cfg(not(windows))]
                let reparse = false;
                if metadata.file_type().is_symlink() || reparse
                    || (index + 1 == components.len() && !metadata.is_file()) {
                    return Err(invalid());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(vec!["file".into(), "obliterate".into(), format!("--path={normalized}")])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_broad_or_escaping_targets_without_deleting_anything() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("folder")).unwrap();
        for path in ["", ".", "..", "../secret", "/absolute", "C:\\secret", "folder",
            "folder/../secret", ".LoRe/config", "folder/.git/config", "file:stream", "folder./file"] {
            assert!(obliterate_args(root.path(), path).is_err(), "{path}");
        }
        assert!(root.path().join("folder").is_dir());
    }

    #[test]
    fn supports_missing_files_and_literal_option_like_names() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(obliterate_args(root.path(), "folder\\missing file.bin").unwrap(),
            ["file", "obliterate", "--path=folder/missing file.bin"]);
        assert_eq!(obliterate_args(root.path(), "--address=anything").unwrap()[2],
            "--path=--address=anything");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_ancestors() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
        assert!(obliterate_args(root.path(), "link/file").is_err());
    }
}
