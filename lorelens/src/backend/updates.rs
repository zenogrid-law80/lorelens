use crate::i18n::{t, tf};
use reqwest::{Url, blocking::Client, redirect::Policy};
use semver::Version;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
  collections::HashMap,
  fs::{self, File},
  io::{Read, Write},
  path::{Path, PathBuf},
  sync::{Mutex, OnceLock, mpsc},
  time::Duration,
};

pub const RELEASES_URL: &str = "https://github.com/zenogrid-law80/lorelens/releases";
const RELEASES_API: &str = "https://api.github.com/repos/zenogrid-law80/lorelens/releases?per_page=20";
const LATEST_RELEASE_API: &str = "https://api.github.com/repos/zenogrid-law80/lorelens/releases/latest";
const MAX_API_BYTES: u64 = 2 * 1024 * 1024;
const MAX_CHECKSUM_BYTES: u64 = 64 * 1024;
const MAX_INSTALLER_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Release {
  pub version: String,
  pub url: String,
  pub published_at: String,
  pub notes: String,
  pub installer: Option<Asset>,
}

#[derive(Clone, Debug)]
pub struct Asset {
  pub name: String,
  pub url: String,
  pub size: u64,
  // GitHub's digest is optional on older releases. The published checksum
  // manifest is always required and is resolved again before downloading.
  pub sha256: String,
}

struct StagedInstaller {
  sha256: String,
  size: u64,
  launched: bool,
  _directory: tempfile::TempDir,
}

fn staged_installers() -> &'static Mutex<HashMap<PathBuf, StagedInstaller>> {
  static INSTALLERS: OnceLock<Mutex<HashMap<PathBuf, StagedInstaller>>> = OnceLock::new();
  INSTALLERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn installer_supported() -> bool {
  cfg!(all(target_os = "windows", target_arch = "x86_64"))
}

fn parse_version(version: &str) -> Option<Version> {
  Version::parse(version.strip_prefix('v').unwrap_or(version)).ok()
}

pub fn is_newer(latest: &str, current: &str) -> bool {
  match (parse_version(latest), parse_version(current)) {
    (Some(latest), Some(current)) => latest.pre.is_empty() && latest.cmp_precedence(&current).is_gt(),
    _ => false,
  }
}

fn https_url(url: &Url) -> bool {
  url.scheme() == "https" && url.username().is_empty() && url.password().is_none() && url.port().is_none()
}

fn official_url(url: &str, expected_path: &str) -> bool {
  Url::parse(url).is_ok_and(|url| https_url(&url) && url.host_str() == Some("github.com") && url.path() == expected_path && url.query().is_none() && url.fragment().is_none())
}

fn allowed_redirect(url: &Url) -> bool {
  https_url(url)
    && matches!(
      url.host_str(),
      Some("api.github.com" | "github.com" | "release-assets.githubusercontent.com" | "objects.githubusercontent.com" | "github-releases.githubusercontent.com")
    )
}

fn client(timeout: Duration) -> Result<Client, String> {
  Client::builder()
    .user_agent(concat!("LoreLens/", env!("CARGO_PKG_VERSION")))
    .connect_timeout(Duration::from_secs(10))
    .timeout(timeout)
    .redirect(Policy::custom(|attempt| {
      if attempt.previous().len() >= 5 || !allowed_redirect(attempt.url()) {
        attempt.error(t("Update download redirected outside official GitHub hosts"))
      } else {
        attempt.follow()
      }
    }))
    .build()
    .map_err(|error| error.without_url().to_string())
}

fn read_bounded(mut reader: impl Read, limit: u64) -> Result<Vec<u8>, String> {
  let mut bytes = Vec::new();
  reader.by_ref().take(limit + 1).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
  if bytes.len() as u64 > limit {
    return Err(t("Update response exceeded the allowed size"));
  }
  Ok(bytes)
}

