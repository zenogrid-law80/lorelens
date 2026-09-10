use serde_json::{Value, json};
use std::{
  fs, io,
  path::{Path, PathBuf},
};

const DEFAULT_TEXT_EXTENSIONS: &[&str] = &["h", "cpp", "hpp", "cc", "cs", "rs", "go", "rb", "py", "xml", "json", "txt", "ini"];

fn default_text_extensions() -> Vec<String> {
  DEFAULT_TEXT_EXTENSIONS.iter().map(|extension| (*extension).into()).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bookmark {
  pub root: PathBuf,
  pub path: PathBuf,
}

pub struct Settings {
  pub shortcuts: std::collections::BTreeMap<String, String>,
  pub login_remote: Option<String>,
  pub login_urls: Vec<String>,
  pub recent: Vec<PathBuf>,
  pub bookmarks: Vec<Bookmark>,
  pub cli: Option<PathBuf>,
  pub theme: String,
  pub show_command_log: bool,
  pub auto_refresh: bool,
  pub text_line_ending: String,
  pub text_encoding: String,
  pub text_extensions: Vec<String>,
  pub language: String,
  pub external_tool: String,
  pub tool_paths: std::collections::BTreeMap<String, PathBuf>,
  pub custom_tool_name: String,
  pub custom_tool_path: PathBuf,
  pub custom_tool_arguments: String,
  pub identity: Option<String>,
  pub create_url: String,
  pub create_destination: String,
  pub create_urls: Vec<String>,
  pub create_destinations: Vec<String>,
  pub clone_url: String,
  pub clone_destination: String,
  pub clone_urls: Vec<String>,
  pub clone_destinations: Vec<String>,
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      shortcuts: Default::default(),
      login_remote: None,
      login_urls: Vec::new(),
      recent: Vec::new(),
      bookmarks: Vec::new(),
      cli: None,
      theme: crate::theme::DEFAULT_THEME.into(),
      show_command_log: true,
      auto_refresh: true,
      text_line_ending: "LF".into(),
      text_encoding: "UTF-8 no BOM".into(),
      text_extensions: default_text_extensions(),
      language: "en-US".into(),
      external_tool: "idea".into(),
      tool_paths: Default::default(),
      custom_tool_name: "Custom".into(),
      custom_tool_path: PathBuf::new(),
      custom_tool_arguments: "{base} {yours}".into(),
      identity: None,
      create_url: String::new(),
      create_destination: String::new(),
      create_urls: Vec::new(),
      create_destinations: Vec::new(),
      clone_url: String::new(),
      clone_destination: String::new(),
      clone_urls: Vec::new(),
      clone_destinations: Vec::new(),
    }
  }
}

