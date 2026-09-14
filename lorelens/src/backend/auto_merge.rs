use std::{
  fs,
  io::{Read, Write},
  path::Path,
};
use unicode_segmentation::UnicodeSegmentation;

struct Edit<'a> {
  start: usize,
  end: usize,
  lines: Vec<&'a str>,
}

// Myers shortest edit path with bounded trace memory and comparison work.
fn edits<'a>(base: &[&str], changed: &[&'a str]) -> Option<Vec<Edit<'a>>> {
  let prefix = base.iter().zip(changed).take_while(|(a, b)| a == b).count();
  let suffix = base[prefix..].iter().rev().zip(changed[prefix..].iter().rev()).take_while(|(a, b)| a == b).count();
  let old = &base[prefix..base.len() - suffix];
  let new = &changed[prefix..changed.len() - suffix];
  if old.is_empty() || new.is_empty() {
    return Some(if old.is_empty() && new.is_empty() {
      Vec::new()
    } else {
      vec![Edit {
        start: prefix,
        end: prefix + old.len(),
        lines: new.to_vec(),
      }]
    });
  }
  let max = old.len() + new.len();
  let offset = max + 1;
  let width = 2 * max + 3;
  if width > 1_000_000 {
    return None;
  }
  let mut frontier = vec![0usize; width];
  let mut trace = Vec::new();
  let mut work = 0usize;
  let distance = 'search: loop {
    let d = trace.len();
    if (d + 1) * width > 1_000_000 {
      return None;
    }
    trace.push(frontier.clone());
    for k in (-(d as isize)..=d as isize).step_by(2) {
      let index = (offset as isize + k) as usize;
      let mut x = if k == -(d as isize) || (k != d as isize && frontier[index - 1] < frontier[index + 1]) {
        frontier[index + 1]
      } else {
        frontier[index - 1] + 1
      };
      let mut y = (x as isize - k) as usize;
      while x < old.len() && y < new.len() && old[x] == new[y] {
        x += 1;
        y += 1;
        work += 1;
        if work > 4_000_000 {
          return None;
        }
      }
      frontier[index] = x;
      if x >= old.len() && y >= new.len() {
        break 'search d;
      }
    }
  };
  let (mut x, mut y) = (old.len(), new.len());
  let mut matches = Vec::new();
  for d in (1..=distance).rev() {
    let k = x as isize - y as isize;
    let index = (offset as isize + k) as usize;
    let previous = &trace[d];
    let previous_k = if k == -(d as isize) || (k != d as isize && previous[index - 1] < previous[index + 1]) {
      k + 1
    } else {
      k - 1
    };
    let previous_x = previous[(offset as isize + previous_k) as usize];
    let previous_y = (previous_x as isize - previous_k) as usize;
    while x > previous_x && y > previous_y {
      x -= 1;
      y -= 1;
      matches.push((x, y));
    }
    x = previous_x;
    y = previous_y;
  }
  matches.reverse();
  let (mut i, mut j) = (0, 0);
  let mut result = Vec::new();
  for (x, y) in matches.into_iter().chain(std::iter::once((old.len(), new.len()))) {
    if i < x || j < y {
      result.push(Edit {
        start: prefix + i,
        end: prefix + x,
        lines: new[j..y].to_vec(),
      });
    }
    i = x + 1;
    j = y + 1;
  }
  Some(result)
}

fn merge_text(base: &str, mine: &str, theirs: &str) -> Option<String> {
  if [base, mine, theirs].iter().any(|text| text.contains('\0')) {
    return None;
  }
  if mine == theirs || base == theirs {
    return Some(mine.into());
  }
  if base == mine {
    return Some(theirs.into());
  }
  // Tiny lines can otherwise expand an 8 MiB input into hundreds of MiB of
  // slice metadata before the Myers budget is checked.
  fn tokenize(text: &str) -> Option<Vec<&str>> {
    let tokens = text.split_inclusive('\n').take(100_001).collect::<Vec<_>>();
    (tokens.len() <= 100_000).then_some(tokens)
  }
  let base = tokenize(base)?;
  let mine = tokenize(mine)?;
  let theirs = tokenize(theirs)?;
  merge_tokens(&base, &mine, &theirs, true)
}

