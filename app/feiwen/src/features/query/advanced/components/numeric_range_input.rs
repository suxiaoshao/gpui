use gpui::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, px,
};
use gpui_component::{
    ActiveTheme, Disableable,
    input::{InputState, NumberInput},
    label::Label,
    v_flex,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RangeInputError {
    Missing,
    InvalidNumber,
    Reversed,
}

pub(crate) struct NumericRangeInputState {
    min: Entity<InputState>,
    max: Entity<InputState>,
    min_label: &'static str,
    max_label: &'static str,
    disabled: bool,
}

impl NumericRangeInputState {
    pub(crate) fn new(
        min_label: &'static str,
        max_label: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            min: cx.new(|cx| InputState::new(window, cx).placeholder(min_label)),
            max: cx.new(|cx| InputState::new(window, cx).placeholder(max_label)),
            min_label,
            max_label,
            disabled: false,
        }
    }

    pub(crate) fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        cx.notify();
    }

    pub(crate) fn values(&self, cx: &gpui::App) -> Result<(i32, i32), RangeInputError> {
        let min = self.min.read(cx).value().trim().to_owned();
        let max = self.max.read(cx).value().trim().to_owned();
        if min.is_empty() || max.is_empty() {
            return Err(RangeInputError::Missing);
        }
        let min = min
            .parse::<i32>()
            .map_err(|_| RangeInputError::InvalidNumber)?;
        let max = max
            .parse::<i32>()
            .map_err(|_| RangeInputError::InvalidNumber)?;
        if min > max {
            return Err(RangeInputError::Reversed);
        }
        Ok((min, max))
    }
}

impl Render for NumericRangeInputState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .gap_2()
            .child(range_input(self.min_label, &self.min, self.disabled, cx))
            .child(range_input(self.max_label, &self.max, self.disabled, cx))
    }
}

fn range_input(
    label: &'static str,
    input: &Entity<InputState>,
    disabled: bool,
    cx: &Context<NumericRangeInputState>,
) -> impl IntoElement {
    v_flex()
        .gap_1()
        .min_w(px(120.))
        .child(
            Label::new(label)
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
        .child(NumberInput::new(input).disabled(disabled))
}
