use gpui::{
    App, AppContext, Context, Entity, EntityId, IntoElement, ParentElement, Styled, View, Window,
    div, px,
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
}

impl NumericRangeInputState {
    pub(crate) fn new(
        min_placeholder: &'static str,
        max_placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            min: cx.new(|cx| InputState::new(window, cx).placeholder(min_placeholder)),
            max: cx.new(|cx| InputState::new(window, cx).placeholder(max_placeholder)),
        }
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

#[derive(IntoElement)]
pub(crate) struct NumericRangeInput {
    state: Entity<NumericRangeInputState>,
    min_label: &'static str,
    max_label: &'static str,
    disabled: bool,
}

impl NumericRangeInput {
    pub(crate) fn new(
        state: &Entity<NumericRangeInputState>,
        min_label: &'static str,
        max_label: &'static str,
    ) -> Self {
        Self {
            state: state.clone(),
            min_label,
            max_label,
            disabled: false,
        }
    }
}

impl Disableable for NumericRangeInput {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl View for NumericRangeInput {
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.state.entity_id())
    }

    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (min, max) = {
            let state = self.state.read(cx);
            (state.min.clone(), state.max.clone())
        };

        div()
            .flex()
            .gap_2()
            .child(range_input(self.min_label, min, self.disabled, cx))
            .child(range_input(self.max_label, max, self.disabled, cx))
    }
}

fn range_input(
    label: &'static str,
    input: Entity<InputState>,
    disabled: bool,
    cx: &App,
) -> impl IntoElement {
    v_flex()
        .gap_1()
        .min_w(px(120.))
        .child(
            Label::new(label)
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
        .child(NumberInput::new(&input).disabled(disabled))
}

#[cfg(test)]
mod tests {
    use super::{NumericRangeInput, NumericRangeInputState, RangeInputError};
    use gpui::{
        AppContext, Context, Entity, IntoElement, Render, TestAppContext, Window, WindowHandle, div,
    };
    use gpui_component::Disableable;

    struct TestHost;

    impl Render for TestHost {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    fn open_range_state(
        cx: &mut TestAppContext,
    ) -> (Entity<NumericRangeInputState>, WindowHandle<TestHost>) {
        let mut range = None;
        let window = cx.update(|cx| {
            gpui_component::init(cx);
            cx.open_window(Default::default(), |window, cx| {
                range =
                    Some(cx.new(|cx| NumericRangeInputState::new("最小值", "最大值", window, cx)));
                cx.new(|_| TestHost)
            })
            .expect("open range input test window")
        });
        (range.expect("range input state"), window)
    }

    #[gpui::test]
    fn view_uses_backing_identity_across_rebuilds(cx: &mut TestAppContext) {
        let (state, _window) = open_range_state(cx);
        let first = NumericRangeInput::new(&state, "最小值", "最大值");
        let rebuilt = NumericRangeInput::new(&state, "下限", "上限").disabled(true);

        assert_eq!(gpui::View::entity_id(&first), Some(state.entity_id()));
        assert_eq!(gpui::View::entity_id(&rebuilt), Some(state.entity_id()));
    }

    #[gpui::test]
    fn range_values_keep_existing_validation_semantics(cx: &mut TestAppContext) {
        let (state, window) = open_range_state(cx);
        let (min, max) = state.read_with(cx, |state, _| (state.min.clone(), state.max.clone()));

        assert_eq!(
            state.read_with(cx, |state, cx| state.values(cx)),
            Err(RangeInputError::Missing)
        );

        window
            .update(cx, |_, window, cx| {
                min.update(cx, |input, cx| input.set_value("invalid", window, cx));
                max.update(cx, |input, cx| input.set_value("2", window, cx));
            })
            .expect("range input test window is alive");
        assert_eq!(
            state.read_with(cx, |state, cx| state.values(cx)),
            Err(RangeInputError::InvalidNumber)
        );

        window
            .update(cx, |_, window, cx| {
                min.update(cx, |input, cx| input.set_value("3", window, cx));
            })
            .expect("range input test window is alive");
        assert_eq!(
            state.read_with(cx, |state, cx| state.values(cx)),
            Err(RangeInputError::Reversed)
        );

        window
            .update(cx, |_, window, cx| {
                min.update(cx, |input, cx| input.set_value("-1", window, cx));
            })
            .expect("range input test window is alive");
        assert_eq!(
            state.read_with(cx, |state, cx| state.values(cx)),
            Ok((-1, 2))
        );
    }
}
