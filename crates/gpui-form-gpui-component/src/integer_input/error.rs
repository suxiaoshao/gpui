#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegerInputError<N> {
    Incomplete,
    InvalidSyntax,
    Overflow,
    OutOfRange { min: Option<N>, max: Option<N> },
}
