use anyhow::{Result, anyhow};
use gpui::{AssetSource, SharedString};
use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "jpg/*.jpg"]
#[include = "png/*.png"]
struct AssetsInner;

impl AssetSource for AssetsInner {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        Self::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow!("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect())
    }
}

pub struct Assets {
    assets: AssetsInner,
    components_assets: gpui_component_assets::Assets,
}

impl Default for Assets {
    fn default() -> Self {
        Self {
            assets: AssetsInner,
            components_assets: gpui_component_assets::Assets,
        }
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        self.assets
            .load(path)
            .or_else(|_| self.components_assets.load(path))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        self.assets
            .list(path)
            .or_else(|_| self.components_assets.list(path))
    }
}
