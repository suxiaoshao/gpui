//! SVG bytes selected by Rust references, without a runtime asset catalog.
//!
//! ```
//! use gpui_lucide::IconName;
//! use gpui_component::{Icon, button::Button};
//! let button = Button::new("search").icon(IconName::Search);
//! let icon = Icon::new(IconName::Search);
//! ```
//!
//! Each associated constant references only its own SVG. There is deliberately
//! no enum-to-bytes match, `ALL` array, or runtime name lookup retaining the
//! catalog. Component default icons still use the application's usual Assets.
use gpui::{App, Entity, IntoElement, RenderOnce, Window};
use gpui_component::Icon;

/// An immutable SVG usable wherever a GPUI Kit component accepts `Into<Icon>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, IntoElement)]
pub struct SvgIcon {
    bytes: &'static [u8],
}

/// The complete upstream Lucide catalog, as independently referenced constants.
pub type IconName = SvgIcon;

impl SvgIcon {
    /// Embed a custom SVG, for example `SvgIcon::new(include_bytes!("logo.svg"))`.
    pub const fn new(bytes: &'static [u8]) -> Self {
        Self { bytes }
    }

    pub const fn bytes(self) -> &'static [u8] {
        self.bytes
    }

    pub fn view(self, cx: &mut App) -> Entity<Icon> {
        Icon::from(self).view(cx)
    }
}

impl From<SvgIcon> for Icon {
    fn from(value: SvgIcon) -> Self {
        Self::default().data(value.bytes)
    }
}

impl RenderOnce for SvgIcon {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        Icon::from(self)
    }
}

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_component::button::Button;

    #[test]
    fn icons_compose_without_registering_an_asset_source() {
        // Dynamic application choice retains just these two byte references.
        let icons = [IconName::Search, IconName::Brain];
        for icon in icons {
            assert!(icon.bytes().starts_with(b"<svg"));
            let _: Icon = icon.into();
            let _ = Button::new("icon-test").icon(icon);
        }
        let custom = SvgIcon::new(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
        let _: Icon = custom.into();
        assert_ne!(icons[0].bytes(), icons[1].bytes());
    }
}
