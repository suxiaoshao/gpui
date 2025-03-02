use gpui::*;

use crate::TextInput;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Number {
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
}

impl From<Number> for u32 {
    fn from(value: Number) -> Self {
        match value {
            Number::Zero => 0,
            Number::One => 1,
            Number::Two => 2,
            Number::Three => 3,
            Number::Four => 4,
            Number::Five => 5,
            Number::Six => 6,
            Number::Seven => 7,
            Number::Eight => 8,
            Number::Nine => 9,
        }
    }
}

impl From<Number> for char {
    fn from(value: Number) -> Self {
        match value {
            Number::Zero => '0',
            Number::One => '1',
            Number::Two => '2',
            Number::Three => '3',
            Number::Four => '4',
            Number::Five => '5',
            Number::Six => '6',
            Number::Seven => '7',
            Number::Eight => '8',
            Number::Nine => '9',
        }
    }
}

impl From<u32> for Number {
    fn from(value: u32) -> Self {
        match value {
            0 => Number::Zero,
            1 => Number::One,
            2 => Number::Two,
            3 => Number::Three,
            4 => Number::Four,
            5 => Number::Five,
            6 => Number::Six,
            7 => Number::Seven,
            8 => Number::Eight,
            9 => Number::Nine,
            _ => unreachable!(),
        }
    }
}

impl Number {
    fn parse_from_u32(mut value: u32) -> Vec<Self> {
        let mut result = vec![];
        if value == 0 {
            return vec![Number::Zero];
        }
        while value > 0 {
            let digit = value % 10;
            let number = digit.into();
            result.push(number);
            value /= 10;
        }
        result.reverse();
        if result.is_empty() {
            vec![Self::Zero]
        } else {
            result
        }
    }
    fn parse_from_str(value: &str) -> Vec<Self> {
        let mut result = vec![];
        let chars = value.chars();
        for char in chars {
            match char {
                '0' if !result.is_empty() => {
                    result.push(Self::Zero);
                }
                '1' => result.push(Self::One),
                '2' => result.push(Self::Two),
                '3' => result.push(Self::Three),
                '4' => result.push(Self::Four),
                '5' => result.push(Number::Five),
                '6' => result.push(Number::Six),
                '7' => result.push(Number::Seven),
                '8' => result.push(Number::Eight),
                '9' => result.push(Number::Nine),
                _ => {}
            }
        }
        if result.is_empty() {
            vec![Self::Zero]
        } else {
            result
        }
    }
    fn to_u32(value: &[Self]) -> u32 {
        let mut result = 0;
        let length = value.len() as u32;
        for (index, value) in value.iter().enumerate() {
            let value: u32 = (*value).into();
            result += value * (10u32.pow(length - 1 - index as u32));
        }
        result
    }
    fn to_string(value: &[Self]) -> String {
        let mut result = String::new();
        for value in value {
            result.push((*value).into());
        }
        result
    }
}

type OnChange = Box<dyn Fn(&u32, &mut Window, &mut App) + 'static>;

pub struct IntInput {
    data: Vec<Number>,
    input: Entity<TextInput>,
    on_change: Option<OnChange>,
}

impl IntInput {
    pub fn new(
        int_cx: &mut Context<Self>,
        value: u32,
        placeholder: impl Into<SharedString>,
    ) -> Self {
        let on_change = int_cx.listener(|this, data: &SharedString, window, cx| {
            let new_value = Number::parse_from_str(data);
            this.data = new_value;
            if let Some(on_change) = &this.on_change {
                on_change(&Number::to_u32(this.data.as_slice()), window, cx);
            }
        });
        let input = int_cx
            .new(|cx| TextInput::new(cx, value.to_string(), placeholder).on_change(on_change));
        Self {
            data: Number::parse_from_u32(value),
            input,
            on_change: None,
        }
    }
    pub fn on_change(mut self, on_change: impl Fn(&u32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(on_change));
        self
    }
}

impl Render for IntInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.input.update(cx, |text_input, cx| {
            if !text_input.focus_handle(cx).is_focused(window) {
                text_input.set_value(Number::to_string(self.data.as_slice()));
            }
        });
        self.input.clone()
    }
}

#[cfg(test)]
mod tests {
    use core::prelude::rust_2024;

    use super::*;

    #[rust_2024::test]
    fn test_parse_from_u32() {
        assert_eq!(Number::parse_from_u32(0), vec![Number::Zero]);
        assert_eq!(
            Number::parse_from_u32(123),
            vec![Number::One, Number::Two, Number::Three]
        );
        assert_eq!(
            Number::parse_from_u32(456789),
            vec![
                Number::Four,
                Number::Five,
                Number::Six,
                Number::Seven,
                Number::Eight,
                Number::Nine
            ]
        );
    }

    #[rust_2024::test]
    fn test_parse_from_str() {
        assert_eq!(Number::parse_from_str("0"), vec![Number::Zero]);
        assert_eq!(
            Number::parse_from_str("123"),
            vec![Number::One, Number::Two, Number::Three]
        );
        assert_eq!(
            Number::parse_from_str("456789"),
            vec![
                Number::Four,
                Number::Five,
                Number::Six,
                Number::Seven,
                Number::Eight,
                Number::Nine
            ]
        );
    }

    #[rust_2024::test]
    fn test_to_u32() {
        assert_eq!(Number::to_u32(&[Number::Zero]), 0);
        assert_eq!(
            Number::to_u32(&[Number::One, Number::Two, Number::Three]),
            123
        );
        assert_eq!(
            Number::to_u32(&[
                Number::Four,
                Number::Five,
                Number::Six,
                Number::Seven,
                Number::Eight,
                Number::Nine
            ]),
            456789
        );
    }

    #[rust_2024::test]
    fn test_to_string() {
        assert_eq!(Number::to_string(&[Number::Zero]), "0");
        assert_eq!(
            Number::to_string(&[Number::One, Number::Two, Number::Three]),
            "123"
        );
        assert_eq!(
            Number::to_string(&[
                Number::Four,
                Number::Five,
                Number::Six,
                Number::Seven,
                Number::Eight,
                Number::Nine
            ]),
            "456789"
        );
    }
}
