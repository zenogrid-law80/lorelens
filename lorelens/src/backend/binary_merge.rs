use std::{
  fs,
  path::{Path, PathBuf},
};

#[derive(Clone, Copy)]
pub enum MergeSide {
  Mine,
  Theirs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeInputs {
  TwoWay,
  ThreeWay,
}

fn side(path: &Path, suffix: &str) -> PathBuf {
  let mut name = path.as_os_str().to_owned();
  name.push(suffix);
  name.into()
}

pub fn merge_inputs(root: &Path, relative: &str) -> Option<MergeInputs> {
  let file = root.join(relative);
  if !file.is_file() || !side(&file, "~base").is_file() || !side(&file, "~theirs").is_file() {
    return None;
  }
  if side(&file, "~mine").is_file() {
    Some(MergeInputs::ThreeWay)
  } else if fs::symlink_metadata(side(&file, "~mine")).is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound) {
    Some(MergeInputs::TwoWay)
  } else {
    None
  }
}

pub fn select_merge_sides(root: &Path, relatives: &[String], selected_side: MergeSide) -> Result<(), String> {
  let root = root.canonicalize().map_err(|e| e.to_string())?;
  let mut copies = Vec::with_capacity(relatives.len());
  for relative in relatives {
    let Some(inputs) = merge_inputs(&root, relative) else {
      return Err(crate::i18n::t("Merge inputs have changed. Try resolving again."));
    };
    let file = root.join(relative);
    let base = side(&file, "~base");
    let mine = side(&file, "~mine");
    let theirs = side(&file, "~theirs");
    let selected = match selected_side {
      MergeSide::Mine if inputs == MergeInputs::ThreeWay => &mine,
      MergeSide::Mine => &base,
      MergeSide::Theirs => &theirs,
    };
    for path in [&file, &base, &theirs, selected] {
      let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
      if !metadata.is_file() || metadata.file_type().is_symlink() || !path.canonicalize().map_err(|e| e.to_string())?.starts_with(&root) {
        return Err(crate::i18n::t("Merge inputs must be files inside the repository."));
      }
    }
    copies.push((selected.clone(), file));
  }
  for (source, destination) in copies {
    fs::copy(source, destination).map_err(|e| e.to_string())?;
  }
  // The normal resolve command removes backups only after Lore confirms success.
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  fn select(root: &Path, relative: &str, side: MergeSide) -> Result<(), String> {
    select_merge_sides(root, &[relative.into()], side)
  }

  #[test]
  fn detects_two_way_merge_and_selects_each_side() {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in [("file", b"original".as_slice()), ("file~base", b"\0base"), ("file~theirs", b"\0theirs")] {
      fs::write(dir.path().join(name), bytes).unwrap();
    }
    assert_eq!(merge_inputs(dir.path(), "file"), Some(MergeInputs::TwoWay));
    for (choice, expected) in [(MergeSide::Mine, b"\0base".as_slice()), (MergeSide::Theirs, b"\0theirs")] {
      select(dir.path(), "file", choice).unwrap();
      assert_eq!(fs::read(dir.path().join("file")).unwrap(), expected);
      assert_eq!(merge_inputs(dir.path(), "file"), Some(MergeInputs::TwoWay));
    }
  }

  #[test]
  fn detects_three_way_merge_and_uses_mine_backup() {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in [("file", b"merged".as_slice()), ("file~base", b"base"), ("file~mine", b"mine"), ("file~theirs", b"theirs")] {
      fs::write(dir.path().join(name), bytes).unwrap();
    }
    assert_eq!(merge_inputs(dir.path(), "file"), Some(MergeInputs::ThreeWay));
    select(dir.path(), "file", MergeSide::Mine).unwrap();
    assert_eq!(fs::read(dir.path().join("file")).unwrap(), b"mine");
    assert_eq!(merge_inputs(dir.path(), "file"), Some(MergeInputs::ThreeWay));
  }

  #[test]
  fn rejects_incomplete_or_unsafe_merge_inputs() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("file"), b"original").unwrap();
    fs::write(dir.path().join("file~base"), b"base").unwrap();
    assert_eq!(merge_inputs(dir.path(), "file"), None);
    assert!(select(dir.path(), "file", MergeSide::Mine).is_err());
  }

  #[test]
  fn batch_validates_every_input_before_overwriting_files() {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in [("valid", b"original".as_slice()), ("valid~base", b"mine"), ("valid~theirs", b"theirs"), ("invalid", b"unchanged")] {
      fs::write(dir.path().join(name), bytes).unwrap();
    }
    assert!(select_merge_sides(dir.path(), &["valid".into(), "invalid".into()], MergeSide::Theirs).is_err());
    assert_eq!(fs::read(dir.path().join("valid")).unwrap(), b"original");
    assert_eq!(fs::read(dir.path().join("invalid")).unwrap(), b"unchanged");
  }
}
