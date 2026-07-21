#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValidationTrigger {
    Mount,
    Change,
    Blur,
    Dynamic,
    Submit,
}
