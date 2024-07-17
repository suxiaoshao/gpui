use gpui::*;
use smallvec::smallvec;
use theme::ElevationColor;

pub fn input_border(theme: &theme::Theme) -> Div {
    div()
        .rounded_md()
        .shadow(smallvec![
            BoxShadow {
                color: hsla(0., 0., 0., 0.12),
                offset: point(px(0.), px(2.)),
                blur_radius: px(3.),
                spread_radius: px(0.),
            },
            BoxShadow {
                color: hsla(0., 0., 0., 0.08),
                offset: point(px(0.), px(3.)),
                blur_radius: px(6.),
                spread_radius: px(0.),
            },
            BoxShadow {
                color: hsla(0., 0., 0., 0.04),
                offset: point(px(0.), px(6.)),
                blur_radius: px(12.),
                spread_radius: px(0.),
            },
        ])
        .border(px(0.5))
        .border_color(theme.divider_color())
        .bg(theme.bg_color().elevation_color(3.0))
        .p_1()
}
