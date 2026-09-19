use super::ignore::{load_loreignore, loreignored};
use super::*;

/// Validate configured text files before staging without modifying them.
pub fn validate_text_files(root: &Path, paths: &[String], extensions: &[String], line_ending: &str, encoding: &str) -> Result<(), String> {
  if extensions.is_empty() || (line_ending == "System" && encoding == "System") {
    return Ok(());
  }
  let root = root.canonicalize().map_err(|e| e.to_string())?;
  for relative in paths {
    let relative = Path::new(relative);
    if relative.is_absolute() || relative.components().any(|component| !matches!(component, std::path::Component::Normal(_))) {
      return Err(format!("Invalid staging path: {}", relative.display()));
    }
    let path = root.join(relative);
    let metadata = match fs::symlink_metadata(&path) {
      Ok(metadata) => metadata,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
      Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
      continue;
    }
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()).map(str::to_ascii_lowercase) else {
      continue;
    };
    if !extensions.iter().any(|configured| configured.eq_ignore_ascii_case(&extension)) {
      continue;
    }
    let canonical = path.canonicalize().map_err(|e| format!("{}: {e}", path.display()))?;
    if !canonical.starts_with(&root) {
      return Err(format!("Staging path is outside the repository: {}", path.display()));
    }
    let bytes = fs::read(&canonical).map_err(|e| format!("{}: {e}", path.display()))?;
    let valid_encoding = match encoding {
      "UTF-8" => std::str::from_utf8(&bytes).is_ok(),
      "UTF-8 no BOM" => !bytes.starts_with(&[0xef, 0xbb, 0xbf]) && std::str::from_utf8(&bytes).is_ok(),
      _ => true,
    };
    if !valid_encoding {
      return Err(crate::i18n::tf(
        "Stage blocked: {path} does not match the required encoding {encoding}.",
        &[("path", relative.display().to_string()), ("encoding", encoding.into())],
      ));
    }
    let valid_line_ending = match line_ending {
      "LF" => !bytes.contains(&b'\r'),
      "CR" => !bytes.contains(&b'\n'),
      "CRLF" => bytes.iter().enumerate().all(|(index, byte)| match byte {
        b'\r' => bytes.get(index + 1) == Some(&b'\n'),
        b'\n' => index > 0 && bytes[index - 1] == b'\r',
        _ => true,
      }),
      _ => true,
    };
    if !valid_line_ending {
      return Err(crate::i18n::tf(
        "Stage blocked: {path} does not use the required {line_ending} line endings.",
        &[("path", relative.display().to_string()), ("line_ending", line_ending.into())],
      ));
    }
  }
  Ok(())
}

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
  if !destination.is_dir()
    || relative
      .components()
      .any(|part| [".git", ".lore", ".urc"].iter().any(|name| part.as_os_str().eq_ignore_ascii_case(name)))
  {
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
  if link {
    return Err("Deleting symbolic links or junctions is not supported.".into());
  }
  let source = source.canonicalize().map_err(|e| e.to_string())?;
  let relative = source.strip_prefix(&root).map_err(|_| "Delete must stay inside the repository.")?;
  if relative.as_os_str().is_empty()
    || relative
      .components()
      .any(|part| [".git", ".lore", ".urc"].iter().any(|name| part.as_os_str().eq_ignore_ascii_case(name)))
  {
    return Err("Cannot delete repository root or metadata.".into());
  }
  // remove_dir_all removes links within the tree without following their targets.
  if metadata.is_dir() { fs::remove_dir_all(&source) } else { fs::remove_file(&source) }.map_err(|e| format!("Delete failed: {e}"))
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
mod line_ending_tests {
  use super::*;

  #[test]
  fn validates_only_configured_extensions_without_modifying_files() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("nested")).unwrap();
    fs::write(root.path().join("nested/text.txt"), b"first\r\nsecond\r\n").unwrap();
    fs::write(root.path().join("ignored.rs"), b"keep\n").unwrap();

    let paths = vec!["nested/text.txt".into(), "ignored.rs".into(), "deleted.txt".into()];
    assert!(validate_text_files(root.path(), &paths, &["txt".into()], "CRLF", "UTF-8 no BOM").is_ok());
    assert!(validate_text_files(root.path(), &paths, &["txt".into()], "LF", "UTF-8").is_err());
    assert_eq!(fs::read(root.path().join("nested/text.txt")).unwrap(), b"first\r\nsecond\r\n");
  }

  #[test]
  fn rejects_bom_and_invalid_utf8_when_requested() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("bom.txt"), b"\xef\xbb\xbftext\n").unwrap();
    fs::write(root.path().join("invalid.txt"), [0xff]).unwrap();
    assert!(validate_text_files(root.path(), &["bom.txt".into()], &["txt".into()], "System", "UTF-8").is_ok());
    assert!(validate_text_files(root.path(), &["bom.txt".into()], &["txt".into()], "System", "UTF-8 no BOM").is_err());
    assert!(validate_text_files(root.path(), &["invalid.txt".into()], &["txt".into()], "System", "UTF-8").is_err());
  }

  #[test]
  fn rejects_paths_outside_the_repository() {
    let root = tempfile::tempdir().unwrap();
    assert!(validate_text_files(root.path(), &["../outside.txt".into()], &["txt".into()], "LF", "UTF-8").is_err());
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
    if [".git", ".lore", "target"].iter().any(|name| entry.file_name() == *name) {
      continue;
    }
    let meta = entry.metadata().map_err(|e| e.to_string())?;
    let path = entry.path();
    let relative = path.strip_prefix(root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
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
    return Ok("Binary file — text preview unavailable. Use File history to inspect revisions.".into());
  }
  let mut text = String::from_utf8_lossy(&bytes).into_owned();
  if truncated {
    text.push_str("\n\n[Preview limited to 128 KiB]");
  }
  Ok(text)
}

