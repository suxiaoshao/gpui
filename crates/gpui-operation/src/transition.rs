/// Delivers an owned message to a state machine.
///
/// Named-state implementations consume the current state and return the exact
/// next named state. Complete runtime operations implement this trait for a
/// mutable reference, replace their current variant in place, and return `()`.
///
/// ```compile_fail
/// use gpui_operation::{Cancel, Transition};
///
/// struct Idle;
///
/// // `Idle` does not implement `Transition<Cancel>`, so this will not compile.
/// let idle = Idle;
/// let _ = idle.transition(Cancel);
/// ```
pub trait Transition<Message> {
    /// The concrete type that results from this transition.
    type Output;

    /// Deliver `message` to this receiver.
    fn transition(self, message: Message) -> Self::Output;
}
