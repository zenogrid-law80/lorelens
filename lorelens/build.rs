fn main() {
  println!("cargo:rerun-if-changed=dist/favicon.ico");
  println!("cargo:rerun-if-changed=themes");

  let mut themes = std::fs::read_dir("themes")
    .expect("themes directory must be readable")
    .filter_map(Result::ok)
    .map(|entry| entry.path())
    .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("json"))
    .collect::<Vec<_>>();
  themes.sort();
  let entries = themes
    .iter()
    .map(|path| {
      let name = path.file_name().expect("theme path must have a file name").to_string_lossy();
      format!("  include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/themes/{name}\")),\n")
    })
    .collect::<String>();
  let generated = format!("pub const BUNDLED_THEMES: &[&str] = &[\n{entries}];\n");
  let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR must be set")).join("bundled_themes.rs");
  std::fs::write(output, generated).expect("bundled theme catalog must be writable");

  #[cfg(windows)]
  {
    use std::path::PathBuf;
    let ico = PathBuf::from("dist").join("favicon.ico");

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(ico.to_str().expect("icon path is not valid UTF-8"));
    resource.compile().expect("failed to compile Windows application icon");
  }
}
