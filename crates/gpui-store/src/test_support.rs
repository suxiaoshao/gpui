use gpui::{Context, Entity, Subscription};

pub(crate) struct NotifyCounter<T: 'static> {
    count: usize,
    _subscription: Subscription,
    _marker: std::marker::PhantomData<fn(T)>,
}

impl<T: 'static> NotifyCounter<T> {
    pub(crate) fn new(entity: Entity<T>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&entity, |counter, _, _| {
            counter.count += 1;
        });

        Self {
            count: 0,
            _subscription: subscription,
            _marker: std::marker::PhantomData,
        }
    }

    pub(crate) fn count(&self) -> usize {
        self.count
    }
}