pub fn is_spreadsheet_path(path: &Path) -> bool {
  path
    .extension()
    .is_some_and(|extension| extension.eq_ignore_ascii_case("xlsx") || extension.eq_ignore_ascii_case("xlsm"))
}

pub fn is_unreal_asset_path(path: &Path) -> bool {
  path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("uasset"))
}

pub fn preview_unreal_asset(root: &Path, path: &Path) -> Result<String, String> {
  let canonical_root = root.canonicalize().map_err(|error| error.to_string())?;
  let canonical_path = path.canonicalize().map_err(|error| error.to_string())?;
  if !canonical_path.starts_with(&canonical_root) || !canonical_path.is_file() {
    return Err("Invalid Unreal asset path".into());
  }
  let bytes = fs::read(&canonical_path).map_err(|error| error.to_string())?;
  let read_u32 = |offset: usize| bytes.get(offset..offset + 4).map(|value| u32::from_le_bytes(value.try_into().unwrap()));
  let read_i32 = |offset: usize| read_u32(offset).map(|value| i32::from_le_bytes(value.to_le_bytes()));
  let mut output = format!(
    "Unreal Engine Asset\n\nPath: {}\nSize: {} bytes\n",
    canonical_path.strip_prefix(&canonical_root).unwrap_or(&canonical_path).display(),
    bytes.len()
  );
  let magic = read_u32(0);
  output.push_str(&format!("\nPackage header\nMagic: {}\n", magic.map_or("Unavailable".into(), |value| format!("0x{value:08X}"))));
  output.push_str(&format!("Valid Unreal package: {}\n", magic == Some(0x9E2A83C1)));
  if magic == Some(0x9E2A83C1) {
    output.push_str(&format!("Legacy file version: {}\n", read_i32(4).map_or("Unavailable".into(), |value| value.to_string())));
    output.push_str(&format!("Legacy UE3 version: {}\n", read_i32(8).map_or("Unavailable".into(), |value| value.to_string())));
    output.push_str(&format!("UE4 file version: {}\n", read_i32(12).map_or("Unavailable".into(), |value| value.to_string())));
    output.push_str(&format!("Package flags: {}\n", read_u32(16).map_or("Unavailable".into(), |value| format!("0x{value:08X}"))));
  }
  output.push_str("\nRelated files\n");
  let mut related = false;
  for extension in ["uexp", "ubulk"] {
    let sibling = canonical_path.with_extension(extension);
    if sibling.is_file() {
      let size = fs::metadata(&sibling).map(|metadata| metadata.len()).unwrap_or(0);
      output.push_str(&format!("{}: {} bytes\n", sibling.file_name().unwrap_or_default().to_string_lossy(), size));
      related = true;
    }
  }
  if !related {
    output.push_str("None found\n");
  }
  output.push_str("\nThis is a read-only metadata preview. Asset contents and 3D rendering are not decoded.");
  Ok(output)
}

