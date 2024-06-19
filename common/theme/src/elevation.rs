/*
 * @Author: suxiaoshao suxiaoshao@gmail.com
 * @Date: 2024-06-12 20:03:29
 * @LastEditors: suxiaoshao suxiaoshao@gmail.com
 * @LastEditTime: 2024-06-19 09:39:03
 * @FilePath: /gpui-app/common/theme/src/elevation.rs
 */
use gpui::Rgba;

fn get_overlay_alpha(elevation: f32) -> f32 {
    let alpha_value = if elevation < 1.0 {
        5.11916 * elevation.powi(2)
    } else {
        4.5 * (elevation + 1.0).ln() + 2.0
    };
    (alpha_value * 10.0).round() / 1000.0
}
pub trait ElevationColor {
    fn elevation_color(&self, elevation: f32) -> Self;
}

impl ElevationColor for Rgba {
    fn elevation_color(&self, elevation: f32) -> Self {
        let alpha = get_overlay_alpha(elevation);
        Rgba {
            r: 255.0,
            g: 255.0,
            b: 255.0,
            a: alpha,
        }
    }
}