impl Settings {
  fn normalize_recent_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    if let Some(text) = path.to_str() {
      let text = text.replace('/', "\\");
      if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
      }
      if let Some(rest) = text.strip_prefix(r"\\?\") {
        // Only strip extended drive-path prefixes, not other device paths.
        if rest.as_bytes().get(1) == Some(&b':') {
          return PathBuf::from(rest);
        }
      }
      return PathBuf::from(text);
    }
    path.to_path_buf()
  }

  fn recent_key(path: &Path) -> PathBuf {
    #[cfg(windows)]
    if let Some(text) = path.to_str() {
      return PathBuf::from(text.to_ascii_lowercase());
    }
    path.to_path_buf()
  }

  fn normalized_recent(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    paths
      .into_iter()
      .map(|path| Self::normalize_recent_path(&path))
      .filter(|path| seen.insert(Self::recent_key(path)))
      .take(10)
      .collect()
  }

  fn normalized_bookmarks(bookmarks: impl IntoIterator<Item = Bookmark>) -> Vec<Bookmark> {
    let mut seen = std::collections::HashSet::new();
    bookmarks
      .into_iter()
      .filter_map(|bookmark| {
        let root = Self::normalize_recent_path(&bookmark.root);
        let path = bookmark.path;
        let safe = !path.as_os_str().is_empty() && !path.is_absolute() && path.components().all(|component| matches!(component, std::path::Component::Normal(_)));
        safe.then_some(Bookmark { root, path })
      })
      .filter(|bookmark| seen.insert((Self::recent_key(&bookmark.root), Self::recent_key(&bookmark.path))))
      .collect()
  }
  /// Older settings stored login input history but no successful-login URL.
  /// Only recover an unambiguous destination; history is not proof of authentication.
  pub fn clone_remote(&self) -> Option<String> {
    if let Some(remote) = self.login_remote.as_deref().map(str::trim).filter(|url| !url.is_empty()) {
      return Some(remote.into());
    }
    let mut candidates = self.login_urls.iter().map(|url| url.trim()).filter(|url| !url.is_empty());
    let remote = candidates.next()?;
    if candidates.any(|other| other != remote) {
      return None;
    }
    Some(remote.into())
  }

  pub fn path() -> PathBuf {
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
      PathBuf::from(base).join("LoreLens/settings.json")
    } else {
      std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"))
        .join("lorelens/settings.json")
    }
  }

  pub fn load(path: &Path) -> io::Result<Self> {
    let text = match fs::read_to_string(path) {
      Ok(text) => text,
      Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
      Err(e) => return Err(e),
    };
    let data: Value = serde_json::from_str(&text)?;
    let recent: Vec<PathBuf> = serde_json::from_value(data.get("recent").cloned().unwrap_or(json!([])))?;
    let bookmarks: Vec<Bookmark> = data["bookmarks"]
      .as_array()
      .into_iter()
      .flatten()
      .filter_map(|bookmark| {
        Some(Bookmark {
          root: PathBuf::from(bookmark["root"].as_str()?),
          path: PathBuf::from(bookmark["path"].as_str()?),
        })
      })
      .collect();
    let mut shortcuts: std::collections::BTreeMap<String, String> = serde_json::from_value(data.get("shortcuts").cloned().unwrap_or(json!({})))?;
    // Ctrl+H was the old File history default. It now opens a command window,
    // so migrate settings saved before the terminal shortcut was introduced.
    if shortcuts.get("history").is_some_and(|shortcut| shortcut.eq_ignore_ascii_case("Ctrl+H")) && !shortcuts.contains_key("terminal") {
      shortcuts.insert("history".into(), "Ctrl+Shift+H".into());
    }
    let cli = serde_json::from_value(data.get("cli").cloned().unwrap_or(Value::Null))?;
    Ok(Self {
      shortcuts,
      login_remote: data["login_remote"].as_str().filter(|url| !url.trim().is_empty()).map(str::to_owned),
      login_urls: serde_json::from_value(data.get("login_urls").cloned().unwrap_or(json!([])))?,
      recent: Self::normalized_recent(recent),
      bookmarks: Self::normalized_bookmarks(bookmarks),
      cli,
      language: crate::i18n::normalize(data["language"].as_str().unwrap_or("en-US")).into(),
      theme: data["theme"].as_str().unwrap_or(crate::theme::DEFAULT_THEME).to_string(),
      show_command_log: data["show_command_log"].as_bool().unwrap_or(true),
      auto_refresh: data["auto_refresh"].as_bool().unwrap_or(true),
      text_line_ending: match data["text_line_ending"].as_str().unwrap_or("LF") {
        value @ ("LF" | "CR" | "CRLF") => value.into(),
        _ => "LF".into(),
      },
      text_encoding: match data["text_encoding"].as_str().unwrap_or("UTF-8 no BOM") {
        value @ ("UTF-8" | "UTF-8 no BOM") => value.into(),
        _ => "UTF-8 no BOM".into(),
      },
      text_extensions: Self::normalize_extensions(serde_json::from_value(data.get("text_extensions").cloned().unwrap_or_else(|| json!(default_text_extensions())))?),
      external_tool: data["external_tool"]
        .as_str()
        .or_else(|| data["diff_tool"].as_str())
        .or_else(|| data["merge_tool"].as_str())
        .unwrap_or("idea")
        .into(),
      tool_paths: serde_json::from_value(data.get("tool_paths").cloned().unwrap_or(json!({})))?,
      custom_tool_name: data["custom_tool_name"].as_str().unwrap_or("Custom").into(),
      custom_tool_path: serde_json::from_value(data.get("custom_tool_path").cloned().unwrap_or(json!("")))?,
      custom_tool_arguments: data["custom_tool_arguments"].as_str().unwrap_or("{base} {yours}").into(),
      identity: data["identity"].as_str().filter(|id| !id.is_empty()).map(str::to_string),
      create_url: data["create_url"].as_str().unwrap_or_default().into(),
      create_destination: data["create_destination"].as_str().unwrap_or_default().into(),
      create_urls: serde_json::from_value(data.get("create_urls").cloned().unwrap_or(json!([])))?,
      create_destinations: serde_json::from_value(data.get("create_destinations").cloned().unwrap_or(json!([])))?,
      clone_url: data["clone_url"].as_str().unwrap_or_default().into(),
      clone_destination: data["clone_destination"].as_str().unwrap_or_default().into(),
      clone_urls: serde_json::from_value(data.get("clone_urls").cloned().unwrap_or(json!([])))?,
      clone_destinations: serde_json::from_value(data.get("clone_destinations").cloned().unwrap_or(json!([])))?,
    })
  }

  pub fn remember(&mut self, path: &Path) {
    self.recent = Self::normalized_recent(std::iter::once(path.to_path_buf()).chain(self.recent.iter().cloned()));
  }

  pub fn normalize_extensions(extensions: Vec<String>) -> Vec<String> {
    let mut extensions: Vec<_> = extensions
      .into_iter()
      .flat_map(|value| value.split([',', ';']).map(str::to_owned).collect::<Vec<_>>())
      .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
      .filter(|value| !value.is_empty() && value.chars().all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '+')))
      .collect();
    extensions.sort();
    extensions.dedup();
    extensions
  }

  pub fn prune_recent(&mut self) -> bool {
    let before = self.recent.len();
    self.recent.retain(|path| match fs::metadata(path) {
      Ok(metadata) => metadata.is_dir(),
      Err(error) => error.kind() != io::ErrorKind::NotFound,
    });
    self.recent.len() != before
  }

  pub fn is_bookmarked(&self, root: &Path, path: &Path) -> bool {
    let root = Self::normalize_recent_path(root);
    let Some(path) = path.strip_prefix(&root).ok().filter(|path| !path.as_os_str().is_empty()) else {
      return false;
    };
    let key = (Self::recent_key(&root), Self::recent_key(path));
    self.bookmarks.iter().any(|bookmark| (Self::recent_key(&bookmark.root), Self::recent_key(&bookmark.path)) == key)
  }

  pub fn toggle_bookmark(&mut self, root: &Path, path: &Path) -> bool {
    let root = Self::normalize_recent_path(root);
    let Some(path) = path.strip_prefix(&root).ok().filter(|path| !path.as_os_str().is_empty()).map(Path::to_path_buf) else {
      return false;
    };
    let key = (Self::recent_key(&root), Self::recent_key(&path));
    if let Some(index) = self.bookmarks.iter().position(|bookmark| (Self::recent_key(&bookmark.root), Self::recent_key(&bookmark.path)) == key) {
      self.bookmarks.remove(index);
      false
    } else {
      self.bookmarks.push(Bookmark { root, path });
      true
    }
  }

  pub fn bookmarks_for(&self, root: &Path) -> Vec<PathBuf> {
    let root = Self::normalize_recent_path(root);
    let key = Self::recent_key(&root);
    self
      .bookmarks
      .iter()
      .filter(|bookmark| Self::recent_key(&bookmark.root) == key && root.join(&bookmark.path).exists())
      .map(|bookmark| bookmark.path.clone())
      .collect()
  }

  pub fn replace_bookmarks(&mut self, bookmarks: Vec<Bookmark>) {
    self.bookmarks = Self::normalized_bookmarks(bookmarks);
  }

  pub fn prune_bookmarks(&mut self) -> bool {
    let before = self.bookmarks.len();
    self.bookmarks.retain(|bookmark| match fs::metadata(bookmark.root.join(&bookmark.path)) {
      Ok(_) => true,
      Err(error) => error.kind() != io::ErrorKind::NotFound,
    });
    self.bookmarks.len() != before
  }

  pub fn remember_create(&mut self, url: &str, destination: &str) {
    self.create_url = url.trim().into();
    self.create_destination = destination.trim().into();
    for (history, value) in [(&mut self.create_urls, &self.create_url), (&mut self.create_destinations, &self.create_destination)] {
      if value.is_empty() {
        continue;
      }
      history.retain(|entry| entry != value);
      history.insert(0, value.clone());
      history.truncate(10);
    }
  }

  pub fn remember_clone(&mut self) {
    for (history, value) in [(&mut self.clone_urls, &self.clone_url), (&mut self.clone_destinations, &self.clone_destination)] {
      let value = value.trim();
      if value.is_empty() {
        continue;
      }
      history.retain(|entry| entry != value);
      history.insert(0, value.into());
      history.truncate(10);
    }
  }

  pub fn restore(&self) -> Option<PathBuf> {
    self.recent.iter().find(|path| path.is_dir()).cloned()
  }

  pub fn remember_login(&mut self, url: &str) {
    let url = url.trim();
    if url.is_empty() {
      return;
    }
    self.login_urls.retain(|entry| entry != url);
    self.login_urls.insert(0, url.into());
    self.login_urls.truncate(10);
  }

  pub fn save(&self, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = fs::File::create(&temp)?;
    let bookmarks: Vec<_> = Self::normalized_bookmarks(self.bookmarks.clone())
      .into_iter()
      .map(|bookmark| json!({"root": bookmark.root, "path": bookmark.path}))
      .collect();
    serde_json::to_writer_pretty(
      &mut file,
      &json!({"shortcuts": self.shortcuts, "login_remote": self.login_remote, "login_urls": self.login_urls, "language": self.language, "tool_paths": self.tool_paths, "external_tool": self.external_tool, "custom_tool_name": self.custom_tool_name, "custom_tool_path": self.custom_tool_path, "custom_tool_arguments": self.custom_tool_arguments, "recent": Self::normalized_recent(self.recent.clone()), "bookmarks": bookmarks, "cli": self.cli, "theme": self.theme, "show_command_log": self.show_command_log, "auto_refresh": self.auto_refresh, "text_line_ending": self.text_line_ending, "text_encoding": self.text_encoding, "text_extensions": Self::normalize_extensions(self.text_extensions.clone()), "identity": self.identity, "create_url": self.create_url, "create_destination": self.create_destination, "create_urls": self.create_urls, "create_destinations": self.create_destinations, "clone_url": self.clone_url, "clone_destination": self.clone_destination, "clone_urls": self.clone_urls, "clone_destinations": self.clone_destinations}),
    )?;
    file.sync_all()?;
    drop(file);
    fs::rename(temp, path)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn auto_refresh_survives_restart_and_defaults_to_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    assert!(Settings::load(&path).unwrap().auto_refresh);
    fs::write(&path, "{}").unwrap();
    assert!(Settings::load(&path).unwrap().auto_refresh);

    for enabled in [false, true] {
      let mut settings = Settings::load(&path).unwrap();
      settings.auto_refresh = enabled;
      settings.save(&path).unwrap();
      assert_eq!(Settings::load(&path).unwrap().auto_refresh, enabled);
    }
  }

  #[test]
  fn command_log_visibility_survives_restart_and_defaults_to_visible() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    assert!(Settings::load(&path).unwrap().show_command_log);

    let mut settings = Settings::default();
    settings.show_command_log = false;
    settings.save(&path).unwrap();
    assert!(!Settings::load(&path).unwrap().show_command_log);
  }

  #[test]
  fn text_file_rules_survive_restart_and_use_requested_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let defaults = Settings::load(&path).unwrap();
    assert_eq!(defaults.text_line_ending, "LF");
    assert_eq!(defaults.text_encoding, "UTF-8 no BOM");
    assert_eq!(defaults.text_extensions, ["h", "cpp", "hpp", "cc", "cs", "rs", "go", "rb", "py", "xml", "json", "txt", "ini"]);

    let mut settings = Settings::default();
    settings.text_line_ending = "CRLF".into();
    settings.text_encoding = "UTF-8 no BOM".into();
    settings.text_extensions = vec![".RS".into(), "txt, rs".into()];
    settings.save(&path).unwrap();
    let restored = Settings::load(&path).unwrap();
    assert_eq!(restored.text_line_ending, "CRLF");
    assert_eq!(restored.text_encoding, "UTF-8 no BOM");
    assert_eq!(restored.text_extensions, ["rs", "txt"]);
  }

  #[test]
  fn create_history_survives_restart_and_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    fs::write(&path, "{}").unwrap();
    let mut settings = Settings::load(&path).unwrap();
    assert!(settings.create_urls.is_empty());
    assert!(settings.create_destination.is_empty());
    for i in 0..12 {
      settings.remember_create(&format!("lores://server/repo-{i}"), &format!("C:/작업/repo-{i}"));
    }
    settings.remember_create("  lores://server/repo-5  ", " C:/작업/repo-5 ");
    assert_eq!(settings.create_urls.len(), 10);
    assert_eq!(settings.create_destinations.len(), 10);
    assert_eq!(settings.create_urls[0], "lores://server/repo-5");
    assert_eq!(settings.create_destinations[0], "C:/작업/repo-5");
    assert_eq!(settings.create_urls.iter().filter(|url| *url == "lores://server/repo-5").count(), 1);
    settings.save(&path).unwrap();
    let restored = Settings::load(&path).unwrap();
    assert_eq!(restored.create_url, settings.create_url);
    assert_eq!(restored.create_destination, settings.create_destination);
    assert_eq!(restored.create_urls, settings.create_urls);
    assert_eq!(restored.create_destinations, settings.create_destinations);
    settings.remember_create(" ", " ");
    assert_eq!(settings.create_urls, restored.create_urls);
    assert_eq!(settings.create_destinations, restored.create_destinations);
    assert!(settings.clone_urls.is_empty());
  }

  #[test]
  fn pruning_recent_removes_missing_directories_and_files_and_persists() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("repository");
    fs::create_dir(&folder).unwrap();
    let file = root.path().join("plain-file");
    fs::write(&file, "file").unwrap();
    let mut settings = Settings::default();
    settings.recent = vec![root.path().join("missing"), folder.clone(), file];
    assert!(settings.prune_recent());
    assert_eq!(settings.recent, vec![folder]);
    assert!(!settings.prune_recent());
    let path = root.path().join("settings.json");
    settings.save(&path).unwrap();
    assert_eq!(Settings::load(&path).unwrap().recent, settings.recent);
  }

  #[test]
  fn bookmarks_persist_toggle_and_prune_missing_paths() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("bookmarked.txt");
    fs::write(&file, "bookmark").unwrap();
    let missing = root.path().join("missing.txt");
    let mut settings = Settings::default();
    assert!(settings.toggle_bookmark(root.path(), &file));
    assert!(settings.is_bookmarked(root.path(), &file));
    assert!(!settings.toggle_bookmark(root.path(), &file));
    assert!(!settings.is_bookmarked(root.path(), &file));
    assert!(settings.toggle_bookmark(root.path(), &file));
    assert!(settings.toggle_bookmark(root.path(), &missing));
    assert!(settings.prune_bookmarks());
    assert_eq!(settings.bookmarks_for(root.path()), vec![PathBuf::from("bookmarked.txt")]);
    let path = root.path().join("settings.json");
    settings.save(&path).unwrap();
    assert_eq!(Settings::load(&path).unwrap().bookmarks_for(root.path()), vec![PathBuf::from("bookmarked.txt")]);
  }

  #[test]
  fn bookmarks_reject_paths_that_escape_the_repository() {
    let root = tempfile::tempdir().unwrap();
    let settings = Settings {
      bookmarks: vec![
        Bookmark {
          root: root.path().into(),
          path: PathBuf::from("../outside.txt"),
        },
        Bookmark {
          root: root.path().into(),
          path: PathBuf::from("folder/../../outside.txt"),
        },
        Bookmark {
          root: root.path().into(),
          path: PathBuf::from("folder/file.txt"),
        },
      ],
      ..Settings::default()
    };

    let normalized = Settings::normalized_bookmarks(settings.bookmarks);
    assert_eq!(
      normalized,
      vec![Bookmark {
        root: root.path().into(),
        path: PathBuf::from("folder/file.txt")
      }]
    );
  }

  #[test]
  fn migrates_old_history_shortcut_away_from_command_window_shortcut() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("settings.json");
    fs::write(&path, r#"{"shortcuts":{"history":"Ctrl+H","open":"Ctrl+O"}}"#).unwrap();
    let settings = Settings::load(&path).unwrap();
    assert_eq!(settings.shortcuts.get("history").map(String::as_str), Some("Ctrl+Shift+H"));
    assert!(!settings.shortcuts.contains_key("terminal"));

    fs::write(&path, r#"{"shortcuts":{"history":"Ctrl+H","terminal":"Alt+T"}}"#).unwrap();
    let customized = Settings::load(&path).unwrap();
    assert_eq!(customized.shortcuts.get("history").map(String::as_str), Some("Ctrl+H"));
    assert_eq!(customized.shortcuts.get("terminal").map(String::as_str), Some("Alt+T"));
  }
  #[test]
  fn clone_recovers_legacy_login_url_without_guessing_between_servers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    fs::write(&path, r#"{"login_urls":["lores://server:41337"]}"#).unwrap();
    let mut settings = Settings::load(&path).unwrap();
    assert_eq!(settings.clone_remote().as_deref(), Some("lores://server:41337"));
    assert!(settings.login_remote.is_none());
    settings.save(&path).unwrap();
    assert_eq!(Settings::load(&path).unwrap().clone_remote(), settings.clone_remote());
    settings.remember_login("lores://other:41337");
    assert!(settings.clone_remote().is_none());
    settings.login_remote = Some("lores://signed-in:41337".into());
    assert_eq!(settings.clone_remote().as_deref(), Some("lores://signed-in:41337"));
    assert!(Settings::default().clone_remote().is_none());
  }

  #[test]
  fn login_history_survives_restart_and_deduplicates() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut settings = Settings::default();
    for i in 0..12 {
      settings.remember_login(&format!("lores://server-{i}:443"));
    }
    settings.remember_login("  lores://server-5:443  ");
    settings.remember_login(" ");
    assert_eq!(settings.login_urls.len(), 10);
    assert_eq!(settings.login_urls[0], "lores://server-5:443");
    assert_eq!(settings.login_urls.iter().filter(|url| *url == "lores://server-5:443").count(), 1);
    settings.save(&path).unwrap();
    let mut restarted = Settings::load(&path).unwrap();
    assert_eq!(restarted.login_urls, settings.login_urls);
    restarted.theme = "Light".into();
    restarted.save(&path).unwrap();
    assert_eq!(Settings::load(&path).unwrap().login_urls, settings.login_urls);
    fs::write(&path, "{}").unwrap();
    assert!(Settings::load(&path).unwrap().login_urls.is_empty());
  }

  #[test]
  fn remembers_and_restores_across_reloads() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("target/settings-test").join(format!("{}.json", std::process::id()));
    let mut settings = Settings::default();
    settings.remember(&root.join("src"));
    settings.remember(&root);
    settings.remember(&root.join("src"));
    settings.cli = Some(root.join("한글 CLI.exe"));
    settings.theme = "Light".into();
    settings.language = "ko-KR".into();
    settings.external_tool = "p4merge".into();
    settings.tool_paths.insert("rider".into(), root.join("한글 tools/rider64.exe"));
    settings.custom_tool_name = "My Merge".into();
    settings.custom_tool_path = root.join("tools/custom.exe");
    settings.custom_tool_arguments = "--wait {base} {yours}".into();
    settings.clone_url = "lores://example/repo".into();
    settings.clone_destination = "C:/작업/repo".into();
    settings.save(&path).unwrap();
    settings.save(&path).unwrap();
    let restored = Settings::load(&path).unwrap();
    assert_eq!(restored.recent.len(), 2);
    assert_eq!(restored.restore(), Some(root.join("src")));
    assert_eq!(restored.cli, settings.cli);
    assert_eq!(restored.theme, "Light");
    assert_eq!(restored.language, "ko-KR");
    assert_eq!(restored.external_tool, "p4merge");
    assert_eq!(restored.tool_paths, settings.tool_paths);
    assert_eq!(restored.custom_tool_name, settings.custom_tool_name);
    assert_eq!(restored.custom_tool_path, settings.custom_tool_path);
    assert_eq!(restored.custom_tool_arguments, settings.custom_tool_arguments);
    assert_eq!(restored.clone_url, settings.clone_url);
    assert_eq!(restored.clone_destination, settings.clone_destination);
  }
  #[test]
  fn tool_locations_survive_restart_and_later_settings_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut settings = Settings::default();
    for tool in ["idea", "p4merge", "TortoiseGitMerge"] {
      settings.tool_paths.insert(tool.into(), PathBuf::from(format!("C:/한글 Tools/{tool}.exe")));
    }
    settings.external_tool = "rider".into();
    settings.save(&path).unwrap();
    let expected = settings.tool_paths.clone();
    drop(settings);
    let mut restarted = Settings::load(&path).unwrap();
    assert_eq!(restarted.tool_paths, expected);
    assert_eq!(restarted.external_tool, "rider");
    restarted.theme = "Light".into();
    restarted.save(&path).unwrap();
    drop(restarted);
    assert_eq!(Settings::load(&path).unwrap().tool_paths, expected);
  }
  #[test]
  fn migrates_separate_tool_selections_to_shared_tool() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    fs::write(&path, r#"{"diff_tool":"rider","merge_tool":"p4merge"}"#).unwrap();
    let settings = Settings::load(&path).unwrap();
    assert_eq!(settings.external_tool, "rider");
    settings.save(&path).unwrap();
    assert_eq!(Settings::load(&path).unwrap().external_tool, "rider");
    fs::write(&path, r#"{"merge_tool":"p4merge"}"#).unwrap();
    assert_eq!(Settings::load(&path).unwrap().external_tool, "p4merge");
  }
  #[test]
  #[cfg(windows)]
  fn recent_paths_merge_extended_drive_and_unc_spellings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    fs::write(
      &path,
      json!({"recent": [
          r"C:\Lore", r"\\?\C:\Lore", r"c:/lore",
          r"\\?\UNC\server\share\repo", r"\\server\share\repo",
          r"C:\Other"
      ]})
      .to_string(),
    )
    .unwrap();
    let mut settings = Settings::load(&path).unwrap();
    assert_eq!(settings.recent, vec![PathBuf::from(r"C:\Lore"), PathBuf::from(r"\\server\share\repo"), PathBuf::from(r"C:\Other")]);
    settings.remember(Path::new(r"\\?\C:\Other"));
    assert_eq!(settings.recent[0], PathBuf::from(r"C:\Other"));
    assert_eq!(settings.recent.len(), 3);
    settings.save(&path).unwrap();
    assert_eq!(Settings::load(&path).unwrap().recent, settings.recent);
  }

  #[test]
  fn bounds_history_and_skips_missing_directories() {
    let mut settings = Settings::default();
    settings.remember(Path::new(env!("CARGO_MANIFEST_DIR")));
    settings.remember(Path::new("missing-lorelens-repository"));
    assert_eq!(settings.restore(), Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))));
    for i in 0..20 {
      settings.remember(Path::new(&format!("repo-{i}")));
    }
    assert_eq!(settings.recent.len(), 10);
  }
}
