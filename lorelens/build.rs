fn main() {
  println!("cargo:rerun-if-changed=dist/favicon.ico");
  println!("cargo:rerun-if-changed=themes");

  let entries = "  include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/themes/lorelens.json\")),\n";
  let generated = format!("pub const BUNDLED_THEMES: &[&str] = &[\n{entries}];\n");
  let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR must be set")).join("bundled_themes.rs");
  std::fs::write(output, generated).expect("bundled theme catalog must be writable");

  #[cfg(windows)]
  {
    use std::path::PathBuf;
    // GPUI's debug UI construction can exhaust the Windows default 1 MiB
    // main-thread stack when opening the conflict dialog after login.
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_env == "msvc" {
      println!("cargo:rustc-link-arg-bin=lorelens=/STACK:16777216");
    } else if target_env == "gnu" {
      println!("cargo:rustc-link-arg-bin=lorelens=-Wl,--stack,16777216");
    }
    let ico = PathBuf::from("dist").join("favicon.ico");

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(ico.to_str().expect("icon path is not valid UTF-8"));
    resource.compile().expect("failed to compile Windows application icon");
  }
}
