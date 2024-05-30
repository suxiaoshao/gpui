use std::{
    ops::{Deref, DerefMut},
    str::FromStr,
};

use gpui::Rgba;
use material_colors::{color::Argb, theme::ThemeBuilder};

pub struct Theme(material_colors::theme::Theme);

impl gpui::Global for Theme {}

impl Deref for Theme {
    type Target = material_colors::theme::Theme;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Theme {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub fn argb_to_rgba(argb: Argb) -> Rgba {
    Rgba {
        r: argb.red as f32 / 255.0,
        g: argb.green as f32 / 255.0,
        b: argb.blue as f32 / 255.0,
        a: argb.alpha as f32 / 255.0,
    }
}

impl Theme {
    pub fn button_color(&self) -> Rgba {
        let color = self.schemes.dark.on_primary;
        argb_to_rgba(color)
    }
    pub fn button_bg_color(&self) -> Rgba {
        let color = self.schemes.dark.primary;
        argb_to_rgba(color)
    }
    pub fn button_hover_color(&self) -> Rgba {
        let color = self.palettes.primary.tone(10);
        argb_to_rgba(color)
    }
    pub fn button_hover_bg_color(&self) -> Rgba {
        let color = self.palettes.primary.tone(70);
        argb_to_rgba(color)
    }
    pub fn button_active_color(&self) -> Rgba {
        let color = self.palettes.primary.tone(0);
        argb_to_rgba(color)
    }
    pub fn button_active_bg_color(&self) -> Rgba {
        let color = self.palettes.primary.tone(60);
        argb_to_rgba(color)
    }
    pub fn bg_color(&self) -> Rgba {
        let color = self.schemes.dark.background;
        argb_to_rgba(color)
    }
    pub fn text_color(&self) -> Rgba {
        let color = self.schemes.dark.on_background;
        argb_to_rgba(color)
    }
}

pub fn get_theme() -> Theme {
    let theme = ThemeBuilder::with_source(Argb::from_str("c47fd7").unwrap()).build();
    Theme(theme)
}