fn merge_tokens(base: &[&str], mine: &[&str], theirs: &[&str], lines: bool) -> Option<String> {
  let mut changes = edits(base, mine)?;
  let mut refined = std::collections::HashMap::new();
  for incoming in edits(base, theirs)? {
    let mut duplicate = false;
    for local in &changes {
      if local.start == incoming.start && local.end == incoming.end && local.lines == incoming.lines {
        duplicate = true;
        break;
      }
      let overlap = if local.start == local.end && incoming.start == incoming.end {
        local.start == incoming.start
      } else if local.start == local.end {
        incoming.start <= local.start && local.start < incoming.end
      } else if incoming.start == incoming.end {
        local.start <= incoming.start && incoming.start < local.end
      } else {
        local.start < incoming.end && incoming.start < local.end
      };
      if overlap {
        // Refine only one-for-one line replacements. Multi-line overlaps and
        // competing insertions still require a person's decision.
        if lines && local.start == incoming.start && local.end == incoming.end && local.end == local.start + 1 && local.lines.len() == 1 && incoming.lines.len() == 1 {
          let original = base[local.start].graphemes(true).take(32_769).collect::<Vec<_>>();
          let mine = local.lines[0].graphemes(true).take(32_769).collect::<Vec<_>>();
          let theirs = incoming.lines[0].graphemes(true).take(32_769).collect::<Vec<_>>();
          if original.len() + mine.len() + theirs.len() > 32_768 {
            return None;
          }
          refined.insert(local.start, merge_tokens(&original, &mine, &theirs, false)?);
          duplicate = true;
          break;
        }
        return None;
      }
    }
    if !duplicate {
      changes.push(incoming);
    }
  }
  changes.sort_by_key(|edit| edit.start);
  let mut result = String::new();
  let mut cursor = 0;
  for edit in changes {
    for token in &base[cursor..edit.start] {
      append_token(&mut result, token, lines)?;
    }
    if let Some(text) = refined.remove(&edit.start) {
      append_token(&mut result, &text, lines)?;
    } else {
      for token in edit.lines {
        append_token(&mut result, token, lines)?;
      }
    }
    cursor = edit.end;
  }
  for token in &base[cursor..] {
    append_token(&mut result, token, lines)?;
  }
  Some(result)
}

fn append_token(output: &mut String, token: &str, lines: bool) -> Option<()> {
  if lines && !token.is_empty() && !output.is_empty() && !output.ends_with('\n') {
    return None;
  }
  output.push_str(token);
  Some(())
}

