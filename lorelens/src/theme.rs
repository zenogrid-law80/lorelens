use gpui::{App, Hsla, Window};
use gpui_component::{Theme, ThemeMode, ThemeRegistry};

#[derive(Clone, Copy)]
#[allow(clippy::upper_case_acronyms)]
pub(crate) enum ColorRole {
  BG,
  PANEL,
  BORDER,
  TEXT,
  MUTED,
  BLUE,
  Divider,
  Hover,
  Selected,
  Success,
  Danger,
  Warning,
}

/// Snapshot the active theme so event closures never borrow the app.
pub(crate) fn palette(cx: &App) -> impl Fn(ColorRole) -> Hsla + Copy + use<> {
  let colors = Theme::global(cx).colors;
  move |role| match role {
    ColorRole::BG => colors.background,
    ColorRole::PANEL => colors.sidebar,
    ColorRole::BORDER | ColorRole::Divider => colors.border,
    ColorRole::TEXT => colors.foreground,
    ColorRole::MUTED => colors.muted_foreground,
    ColorRole::BLUE => colors.primary,
    ColorRole::Hover => colors.list_hover,
    ColorRole::Selected => colors.list_active,
    ColorRole::Success => colors.success,
    ColorRole::Danger => colors.danger,
    ColorRole::Warning => colors.warning,
  }
}

pub(super) fn apply_theme(value: &str, window: Option<&mut Window>, cx: &mut App) {
  match value {
    "System" => {
      reset_default_themes(cx);
      Theme::sync_system_appearance(window, cx);
    }
    "Light" => {
      reset_default_themes(cx);
      Theme::change(ThemeMode::Light, window, cx);
    }
    "Dark" => {
      reset_default_themes(cx);
      Theme::change(ThemeMode::Dark, window, cx);
    }
    name => {
      let Some(config) = ThemeRegistry::global(cx).themes().get(name).cloned() else {
        reset_default_themes(cx);
        Theme::sync_system_appearance(window, cx);
        return;
      };
      let mode = config.mode;
      if mode.is_dark() {
        Theme::global_mut(cx).dark_theme = config;
      } else {
        Theme::global_mut(cx).light_theme = config;
      }
      Theme::change(mode, window, cx);
    }
  }
}

/// Apply a user-selected theme after the current entity update has finished.
pub(super) fn defer_theme(value: impl Into<String>, window: &Window, cx: &mut App) {
  let value = value.into();
  window.defer(cx, move |window, cx| apply_theme(&value, Some(window), cx));
}

fn reset_default_themes(cx: &mut App) {
  let light = ThemeRegistry::global(cx).default_light_theme().clone();
  let dark = ThemeRegistry::global(cx).default_dark_theme().clone();
  let theme = Theme::global_mut(cx);
  theme.light_theme = light;
  theme.dark_theme = dark;
}

#[cfg(test)]
mod tests {
  use std::collections::HashSet;

  #[test]
  fn bundled_themes_are_compatible_and_uniquely_named() {
    assert!(!crate::BUNDLED_THEMES.is_empty());
    let mut names = HashSet::new();
    for content in crate::BUNDLED_THEMES {
      let set: gpui_component::ThemeSet = serde_json::from_str(content).unwrap();
      assert!(!set.themes.is_empty());
      for theme in set.themes {
        assert!(names.insert(theme.name.to_string()), "duplicate bundled theme: {}", theme.name);
      }
    }
  }
}
