use gpui::{AssetSource, SharedString};
use std::borrow::Cow;

gpui_kit_assets::icon_assets!(
  ToolbarIcons,
  [
    Database,
    GitBranch,
    RefreshCw,
    Sparkles,
    ArrowDown,
    ArrowUp,
    Copy,
    Trash,
    Search,
    FileText,
    Folder,
    FolderOpen,
    ChevronDown,
    ChevronRight
  ]
);

pub(crate) struct Assets;

impl AssetSource for Assets {
  fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
    if let Some(bytes) = ToolbarIcons.load(path)? {
      return Ok(Some(bytes));
    }
    gpui_kit_assets::Assets.load(path)
  }

  fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
    let mut paths = gpui_kit_assets::Assets.list(path)?;
    paths.extend(ToolbarIcons.list(path)?);
    paths.sort();
    paths.dedup();
    Ok(paths)
  }
}
