use gpui::{App, Hsla, Window};
use gpui_component::{Theme, ThemeMode};

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
    "Light" => Theme::change(ThemeMode::Light, window, cx),
    "Dark" => Theme::change(ThemeMode::Dark, window, cx),
    _ => Theme::sync_system_appearance(window, cx),
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn longbridge_theme_is_compatible() {
    let set: gpui_component::ThemeSet = serde_json::from_str(include_str!("../themes/longbridge-pro.json")).unwrap();
    assert_eq!(set.themes.len(), 2);
    assert!(set.themes.iter().any(|theme| theme.mode.is_dark()));
    assert!(set.themes.iter().any(|theme| !theme.mode.is_dark()));
  }
}
