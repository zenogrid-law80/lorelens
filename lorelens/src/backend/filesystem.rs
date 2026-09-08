use super::ignore::{load_loreignore, loreignored};
use super::*;

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