fn request(client: &Client, url: &str, limit: u64, accept: &str) -> Result<reqwest::blocking::Response, String> {
  let response = client
    .get(url)
    .header("Accept", accept)
    .header("X-GitHub-Api-Version", "2022-11-28")
    .send()
    .map_err(|error| error.without_url().to_string())?;
  if !response.status().is_success() {
    return Err(tf("Update server returned HTTP {status}", &[("status", response.status().as_u16().to_string())]));
  }
  if response.content_length().is_some_and(|length| length > limit) {
    return Err(t("Update response exceeded the allowed size"));
  }
  Ok(response)
}

pub fn releases() -> Result<Vec<Release>, String> {
  let client = client(Duration::from_secs(30))?;
  let latest_bytes = read_bounded(request(&client, LATEST_RELEASE_API, MAX_API_BYTES, "application/vnd.github+json")?, MAX_API_BYTES)?;
  let latest: Value = serde_json::from_slice(&latest_bytes).map_err(|error| error.to_string())?;
  let latest = parse_releases(&Value::Array(vec![latest]), installer_supported())?
    .pop()
    .ok_or_else(|| t("Invalid latest stable release response"))?;
  // The stable endpoint must succeed before reporting update status. A history
  // filled with prereleases, or unavailable history, cannot hide that release.
  let history = (|| {
    let bytes = read_bounded(request(&client, RELEASES_API, MAX_API_BYTES, "application/vnd.github+json")?, MAX_API_BYTES)?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    parse_releases(&value, installer_supported())
  })()
  .unwrap_or_default();
  Ok(merge_latest(latest, history))
}

fn merge_latest(latest: Release, mut history: Vec<Release>) -> Vec<Release> {
  history.retain(|release| release.version != latest.version);
  let index = history.iter().position(|release| is_newer(&latest.version, &release.version)).unwrap_or(history.len());
  history.insert(index, latest);
  history
}

