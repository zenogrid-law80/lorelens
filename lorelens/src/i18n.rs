use std::{
  collections::BTreeMap,
  sync::{
    LazyLock,
    atomic::{AtomicUsize, Ordering},
  },
};

pub const LOCALES: [(&str, &str); 3] = [("en-US", "English"), ("ko-KR", "한국어"), ("zh-CN", "简体中文")];
static LOCALE: AtomicUsize = AtomicUsize::new(0);
static EN: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| serde_json::from_str(include_str!("../../i18n/en-US.json")).expect("Invalid en-US catalog"));
static KO: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| serde_json::from_str(include_str!("../../i18n/ko-KR.json")).expect("Invalid ko-KR catalog"));
static ZH: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| serde_json::from_str(include_str!("../../i18n/zh-CN.json")).expect("Invalid zh-CN catalog"));

pub fn normalize(locale: &str) -> &'static str {
  match locale {
    "ko-KR" => "ko-KR",
    "zh-CN" => "zh-CN",
    _ => "en-US",
  }
}

pub fn set_locale(locale: &str) {
  let index = LOCALES.iter().position(|(code, _)| *code == normalize(locale)).unwrap_or(0);
  LOCALE.store(index, Ordering::Relaxed);
}

pub fn translate(locale: &str, key: &str) -> String {
  let localized = match normalize(locale) {
    "ko-KR" => KO.get(key),
    "zh-CN" => ZH.get(key),
    _ => None,
  };
  localized.or_else(|| EN.get(key)).cloned().unwrap_or_else(|| key.to_owned())
}

pub fn t(key: &str) -> String {
  translate(LOCALES[LOCALE.load(Ordering::Relaxed)].0, key)
}

// Replace only template tokens, never tokens inside user-supplied values.
pub fn interpolate(template: &str, values: &[(&str, String)]) -> String {
  let mut output = String::new();
  let mut rest = template;
  while let Some(start) = rest.find('{') {
    output.push_str(&rest[..start]);
    rest = &rest[start..];
    let Some(end) = rest.find('}') else {
      break;
    };
    let name = &rest[1..end];
    if let Some((_, value)) = values.iter().find(|(key, _)| *key == name) {
      output.push_str(value);
    } else {
      output.push_str(&rest[..=end]);
    }
    rest = &rest[end + 1..];
  }
  output.push_str(rest);
  output
}

pub fn tf(key: &str, values: &[(&str, String)]) -> String {
  interpolate(&t(key), values)
}

#[cfg(test)]
mod tests {
  use super::*;
  fn tokens(text: &str) -> std::collections::BTreeSet<&str> {
    text.split('{').skip(1).filter_map(|part| part.split_once('}').map(|(name, _)| name)).collect()
  }
  #[test]
  fn catalogs_have_matching_keys_and_placeholders() {
    assert!(!EN.is_empty());
    for catalog in [&*KO, &*ZH] {
      assert_eq!(EN.keys().collect::<Vec<_>>(), catalog.keys().collect::<Vec<_>>());
      for (key, en) in EN.iter() {
        assert!(!catalog[key].is_empty(), "{key}");
        assert_eq!(tokens(en), tokens(&catalog[key]), "{key}");
      }
    }
  }
  #[test]
  fn fallback_and_interpolation_preserve_user_data() {
    assert_eq!(translate("unknown", "Repository"), "Repository");
    assert_eq!(translate("ko-KR", "Repository"), "저장소");
    assert_eq!(normalize("zh-CN"), "zh-CN");
    assert_eq!(translate("zh-CN", "Repository"), "仓库");
    assert_eq!(translate("zh-CN", "unknown.key"), "unknown.key");
    assert_eq!(translate("ko-KR", "unknown.key"), "unknown.key");
    assert_eq!(interpolate("{name}: {count}", &[("name", "한글 {count}".into()), ("count", "2".into())]), "한글 {count}: 2");
  }
}