/// Returns false without changing the working file when a tool must handle it.
pub fn try_auto_merge(root: &Path, relative: &str) -> Result<bool, String> {
  let root = root.canonicalize().map_err(|e| e.to_string())?;
  let file = root.join(relative);
  let mut paths = vec![file.clone()];
  for suffix in ["~base", "~mine", "~theirs"] {
    let mut path = file.as_os_str().to_owned();
    path.push(suffix);
    paths.push(path.into());
  }
  let mut contents = Vec::new();
  for path in &paths {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || !path.canonicalize().map_err(|e| e.to_string())?.starts_with(&root) {
      return Err(crate::i18n::t("Merge inputs must be files inside the repository."));
    }
    if metadata.len() > 8 * 1024 * 1024 {
      return Ok(false);
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
      .map_err(|e| e.to_string())?
      .take(8 * 1024 * 1024 + 1)
      .read_to_end(&mut bytes)
      .map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
      return Ok(false);
    }
    contents.push(bytes);
  }
  let text = contents[1..].iter().map(|bytes| std::str::from_utf8(bytes)).collect::<Result<Vec<_>, _>>();
  let Ok(text) = text else { return Ok(false) };
  let Some(result) = merge_text(text[0], text[1], text[2]) else { return Ok(false) };
  let mut temporary = tempfile::NamedTempFile::new_in(file.parent().ok_or("Invalid merge path")?).map_err(|e| e.to_string())?;
  temporary.write_all(result.as_bytes()).map_err(|e| e.to_string())?;
  temporary
    .as_file()
    .set_permissions(fs::metadata(&file).map_err(|e| e.to_string())?.permissions())
    .map_err(|e| e.to_string())?;
  temporary.as_file().sync_all().map_err(|e| e.to_string())?;
  for (path, original) in paths.iter().zip(&contents) {
    if fs::read(path).map_err(|e| e.to_string())? != *original {
      return Err(crate::i18n::t("Merge inputs have changed. Try resolving again."));
    }
  }
  temporary.persist(&file).map_err(|e| e.to_string())?;
  Ok(true)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn combines_independent_changes_and_preserves_line_endings() {
    assert_eq!(
      merge_text("one\r\nshared\r\nthree", "MINE\r\nshared\r\nthree", "one\r\nshared\r\nTHEIRS"),
      Some("MINE\r\nshared\r\nTHEIRS".into())
    );
    assert_eq!(merge_text("a\nb\nc\nd\n", "a\nc\nd\n", "a\nb\nc\nd\nextra\n"), Some("a\nc\nd\nextra\n".into()));
    assert_eq!(merge_text("a\nb\n", "A\nb\n", "a\nB\n"), Some("A\nB\n".into()));
  }

  #[test]
  fn rejects_overlapping_changes_and_binary_content() {
    assert_eq!(merge_text("a\n", "mine\n", "theirs\n"), None);
    assert_eq!(merge_text("a\n", "x\na\n", "y\na\n"), None);
    assert_eq!(merge_text("\0", "\0", "\0"), None);
    assert_eq!(merge_text("a\n", "same\n", "same\n"), Some("same\n".into()));
    assert_eq!(merge_text("a\n", "a\n", ""), Some(String::new()));
  }

  #[test]
  fn ignores_lore_result_and_keeps_conflicts_and_backups_intact() {
    let dir = tempfile::tempdir().unwrap();
    for (name, content) in [("A.txt", "Lore result"), ("A.txt~base", "a\nb\n"), ("A.txt~mine", "A\nb\n"), ("A.txt~theirs", "a\nB\n")] {
      fs::write(dir.path().join(name), content).unwrap();
    }
    assert!(try_auto_merge(dir.path(), "A.txt").unwrap());
    assert_eq!(fs::read_to_string(dir.path().join("A.txt")).unwrap(), "A\nB\n");
    assert!(dir.path().join("A.txt~base").exists());
    fs::write(dir.path().join("A.txt~theirs"), "other\nb\n").unwrap();
    assert!(!try_auto_merge(dir.path(), "A.txt").unwrap());
    assert_eq!(fs::read_to_string(dir.path().join("A.txt")).unwrap(), "A\nB\n");
  }

  #[test]
  fn bounds_diff_memory() {
    assert!(edits(&vec!["a"; 2000], &vec!["b"; 2000]).is_none());
  }

  #[test]
  fn appends_after_replaced_lines_in_either_direction() {
    let base = "ABCD\nEFG\n";
    let mine = "ABCDQQ\nEFGQQ\n";
    let theirs = "ABCD\nEFG\n\nHIJ\n";
    let expected = "ABCDQQ\nEFGQQ\n\nHIJ\n";
    assert_eq!(merge_text(base, mine, theirs).as_deref(), Some(expected));
    assert_eq!(merge_text(base, theirs, mine).as_deref(), Some(expected));
    assert_eq!(merge_text("a\nb\nc\n", "A\nb\nc\n", "a\nextra\nb\nc\n").as_deref(), Some("A\nextra\nb\nc\n"));
    assert_eq!(merge_text("a\nb\nc\n", "A\nc\n", "a\nextra\nb\nc\n"), None);
  }

  #[test]
  fn identical_edits_are_not_duplicated_and_insertions_stay_ordered() {
    assert_eq!(merge_text("a\nb\nc\nd\n", "A\nb\nc\nD\n", "A\nb\nC\nd\n"), Some("A\nb\nC\nD\n".into()));
    assert_eq!(merge_text("a\nb\nc\n", "first\na\nb\nc\n", "a\nb\nc\nlast\n"), Some("first\na\nb\nc\nlast\n".into()));
    assert_eq!(merge_text("a\nb\n", "x\nb\n", "insert\na\nb\n"), None);
  }

  #[test]
  fn unsupported_encoding_does_not_touch_working_file() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("A.txt"), b"keep working contents").unwrap();
    for suffix in ["~base", "~mine", "~theirs"] {
      fs::write(dir.path().join(format!("A.txt{suffix}")), [0xff, 0xfe, 0]).unwrap();
    }
    assert!(!try_auto_merge(dir.path(), "A.txt").unwrap());
    assert_eq!(fs::read(dir.path().join("A.txt")).unwrap(), b"keep working contents");
    assert!(try_auto_merge(dir.path(), "missing.txt").is_err());
  }

  #[test]
  fn newline_removal_cannot_join_another_sides_appended_line() {
    assert_eq!(merge_text("a\n", "A", "a\nb\n"), None);
    assert_eq!(merge_text("a\n", "a\nb\n", "A"), None);
    assert_eq!(merge_text("a\r\n", "A", "a\r\nb\r\n"), None);
  }

  #[test]
  fn merges_large_files_with_sparse_edits() {
    let base = (0..10_000).map(|i| format!("line {i}\n")).collect::<String>();
    let mine = base.replacen("line 10\n", "mine\n", 1).replacen("line 9900\n", "mine end\n", 1);
    let theirs = base.replacen("line 5000\n", "theirs\n", 1);
    let expected = mine.replacen("line 5000\n", "theirs\n", 1);
    assert_eq!(merge_text(&base, &mine, &theirs), Some(expected));
  }

  #[test]
  fn refines_same_line_without_splitting_unicode_graphemes() {
    assert_eq!(merge_text("x=1; y=2;\n", "x=3; y=2;\n", "x=1; y=4;\n").as_deref(), Some("x=3; y=4;\n"));
    assert_eq!(merge_text("이름=가; 값=1\r\n", "이름=나; 값=1\r\n", "이름=가; 값=2\r\n").as_deref(), Some("이름=나; 값=2\r\n"));
    assert_eq!(merge_text("👩‍💻 e\u{301}\n", "👨‍💻 e\u{301}\n", "👩‍💻 a\u{301}\n").as_deref(), Some("👨‍💻 a\u{301}\n"));
    assert_eq!(merge_text("e\u{301}\n", "a\u{301}\n", "e\u{300}\n"), None);
    assert_eq!(merge_text("x=1\n", "x=2\n", "x=3\n"), None);
  }

  #[test]
  fn myers_edits_reconstruct_short_sequences() {
    let sequences = (0..64usize)
      .map(|bits| (0..6).map(|i| if bits & (1 << i) == 0 { "a" } else { "b" }).collect::<Vec<_>>())
      .collect::<Vec<_>>();
    for base in &sequences {
      for changed in &sequences {
        let mut reconstructed = Vec::new();
        let mut cursor = 0;
        for edit in edits(base, changed).unwrap() {
          assert!(cursor <= edit.start && edit.start <= edit.end);
          reconstructed.extend_from_slice(&base[cursor..edit.start]);
          reconstructed.extend(edit.lines);
          cursor = edit.end;
        }
        reconstructed.extend_from_slice(&base[cursor..]);
        assert_eq!(&reconstructed, changed);
      }
    }
  }

  #[test]
  fn variable_length_inputs_do_not_panic_or_depend_on_side_order() {
    let alphabet = ["a\n", "b\n", "\r\n", "한글\n", "👩‍💻\n", "last"];
    let mut seed = 123456789u64;
    let mut next = || {
      seed ^= seed << 13;
      seed ^= seed >> 7;
      seed ^= seed << 17;
      seed as usize
    };
    for _ in 0..20_000 {
      let mut texts = Vec::new();
      for _ in 0..3 {
        let length = next() % 25;
        let text = (0..length).map(|_| alphabet[next() % alphabet.len()]).collect::<String>();
        texts.push(text);
      }
      let result = merge_text(&texts[0], &texts[1], &texts[2]);
      assert_eq!(result, merge_text(&texts[0], &texts[2], &texts[1]), "inputs: {texts:?}");
      let base = texts[0].split_inclusive('\n').collect::<Vec<_>>();
      let changed = texts[1].split_inclusive('\n').collect::<Vec<_>>();
      let mut rebuilt = String::new();
      let mut cursor = 0;
      for edit in edits(&base, &changed).unwrap() {
        rebuilt.extend(base[cursor..edit.start].iter().copied());
        rebuilt.extend(edit.lines);
        cursor = edit.end;
      }
      rebuilt.extend(base[cursor..].iter().copied());
      assert_eq!(rebuilt, texts[1]);
    }
  }

  #[test]
  fn excessive_lines_and_long_unicode_lines_fall_back() {
    let base = "a\n".repeat(100_001);
    assert!(merge_text(&base, &format!("mine\n{base}"), &format!("{base}theirs\n")).is_none());
    let line = "👩‍💻".repeat(40_000);
    assert!(merge_text(&line, &format!("mine{line}"), &format!("{line}theirs")).is_none());
  }
}
