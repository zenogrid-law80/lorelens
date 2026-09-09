use super::ignore::{load_loreignore, loreignored};
use super::*;

/// Copy external entries without overwriting existing data or following links.
pub fn copy_entries(root: &Path, destination: &Path, sources: &[PathBuf]) -> Result<usize, String> {
    fn metadata(path: &Path) -> Result<fs::Metadata, String> {
        let meta = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let link = meta.file_type().is_symlink();
        #[cfg(windows)]
        let link = {
            use std::os::windows::fs::MetadataExt;
            link || meta.file_attributes() & 0x400 != 0
        };
        if link || !(meta.is_file() || meta.is_dir()) {
            return Err(format!("Unsupported link or special file: {}", path.display()));
        }
        Ok(meta)
    }
    fn plan(source: &Path, target: &Path, entries: &mut Vec<(PathBuf, PathBuf, bool)>) -> Result<(), String> {
        let meta = metadata(source)?;
        if fs::symlink_metadata(target).is_ok() || entries.iter().any(|(_, p, _)| p == target) {
            return Err(format!("Destination already exists: {}", target.display()));
        }
        if target.file_name().is_some_and(|name| [".git", ".lore", ".urc"].iter().any(|n| name.eq_ignore_ascii_case(n))) {
            return Err("Cannot copy repository metadata.".into());
        }
        entries.push((source.to_path_buf(), target.to_path_buf(), meta.is_dir()));
        if meta.is_dir() {
            for child in fs::read_dir(source).map_err(|e| e.to_string())? {
                let child = child.map_err(|e| e.to_string())?;
                plan(&child.path(), &target.join(child.file_name()), entries)?;
            }
        }
        Ok(())
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let destination = destination.canonicalize().map_err(|e| e.to_string())?;
    let relative = destination.strip_prefix(&root).map_err(|_| "Copy must stay inside the repository.")?;
    if !destination.is_dir() || relative.components().any(|part| {
        [".git", ".lore", ".urc"].iter().any(|name| part.as_os_str().eq_ignore_ascii_case(name))
    }) {
        return Err("Invalid copy destination.".into());
    }
    let mut entries = Vec::new();
    for source in sources {
        metadata(source)?;
        let canonical = source.canonicalize().map_err(|e| e.to_string())?;
        if destination.starts_with(&canonical) {
            return Err("Cannot copy a folder into itself or its descendants.".into());
        }
        let name = source.file_name().ok_or("Cannot copy a filesystem root.")?;
        plan(source, &destination.join(name), &mut entries)?;
    }
    // Validate the entire batch before creating anything. Exclusive creation also
    // protects against destinations appearing between validation and copying.
    for (source, target, directory) in entries {
        let result = if directory {
            fs::create_dir(&target)
        } else {
            (|| {
                let mut input = fs::File::open(&source)?;
                let mut output = fs::OpenOptions::new().write(true).create_new(true).open(&target)?;
                std::io::copy(&mut input, &mut output)?;
                fs::set_permissions(&target, input.metadata()?.permissions())
            })()
        };
        result.map_err(|e| format!("Copy failed at {}: {e}. Some entries may have been copied.", target.display()))?;
    }
    Ok(sources.len())
}

pub fn delete_entry(root: &Path, source: &Path) -> Result<(), String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let metadata = fs::symlink_metadata(source).map_err(|e| e.to_string())?;
    let link = metadata.file_type().is_symlink();
    #[cfg(windows)]
    let link = {
        use std::os::windows::fs::MetadataExt;
        link || metadata.file_attributes() & 0x400 != 0
    };
    if link { return Err("Deleting symbolic links or junctions is not supported.".into()); }
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    let relative = source.strip_prefix(&root).map_err(|_| "Delete must stay inside the repository.")?;
    if relative.as_os_str().is_empty() || relative.components().any(|part| {
        [".git", ".lore", ".urc"].iter().any(|name| part.as_os_str().eq_ignore_ascii_case(name))
    }) {
        return Err("Cannot delete repository root or metadata.".into());
    }
    // remove_dir_all removes links within the tree without following their targets.
    if metadata.is_dir() { fs::remove_dir_all(&source) } else { fs::remove_file(&source) }
        .map_err(|e| format!("Delete failed: {e}"))
}

#[cfg(test)]
mod copy_tests {
    use super::*;

