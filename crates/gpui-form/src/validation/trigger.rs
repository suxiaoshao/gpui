#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValidationTrigger {
    Mount,
    Change,
    Blur,
    Dynamic,
    Submit,
}
