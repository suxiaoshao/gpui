use gpui_component::Icon;
use gpui_lucide::IconName;

fn main() {
    std::hint::black_box(Icon::from(IconName::Search));
    std::hint::black_box(Icon::from(IconName::Brain));
}