fn valid_hash(hash: &str) -> bool {
  hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn github_digest(asset: &Value) -> Option<String> {
  match asset.get("digest").filter(|digest| !digest.is_null()) {
    None => Some(String::new()),
    Some(digest) => digest.as_str()?.strip_prefix("sha256:").filter(|hash| valid_hash(hash)).map(str::to_ascii_lowercase),
  }
}

fn release_asset(assets: &[Value], name: &str, tag: &str, max_size: u64) -> Option<Asset> {
  let mut matching = assets.iter().filter(|asset| asset["name"].as_str() == Some(name));
  let asset = matching.next()?;
  if matching.next().is_some() || asset["state"].as_str() != Some("uploaded") {
    return None;
  }
  let url = asset["browser_download_url"].as_str()?;
  let size = asset["size"].as_u64()?;
  if size == 0 || size > max_size || !official_url(url, &format!("/zenogrid-law80/lorelens/releases/download/{tag}/{name}")) {
    return None;
  }
  Some(Asset {
    name: name.into(),
    url: url.into(),
    size,
    sha256: github_digest(asset)?,
  })
}

fn parse_releases(value: &Value, installers: bool) -> Result<Vec<Release>, String> {
  let entries = value.as_array().ok_or_else(|| t("Invalid release history response"))?;
  let mut releases = Vec::new();
  for entry in entries.iter().take(20) {
    if entry["draft"].as_bool() != Some(false) || entry["prerelease"].as_bool() != Some(false) {
      continue;
    }
    let Some(tag) = entry["tag_name"].as_str() else { continue };
    let Some(version) = parse_version(tag).filter(|version| version.pre.is_empty() && version.build.is_empty()) else {
      continue;
    };
    let version_text = version.to_string();
    if tag != version_text && tag != format!("v{version_text}") {
      continue;
    }
    let Some(url) = entry["html_url"].as_str().filter(|url| official_url(url, &format!("/zenogrid-law80/lorelens/releases/tag/{tag}"))) else {
      continue;
    };
    let Some(published_at) = entry["published_at"].as_str().filter(|date| chrono::DateTime::parse_from_rfc3339(date).is_ok()) else {
      continue;
    };
    let assets = entry["assets"].as_array();
    let installer = assets.filter(|_| installers).and_then(|assets| {
      release_asset(assets, &format!("SHA256SUMS-{tag}.txt"), tag, MAX_CHECKSUM_BYTES)?;
      release_asset(assets, &format!("LoreLens-{version_text}.msi"), tag, MAX_INSTALLER_BYTES)
    });
    releases.push((
      version,
      Release {
        version: version_text,
        url: url.into(),
        published_at: published_at.into(),
        notes: entry["body"].as_str().unwrap_or_default().chars().take(64 * 1024).collect(),
        installer,
      },
    ));
  }
  releases.sort_by(|(left, _), (right, _)| right.cmp_precedence(left));
  releases.dedup_by(|(left, _), (right, _)| left == right);
  Ok(releases.into_iter().map(|(_, release)| release).collect())
}

fn checksum_for(manifest: &str, name: &str) -> Result<String, String> {
  let mut result = None;
  for line in manifest.trim_start_matches('\u{feff}').lines() {
    let Some((hash, filename)) = line.trim().split_once(char::is_whitespace) else { continue };
    let filename = filename.trim_start().strip_prefix('*').unwrap_or(filename.trim_start());
    if filename == name {
      if !valid_hash(hash) || result.is_some() {
        return Err(t("Invalid or duplicate installer checksum"));
      }
      result = Some(hash.to_ascii_lowercase());
    }
  }
  result.ok_or_else(|| t("The release checksum manifest does not contain this installer"))
}

fn installer_tag(release: &Release, asset: &Asset) -> Result<String, String> {
  let version = parse_version(&release.version)
    .filter(|version| version.pre.is_empty() && version.build.is_empty())
    .ok_or_else(|| t("Invalid update version"))?;
  if version.to_string() != release.version || asset.name != format!("LoreLens-{version}.msi") || asset.size == 0 || asset.size > MAX_INSTALLER_BYTES {
    return Err(t("Invalid update installer"));
  }
  let tag = [format!("v{version}"), version.to_string()]
    .into_iter()
    .find(|tag| official_url(&release.url, &format!("/zenogrid-law80/lorelens/releases/tag/{tag}")))
    .ok_or_else(|| t("Invalid official release URL"))?;
  if !official_url(&asset.url, &format!("/zenogrid-law80/lorelens/releases/download/{tag}/{}", asset.name)) || (!asset.sha256.is_empty() && !valid_hash(&asset.sha256)) {
    return Err(t("Invalid official installer URL or checksum"));
  }
  Ok(tag)
}

fn write_verified(reader: impl Read, file: &mut File, size: u64, expected: &str) -> Result<(), String> {
  let mut reader = reader.take(size + 1);
  let mut hash = Sha256::new();
  let mut written = 0u64;
  let mut buffer = [0; 64 * 1024];
  loop {
    let count = reader.read(&mut buffer).map_err(|error| error.to_string())?;
    if count == 0 {
      break;
    }
    written += count as u64;
    if written > size {
      return Err(t("Downloaded installer exceeded its published size"));
    }
    hash.update(&buffer[..count]);
    file.write_all(&buffer[..count]).map_err(|error| error.to_string())?;
  }
  if written != size || !format!("{:x}", hash.finalize()).eq_ignore_ascii_case(expected) {
    return Err(t("Installer checksum or size verification failed"));
  }
  file.sync_all().map_err(|error| error.to_string())
}

fn verify_staged(path: &Path, installer: &StagedInstaller) -> Result<(), String> {
  let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
  if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != installer.size || path.canonicalize().map_err(|error| error.to_string())? != path {
    return Err(t("The staged installer changed after download"));
  }
  let mut file = File::open(path).map_err(|error| error.to_string())?;
  let mut hash = Sha256::new();
  let mut buffer = [0; 64 * 1024];
  loop {
    let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
    if count == 0 {
      break;
    }
    hash.update(&buffer[..count]);
  }
  if format!("{:x}", hash.finalize()) != installer.sha256 {
    return Err(t("The staged installer changed after download"));
  }
  Ok(())
}

pub fn download_installer(release: &Release) -> Result<PathBuf, String> {
  if !installer_supported() {
    return Err(t("In-app installation is currently supported on Windows x64. Open the release website for this platform."));
  }
  if !is_newer(&release.version, env!("CARGO_PKG_VERSION")) {
    return Err(t("The selected release is not newer than this application"));
  }
  let asset = release.installer.as_ref().ok_or_else(|| t("This release has no supported installer"))?;
  let tag = installer_tag(release, asset)?;
  let checksum_url = format!("{RELEASES_URL}/download/{tag}/SHA256SUMS-{tag}.txt");
  let metadata_client = client(Duration::from_secs(30))?;
  let checksum_bytes = read_bounded(request(&metadata_client, &checksum_url, MAX_CHECKSUM_BYTES, "application/octet-stream")?, MAX_CHECKSUM_BYTES)?;
  let manifest = std::str::from_utf8(&checksum_bytes).map_err(|_| t("Invalid release checksum manifest"))?;
  let sha256 = checksum_for(manifest, &asset.name)?;
  if !asset.sha256.is_empty() && !asset.sha256.eq_ignore_ascii_case(&sha256) {
    return Err(t("GitHub's installer digest does not match the published checksum"));
  }
  let directory = tempfile::Builder::new().prefix("lorelens-update-").tempdir().map_err(|error| error.to_string())?;
  let path = directory.path().join(&asset.name);
  let mut file = File::options().write(true).create_new(true).open(&path).map_err(|error| error.to_string())?;
  let download_client = client(Duration::from_secs(300))?;
  let response = request(&download_client, &asset.url, asset.size, "application/octet-stream")?;
  write_verified(response, &mut file, asset.size, &sha256)?;
  drop(file);
  let path = path.canonicalize().map_err(|error| error.to_string())?;
  staged_installers().lock().map_err(|_| t("Update installer state is unavailable"))?.insert(
    path.clone(),
    StagedInstaller {
      sha256,
      size: asset.size,
      launched: false,
      _directory: directory,
    },
  );
  Ok(path)
}

pub fn launch_installer(path: &Path) -> Result<mpsc::Receiver<Result<(), String>>, String> {
  if !installer_supported() {
    return Err(t("In-app installation is currently supported on Windows x64"));
  }
  let mut installers = staged_installers().lock().map_err(|_| t("Update installer state is unavailable"))?;
  if installers.values().any(|installer| installer.launched) {
    return Err(t("The update installer is already running"));
  }
  let installer = installers.get(path).ok_or_else(|| t("Select an installer downloaded and verified by this application"))?;
  if let Err(error) = verify_staged(path, installer) {
    installers.remove(path);
    return Err(error);
  }
  #[cfg(target_os = "windows")]
  {
    use std::os::windows::process::CommandExt;
    let windows = PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()));
    let child = std::process::Command::new(windows.join("System32/msiexec.exe"))
      .args(installer_args(path))
      .creation_flags(0x08000000)
      .spawn();
    let mut child = match child {
      Ok(child) => child,
      Err(error) => {
        installers.remove(path);
        return Err(error.to_string());
      }
    };
    installers.get_mut(path).unwrap().launched = true;
    drop(installers);
    let path = path.to_owned();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
      // Keep the MSI until Windows Installer has finished reading it. If the
      // app exits first, the persisted temp file remains available to msiexec.
      let result = match child.wait() {
        Ok(status) => {
          if let Ok(mut installers) = staged_installers().lock() {
            installers.remove(&path);
          }
          installation_result(status.code())
        }
        Err(error) => Err(tf("Could not wait for the update installer: {error}", &[("error", error.to_string())])),
      };
      let _ = sender.send(result);
    });
    Ok(receiver)
  }
  #[cfg(not(target_os = "windows"))]
  Err(t("In-app installation is currently supported on Windows x64"))
}

