use std::{
  fs,
  path::{Path, PathBuf},
};

#[derive(Clone, Copy)]
pub enum BinarySide {
  Base,
  Theirs,
}

fn side(path: &Path, suffix: &str) -> PathBuf {
  let mut name = path.as_os_str().to_owned();
  name.push(suffix);
  name.into()
}

pub fn is_binary_merge(root: &Path, relative: &str) -> bool {
  let file = root.join(relative);
  file.is_file() && side(&file, "~base").is_file() && side(&file, "~theirs").is_file() && fs::symlink_metadata(side(&file, "~mine")).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
}

pub fn select_binary_merge(root: &Path, relative: &str, selected: BinarySide) -> Result<(), String> {
  let root = root.canonicalize().map_err(|e| e.to_string())?;
  if !is_binary_merge(&root, relative) {
    return Err(crate::i18n::t("Binary merge inputs have changed. Try resolving again."));
  }
  let file = root.join(relative);
  let base = side(&file, "~base");
  let theirs = side(&file, "~theirs");
  for path in [&file, &base, &theirs] {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || !path.canonicalize().map_err(|e| e.to_string())?.starts_with(&root) {
      return Err(crate::i18n::t("Merge inputs must be files inside the repository."));
    }
  }
  fs::copy(
    match selected {
      BinarySide::Base => &base,
      BinarySide::Theirs => &theirs,
    },
    &file,
  )
  .map_err(|e| e.to_string())?;
  // The normal resolve command removes backups only after Lore confirms success.
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn selects_each_side_and_preserves_backups_until_resolved() {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in [("file", b"original".as_slice()), ("file~base", b"\0base"), ("file~theirs", b"\0theirs")] {
      fs::write(dir.path().join(name), bytes).unwrap();
    }
    assert!(is_binary_merge(dir.path(), "file"));
    for (choice, expected) in [(BinarySide::Base, b"\0base".as_slice()), (BinarySide::Theirs, b"\0theirs")] {
      select_binary_merge(dir.path(), "file", choice).unwrap();
      assert_eq!(fs::read(dir.path().join("file")).unwrap(), expected);
      assert!(is_binary_merge(dir.path(), "file"));
    }
    fs::write(dir.path().join("file~mine"), b"mine").unwrap();
    assert!(!is_binary_merge(dir.path(), "file"));
    assert!(select_binary_merge(dir.path(), "file", BinarySide::Base).is_err());
    assert_eq!(fs::read(dir.path().join("file")).unwrap(), b"\0theirs");
  }
}
