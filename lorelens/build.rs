fn main() {
  println!("cargo:rerun-if-changed=dist/favicon.ico");

  #[cfg(windows)]
  {
    use std::path::PathBuf;
    let ico = PathBuf::from("dist").join("favicon.ico");

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(ico.to_str().expect("icon path is not valid UTF-8"));
    resource.compile().expect("failed to compile Windows application icon");
  }
}
