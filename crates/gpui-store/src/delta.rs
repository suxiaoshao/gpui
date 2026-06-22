pub trait StoreDelta: Default + 'static {
    fn is_empty(&self) -> bool;
}