    #[test]
    fn copies_multiple_files_and_nested_folders_preserving_sources() {
        let source = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let folder = source.path().join("한글 folder");
        fs::create_dir_all(folder.join("empty")).unwrap();
        fs::write(folder.join("nested.txt"), "nested").unwrap();
        let file = source.path().join("file.txt");
        fs::write(&file, "file").unwrap();
        assert_eq!(copy_entries(root.path(), root.path(), &[file.clone(), folder.clone()]).unwrap(), 2);
        assert_eq!(fs::read(root.path().join("file.txt")).unwrap(), b"file");
        assert_eq!(fs::read(root.path().join("한글 folder/nested.txt")).unwrap(), b"nested");
        assert!(root.path().join("한글 folder/empty").is_dir());
        assert!(file.exists() && folder.join("nested.txt").exists());
    }

    #[test]
    fn rejects_conflicts_before_copying_any_item() {
        let source = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        fs::write(source.path().join("new"), "new").unwrap();
        fs::write(source.path().join("keep"), "replacement").unwrap();
        fs::write(root.path().join("keep"), "original").unwrap();
        assert!(copy_entries(root.path(), root.path(), &[source.path().join("new"), source.path().join("keep")]).is_err());
        assert!(!root.path().join("new").exists());
        assert_eq!(fs::read(root.path().join("keep")).unwrap(), b"original");
    }

    #[test]
    fn rejects_recursive_outside_and_metadata_destinations() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let folder = root.path().join("folder");
        fs::create_dir_all(folder.join("child")).unwrap();
        assert!(copy_entries(root.path(), &folder.join("child"), &[folder.clone()]).is_err());
        assert!(copy_entries(root.path(), outside.path(), &[folder.clone()]).is_err());
        fs::create_dir(root.path().join(".git")).unwrap();
        assert!(copy_entries(root.path(), &root.path().join(".git"), &[folder]).is_err());
    }
}

#[cfg(test)]
mod delete_tests {
    use super::*;
    #[test]
    fn deletes_files_and_nonempty_directories_but_preserves_other_files() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("한글 file.txt");
        let folder = root.path().join("folder");
        fs::write(&file, "file").unwrap();
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("nested.txt"), "nested").unwrap();
        fs::write(root.path().join("keep.txt"), "keep").unwrap();
        delete_entry(root.path(), &file).unwrap();
        delete_entry(root.path(), &folder).unwrap();
        assert!(!file.exists());
        assert!(!folder.exists());
        assert_eq!(fs::read_to_string(root.path().join("keep.txt")).unwrap(), "keep");
    }
    #[test]
    fn rejects_root_metadata_and_outside_paths() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        assert!(delete_entry(root.path(), root.path()).is_err());
        assert!(delete_entry(root.path(), outside.path()).is_err());
        for name in [".git", ".lore", ".urc"] {
            let dir = root.path().join(name);
            fs::create_dir(&dir).unwrap();
            fs::write(dir.join("config"), "keep").unwrap();
            assert!(delete_entry(root.path(), &dir).is_err());
            assert!(delete_entry(root.path(), &dir.join("config")).is_err());
            assert!(dir.join("config").exists());
        }
    }
}

pub fn list_directory(root: &Path, directory: &Path) -> Result<Vec<Entry>, String> {
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    let canonical = directory.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(&canonical_root) {
        return Err("Folder is outside this repository.".into());
    }
    let rules = load_loreignore(root)?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if [".git", ".lore", "target"]
            .iter()
            .any(|name| entry.file_name() == *name)
        {
            continue;
        }
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        if loreignored(&rules, &relative, meta.is_dir()) {
            continue;
        }
        entries.push(Entry {
            path: entry.path(),
            directory: meta.is_dir(),
            size: meta.len(),
        });
    }
    entries.sort_by(|a, b| b.directory.cmp(&a.directory).then(a.path.cmp(&b.path)));
    Ok(entries)
}

pub fn preview(root: &Path, path: &Path) -> Result<String, String> {
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    if !path.starts_with(root.canonicalize().map_err(|e| e.to_string())?) {
        return Err("File is outside this repository.".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(128 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let truncated = bytes.len() > 128 * 1024;
    bytes.truncate(128 * 1024);
    if bytes.contains(&0) {
        return Ok(
            "Binary file — text preview unavailable. Use File history to inspect revisions.".into(),
        );
    }
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        text.push_str("\n\n[Preview limited to 128 KiB]");
    }
    Ok(text)
}