pub struct SpreadsheetSheet {
  pub name: String,
  pub first_row: usize,
  pub first_column: usize,
  pub total_rows: usize,
  pub total_columns: usize,
  pub cells: Vec<Vec<String>>,
}

pub fn preview_spreadsheet(path: &Path) -> Result<Vec<SpreadsheetSheet>, String> {
  use calamine::Reader;

  let mut workbook = calamine::open_workbook_auto(path).map_err(|error| format!("{}: {error:?}", path.display()))?;
  let mut sheets = Vec::new();
  for name in workbook.sheet_names() {
    let range = workbook.worksheet_range(&name).map_err(|error| format!("{name}: {error:?}"))?;
    let (rows, columns) = range.get_size();
    let (first_row, first_column) = range.start().map(|(row, column)| (row as usize, column as usize)).unwrap_or((0, 0));
    let cells = range.rows().take(200).map(|row| row.iter().take(30).map(ToString::to_string).collect()).collect();
    sheets.push(SpreadsheetSheet {
      name,
      first_row,
      first_column,
      total_rows: rows,
      total_columns: columns,
      cells,
    });
  }
  if sheets.is_empty() {
    return Err("Workbook contains no worksheets".into());
  }
  Ok(sheets)
}

pub fn render_fbx_preview(path: &Path) -> Result<PathBuf, String> {
  use std::io::Write;
  use threers::{AmbientLight, Color, DirectionalLight, FbxLoader, HeadlessRenderer, Material, Mesh, Object3D, PerspectiveCamera, Scene, StandardMaterial, Vector3};

  let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
  let mut geometry = FbxLoader::parse(&bytes).map_err(|error| format!("FBX parse failed: {error:?}"))?;
  let sphere = geometry.compute_bounding_sphere();
  if sphere.is_empty() || !sphere.radius.is_finite() || sphere.radius <= 0.0 {
    return Err("FBX contains no renderable geometry".into());
  }
  let mut material = StandardMaterial::new(Color::from_hex(0x6ea8fe));
  material.roughness = 0.7;
  material.metalness = 0.05;
  let mut scene = Scene::new();
  scene.background = Color::from_hex(0x171a21);
  scene.add(Object3D::mesh(Mesh::new(geometry, Material::Standard(material))));
  scene.add_light(AmbientLight::new(Color::from_hex(0xffffff), 0.45));
  let mut key = Object3D::light(DirectionalLight::new(Color::from_hex(0xffffff), 2.0));
  key.position = Vector3::new(2.0, 3.0, 4.0);
  scene.add(key);
  let width = 640u32;
  let height = 480u32;
  let mut camera = PerspectiveCamera::new(45.0, width as f32 / height as f32, 0.01, sphere.radius * 20.0);
  camera.position = sphere.center + Vector3::new(sphere.radius * 2.8, sphere.radius * 1.8, sphere.radius * 3.6);
  camera.look_at(sphere.center);
  let mut renderer = HeadlessRenderer::builder().size(width, height).build().map_err(|error| format!("FBX renderer unavailable: {error}"))?;
  let rgba = renderer.render_to_rgba_resolved(&mut scene, &camera);
  let png = threers::encode_png(width, height, &rgba);
  let mut output = tempfile::Builder::new().prefix("lorelens-fbx-").suffix(".png").tempfile().map_err(|error| error.to_string())?;
  output.write_all(&png).map_err(|error| error.to_string())?;
  let (_, output_path) = output.keep().map_err(|error| error.error.to_string())?;
  Ok(output_path)
}

pub fn is_image_path(path: &Path) -> bool {
  let Some(extension) = path.extension().and_then(|extension| extension.to_str()).map(str::to_ascii_lowercase) else {
    return false;
  };
  extension != "tga" && gpui::Img::extensions().iter().any(|supported| *supported == extension)
}