#[cfg(any(target_os = "windows", test))]
fn installation_result(code: Option<i32>) -> Result<(), String> {
  match code {
    Some(0 | 3010) => Ok(()),
    Some(1602) => Err(t("Update installation was canceled.")),
    Some(code) => Err(tf("Update installer exited with code {code}.", &[("code", code.to_string())])),
    None => Err(t("Update installer did not report an exit code.")),
  }
}

#[cfg(target_os = "windows")]
fn installer_args(path: &Path) -> [std::ffi::OsString; 3] {
  use std::os::windows::ffi::{OsStrExt, OsStringExt};
  let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
  let extended: Vec<u16> = "\\\\?\\".encode_utf16().collect();
  let unc: Vec<u16> = "\\\\?\\UNC\\".encode_utf16().collect();
  let package = if wide.starts_with(&unc) {
    let mut normal: Vec<u16> = "\\\\".encode_utf16().collect();
    normal.extend_from_slice(&wide[unc.len()..]);
    std::ffi::OsString::from_wide(&normal)
  } else if wide.starts_with(&extended) {
    std::ffi::OsString::from_wide(&wide[extended.len()..])
  } else {
    path.as_os_str().to_owned()
  };
  ["/i".into(), package, "/norestart".into()]
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  fn release(version: &str) -> Value {
    json!({
      "tag_name": format!("v{version}"),
      "html_url": format!("{RELEASES_URL}/tag/v{version}"),
      "published_at": "2026-10-02T12:00:00Z",
      "draft": false,
      "prerelease": false,
      "body": "Release notes",
      "assets": [
        {
          "name": format!("LoreLens-{version}.msi"), "state": "uploaded", "size": 123,
          "browser_download_url": format!("{RELEASES_URL}/download/v{version}/LoreLens-{version}.msi"),
          "digest": format!("sha256:{}", "a".repeat(64)),
        },
        {
          "name": format!("SHA256SUMS-v{version}.txt"), "state": "uploaded", "size": 88,
          "browser_download_url": format!("{RELEASES_URL}/download/v{version}/SHA256SUMS-v{version}.txt"),
        }
      ],
    })
  }

  #[test]
  fn compares_versions_semantically_and_ignores_build_metadata() {
    assert!(is_newer("v0.1.10", "0.1.9"));
    assert!(is_newer("1.0.0", "1.0.0-rc.1"));
    for (latest, current) in [("0.1.9", "0.1.10"), ("1.0.0+new", "1.0.0+old"), ("1.1.0-rc.1", "1.0.0"), ("invalid", "1.0.0"), ("1.0.0", "invalid")] {
      assert!(!is_newer(latest, current));
    }
  }

  #[test]
  fn selects_stable_versions_and_exact_official_platform_assets() {
    let mut prerelease = release("2.0.0-rc.1");
    prerelease["prerelease"] = json!(true);
    let mut draft = release("2.0.0");
    draft["draft"] = json!(true);
    let entries = json!([release("0.1.9"), draft, release("0.1.10"), prerelease]);
    let parsed = parse_releases(&entries, true).unwrap();
    assert_eq!(parsed.iter().map(|release| release.version.as_str()).collect::<Vec<_>>(), ["0.1.10", "0.1.9"]);
    assert_eq!(parsed[0].installer.as_ref().unwrap().sha256, "a".repeat(64));
    assert!(parse_releases(&entries, false).unwrap().iter().all(|release| release.installer.is_none()));

    for bad_url in [
      "http://github.com/file",
      "https://github.com.evil.test/file",
      "https://github.com/other/repo/file",
      "https://user@github.com/file",
    ] {
      let mut entry = release("1.0.0");
      entry["assets"][0]["browser_download_url"] = json!(bad_url);
      assert!(parse_releases(&json!([entry]), true).unwrap()[0].installer.is_none());
    }
    let mut missing_manifest = release("1.0.0");
    missing_manifest["assets"].as_array_mut().unwrap().pop();
    assert!(parse_releases(&json!([missing_manifest]), true).unwrap()[0].installer.is_none());
    let mut duplicate = release("1.0.0");
    let asset = duplicate["assets"][0].clone();
    duplicate["assets"].as_array_mut().unwrap().push(asset);
    assert!(parse_releases(&json!([duplicate]), true).unwrap()[0].installer.is_none());
  }

  #[test]
  fn latest_stable_is_included_when_history_contains_only_prereleases() {
    let mut prerelease = release("2.0.0-rc.1");
    prerelease["prerelease"] = json!(true);
    let history = parse_releases(&Value::Array(vec![prerelease; 20]), true).unwrap();
    let latest = parse_releases(&json!([release("1.0.0")]), true).unwrap().pop().unwrap();
    let merged = merge_latest(latest.clone(), history);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].version, "1.0.0");
    let history = parse_releases(&json!([release("1.0.0"), release("0.1.10")]), true).unwrap();
    assert_eq!(merge_latest(latest, history).len(), 2);
  }

  #[test]
  fn requires_one_exact_checksum_entry_and_supports_binary_manifest_format() {
    let hash = "ab".repeat(32);
    assert_eq!(
      checksum_for(&format!("\u{feff}{hash}  other.msi\r\n{hash} *LoreLens-1.0.0.msi\r\n"), "LoreLens-1.0.0.msi").unwrap(),
      hash
    );
    for manifest in [
      format!("{hash}  ../LoreLens-1.0.0.msi"),
      "invalid  LoreLens-1.0.0.msi".into(),
      format!("{hash}  LoreLens-1.0.0.msi\n{hash}  LoreLens-1.0.0.msi"),
    ] {
      assert!(checksum_for(&manifest, "LoreLens-1.0.0.msi").is_err());
    }
  }

  #[test]
  fn verifies_download_size_hash_and_staged_changes() {
    let bytes = b"verified installer";
    let hash = format!("{:x}", Sha256::digest(bytes));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("LoreLens-1.0.0.msi");
    let mut file = File::create(&path).unwrap();
    assert!(write_verified(&bytes[..], &mut file, bytes.len() as u64 - 1, &hash).is_err());
    assert!(write_verified(&bytes[..], &mut file, bytes.len() as u64 + 1, &hash).is_err());
    assert!(write_verified(&bytes[..], &mut file, bytes.len() as u64, &"a".repeat(64)).is_err());
    drop(file);
    let mut file = File::create(&path).unwrap();
    write_verified(&bytes[..], &mut file, bytes.len() as u64, &hash).unwrap();
    drop(file);
    let path = path.canonicalize().unwrap();
    let installer = StagedInstaller {
      sha256: hash,
      size: bytes.len() as u64,
      launched: false,
      _directory: directory,
    };
    verify_staged(&path, &installer).unwrap();
    fs::write(&path, b"tampered installer").unwrap();
    assert!(verify_staged(&path, &installer).is_err());
    assert!(launch_installer(&path).is_err());
  }

  #[test]
  fn bounds_metadata_and_rejects_untrusted_redirects() {
    assert_eq!(read_bounded(&b"abc"[..], 3).unwrap(), b"abc");
    assert!(read_bounded(&b"abcd"[..], 3).is_err());
    assert!(allowed_redirect(&Url::parse("https://release-assets.githubusercontent.com/file?token=temporary").unwrap()));
    for url in [
      "http://github.com/file",
      "https://evil.test/file",
      "https://github.com.evil.test/file",
      "https://user@github.com/file",
      "https://github.com:8443/file",
    ] {
      assert!(!allowed_redirect(&Url::parse(url).unwrap()));
    }
  }

  #[test]
  fn installation_completion_distinguishes_success_cancellation_and_failure() {
    assert!(installation_result(Some(0)).is_ok());
    assert!(installation_result(Some(3010)).is_ok());
    assert!(installation_result(Some(1602)).is_err());
    assert!(installation_result(Some(1603)).is_err());
    assert!(installation_result(None).is_err());
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn installer_launch_arguments_preserve_spaces_unicode_and_normalize_windows_paths() {
    for (canonical, normal) in [
      (r"\\?\C:\Users\테스트 사용자\LoreLens-1.0.0.msi", r"C:\Users\테스트 사용자\LoreLens-1.0.0.msi"),
      (r"\\?\UNC\server\share\LoreLens-1.0.0.msi", r"\\server\share\LoreLens-1.0.0.msi"),
      (r"C:\updates\LoreLens-1.0.0.msi", r"C:\updates\LoreLens-1.0.0.msi"),
    ] {
      assert_eq!(
        installer_args(Path::new(canonical)),
        [std::ffi::OsString::from("/i"), std::ffi::OsString::from(normal), std::ffi::OsString::from("/norestart")]
      );
    }
  }
}
