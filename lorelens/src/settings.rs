use serde_json::{Value, json};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub struct Settings {
    pub login_urls: Vec<String>,
    pub recent: Vec<PathBuf>,
    pub cli: Option<PathBuf>,
    pub theme: String,
    pub language: String,
    pub external_tool: String,
    pub tool_paths: std::collections::BTreeMap<String, PathBuf>,
    pub identity: Option<String>,
    pub clone_url: String,
    pub clone_destination: String,
    pub clone_urls: Vec<String>,
    pub clone_destinations: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { login_urls: Vec::new(), recent: Vec::new(), cli: None, theme: "System".into(), language: "en-US".into(), external_tool: "idea".into(), tool_paths: Default::default(), identity: None, clone_url: String::new(), clone_destination: String::new(), clone_urls: Vec::new(), clone_destinations: Vec::new() }
    }
}

impl Settings {
    pub fn path() -> PathBuf {
        if let Some(base) = std::env::var_os("LOCALAPPDATA") {
            PathBuf::from(base).join("LoreLens/settings.json")
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
                })
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
        let recent: Vec<PathBuf> =
            serde_json::from_value(data.get("recent").cloned().unwrap_or(json!([])))?;
        let cli = serde_json::from_value(data.get("cli").cloned().unwrap_or(Value::Null))?;
        Ok(Self {
            login_urls: serde_json::from_value(data.get("login_urls").cloned().unwrap_or(json!([])))?,
            recent: recent.into_iter().take(10).collect(),
            cli,
            language: crate::i18n::normalize(data["language"].as_str().unwrap_or("en-US")).into(),
            theme: data["theme"].as_str().unwrap_or("System").to_string(),
            external_tool: data["external_tool"].as_str()
                .or_else(|| data["diff_tool"].as_str())
                .or_else(|| data["merge_tool"].as_str())
                .unwrap_or("idea").into(),
            tool_paths: serde_json::from_value(data.get("tool_paths").cloned().unwrap_or(json!({})))?,
            identity: data["identity"].as_str().filter(|id| !id.is_empty()).map(str::to_string),
            clone_url: data["clone_url"].as_str().unwrap_or_default().into(),
            clone_destination: data["clone_destination"].as_str().unwrap_or_default().into(),
            clone_urls: serde_json::from_value(data.get("clone_urls").cloned().unwrap_or(json!([])))?,
            clone_destinations: serde_json::from_value(data.get("clone_destinations").cloned().unwrap_or(json!([])))?,
        })
    }

    pub fn remember(&mut self, path: &Path) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(10);
    }

    pub fn remember_clone(&mut self) {
        for (history, value) in [(&mut self.clone_urls, &self.clone_url), (&mut self.clone_destinations, &self.clone_destination)] {
            let value = value.trim();
            if value.is_empty() { continue; }
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
        if url.is_empty() { return; }
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
        serde_json::to_writer_pretty(&mut file, &json!({"login_urls": self.login_urls, "language": self.language, "tool_paths": self.tool_paths, "external_tool": self.external_tool, "recent": self.recent, "cli": self.cli, "theme": self.theme, "identity": self.identity, "clone_url": self.clone_url, "clone_destination": self.clone_destination, "clone_urls": self.clone_urls, "clone_destinations": self.clone_destinations}))?;
        file.sync_all()?;
        drop(file);
        fs::rename(temp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let path = root
            .join("target/settings-test")
            .join(format!("{}.json", std::process::id()));
        let mut settings = Settings::default();
        settings.remember(&root.join("src"));
        settings.remember(&root);
        settings.remember(&root.join("src"));
        settings.cli = Some(root.join("한글 CLI.exe"));
        settings.theme = "Light".into();
        settings.language = "ko-KR".into();
        settings.external_tool = "p4merge".into();
        settings.tool_paths.insert("rider".into(), root.join("한글 tools/rider64.exe"));
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
    fn bounds_history_and_skips_missing_directories() {
        let mut settings = Settings::default();
        settings.remember(Path::new(env!("CARGO_MANIFEST_DIR")));
        settings.remember(Path::new("missing-lorelens-repository"));
        assert_eq!(
            settings.restore(),
            Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")))
        );
        for i in 0..20 {
            settings.remember(Path::new(&format!("repo-{i}")));
        }
        assert_eq!(settings.recent.len(), 10);
    }
}