pub fn is_fbx_path(path: &Path) -> bool {
  path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("fbx"))
}

pub fn syntax_language(path: &Path) -> Option<&'static str> {
  match path.extension().and_then(|extension| extension.to_str()).map(str::to_ascii_lowercase).as_deref() {
    Some("c" | "h" | "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx") => Some("cpp"),
    Some("cs") => Some("csharp"),
    Some("rs") => Some("rust"),
    Some("go") => Some("go"),
    Some("java") => Some("java"),
    Some("diff" | "patch") => Some("diff"),
    Some("css") => Some("css"),
    Some("lua") => Some("lua"),
    Some("js" | "jsx" | "mjs" | "cjs") => Some("javascript"),
    Some("json" | "uproject" | "modules" | "target" | "uplugin") => Some("json"),
    Some("sql") => Some("sql"),
    Some("html" | "htm") => Some("html"),
    Some("php") => Some("php"),
    Some("md") => Some("markdown"),
    Some("proto") => Some("proto"),
    Some("py") => Some("python"),
    Some("ini" | "toml") => Some("toml"),
    Some("yml" | "yaml") => Some("yaml"),
    Some("rb") => Some("ruby"),
    Some("zig") => Some("zig"),
    Some("fbx") => Some("text"),
    _ => None,
  }
}

#[cfg(test)]
mod preview_tests {
  use super::{is_image_path, is_spreadsheet_path, is_unreal_asset_path, preview_unreal_asset};
  use std::path::Path;

  #[test]
  fn recognizes_supported_image_extensions_case_insensitively() {
    for extension in ["png", "JPG", "jpeg", "gif", "webp", "tiff", "svg", "qoi"] {
      assert!(is_image_path(Path::new(&format!("image.{extension}"))));
    }
    assert!(!is_image_path(Path::new("image.tga")));
    assert!(!is_image_path(Path::new("image.TGA")));
    assert!(!is_image_path(Path::new("image.txt")));
    assert!(!is_image_path(Path::new("image")));
  }

  #[test]
  fn recognizes_supported_spreadsheet_extensions_case_insensitively() {
    assert!(is_spreadsheet_path(Path::new("workbook.XLSX")));
    assert!(is_spreadsheet_path(Path::new("macro-workbook.XLSM")));
    assert!(!is_spreadsheet_path(Path::new("workbook.xls")));
  }

  #[test]
  fn previews_unreal_asset_header_and_related_files() {
    let directory = tempfile::tempdir().unwrap();
    let asset = directory.path().join("Example.uasset");
    let mut bytes = vec![0; 20];
    bytes[0..4].copy_from_slice(&0x9E2A83C1u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&(-7i32).to_le_bytes());
    std::fs::write(&asset, bytes).unwrap();
    std::fs::write(directory.path().join("Example.uexp"), b"related").unwrap();
    assert!(is_unreal_asset_path(&asset));
    let preview = preview_unreal_asset(directory.path(), &asset).unwrap();
    assert!(preview.contains("Valid Unreal package: true"));
    assert!(preview.contains("Example.uexp: 7 bytes"));
  }

  #[test]
  fn maps_supported_source_extensions_to_languages() {
    assert_eq!(super::syntax_language(Path::new("main.CPP")), Some("cpp"));
    assert_eq!(super::syntax_language(Path::new("Program.CS")), Some("csharp"));
    assert_eq!(super::syntax_language(Path::new("lib.rs")), Some("rust"));
    assert_eq!(super::syntax_language(Path::new("app.JSX")), Some("javascript"));
    assert_eq!(super::syntax_language(Path::new("config.JSON")), Some("json"));
    assert_eq!(super::syntax_language(Path::new("game.UPROJECT")), Some("json"));
    assert_eq!(super::syntax_language(Path::new("settings.INI")), Some("toml"));
    assert_eq!(super::syntax_language(Path::new("config.TOML")), Some("toml"));
    assert_eq!(super::syntax_language(Path::new("scene.FBX")), Some("text"));
    assert_eq!(super::syntax_language(Path::new("notes.txt")), None);
  }

  #[test]
  fn maps_csharp_to_the_registered_highlighter_name() {
    assert_eq!(super::syntax_language(Path::new("Program.cs")), Some("csharp"));
  }
}
