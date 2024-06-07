/*
 * @Author: suxiaoshao suxiaoshao@gmail.com
 * @Date: 2024-05-31 00:15:11
 * @LastEditors: suxiaoshao suxiaoshao@gmail.com
 * @LastEditTime: 2024-05-31 03:42:47
 * @FilePath: /gpui-app/src/main.rs
 */
use ::theme::SystemAppearance;
use assets::Assets;
use components::{Button, Input};
use gpui::*;
use theme::argb_to_rgba;
use ui::Clickable;

mod components;
mod theme;

struct HelloWorld {
    count: u32,
    input: View<Input>,
}

impl Render for HelloWorld {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let theme = cx.global::<theme::Theme>();
        div()
            .flex()
            .flex_col()
            .bg(theme.bg_color())
            .size_full()
            .justify_center()
            .items_center()
            .shadow_lg()
            .border_1()
            .text_xl()
            .text_color(theme.text_color())
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(format!("Count {}!", &self.count))
                    .child(
                        Button::new("add", "add_button").on_click(cx.listener(|this, _, cx| {
                            this.count += 1;
                            cx.notify()
                        })),
                    )
                    .child(Button::new("sub", "sub_button").on_click(cx.listener(
                        |this, _, cx| {
                            if this.count == 0 {
                                return;
                            }
                            this.count -= 1;
                            cx.notify()
                        },
                    ))),
            )
            .child(div().flex().flex_row().gap_1().children((0..=10).map(|x| {
                div()
                    .bg(argb_to_rgba(theme.palettes.primary.tone(x * 10)))
                    .child(format!("{}", x * 10))
            })))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .child(
                        ui::Button::new(12, "sushao").on_click(cx.listener(|this, _, cx| {
                            this.input.focus_handle(cx).focus(cx);
                        })),
                    ),
            )
            .child(self.input.clone())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .child(ui::Button::new(13, "sushao")),
            )
    }
}

fn main() {
    App::new().with_assets(Assets).run(|cx: &mut AppContext| {
        let theme = theme::get_theme();
        cx.set_global(theme);
        settings::init(cx);
        SystemAppearance::init(cx);
        ::theme::init(::theme::LoadThemes::All(Box::new(Assets)), cx);
        let bounds = Bounds::centered(None, size(px(600.0), px(600.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |cx| {
                let input = cx.new_view(|cx| Input::new("test".to_string(), "input", cx));
                cx.new_view(|_cx| HelloWorld { count: 0, input })
            },
        );
    });
}
