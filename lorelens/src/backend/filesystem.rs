use super::ignore::{load_loreignore, loreignored};
use super::*;

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
