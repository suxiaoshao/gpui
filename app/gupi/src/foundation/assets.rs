use app_assets::{AppAssets, define_lucide_icons};
use gpui_kit::{AssetSource, SharedString};
use std::borrow::Cow;
define_lucide_icons!(pub(crate) enum IconName {
    Settings => "settings", ArrowLeft => "arrow-left", ArrowRight => "arrow-right",
    Check => "check", Sun => "sun", Moon => "moon", Monitor => "monitor",
    RotateCw => "rotate-cw", FolderOpen => "folder-open", Save => "save", Search => "search",
});
const LOGO_BLACK: &str = "brand/logo-black.svg";
const LOGO_WHITE: &str = "brand/logo-white.svg";
pub(crate) fn app_logo(cx: &gpui_kit::App) -> &'static str {
    use gpui_kit::component::ActiveTheme;
    if cx.theme().is_dark() {
        LOGO_WHITE
    } else {
        LOGO_BLACK
    }
}
#[derive(Default)]
pub(crate) struct BrandAssets;
impl AssetSource for BrandAssets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        match path {
            LOGO_BLACK => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/logo-black.svg"
            )))),
            LOGO_WHITE => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/logo-white.svg"
            )))),
            _ => LucideAssets.load(path),
        }
    }
    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        let mut items = LucideAssets.list(path)?;
        items.extend(
            [LOGO_BLACK, LOGO_WHITE]
                .into_iter()
                .filter(|name| name.starts_with(path))
                .map(SharedString::from),
        );
        Ok(items)
    }
}
pub(crate) type Assets = AppAssets<BrandAssets>;
