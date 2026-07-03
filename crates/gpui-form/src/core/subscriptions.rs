use gpui::Subscription;

#[derive(Default)]
pub struct SubscriptionSet {
    subscriptions: Vec<Subscription>,
}

impl SubscriptionSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, subscription: Subscription) {
        self.subscriptions.push(subscription);
    }

    pub fn extend(&mut self, subscriptions: impl IntoIterator<Item = Subscription>) {
        self.subscriptions.extend(subscriptions);
    }

    pub fn clear(&mut self) {
        self.subscriptions.clear();
    }

    pub fn len(&self) -> usize {
        self.subscriptions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.subscriptions.is_empty()
    }
}

impl IntoIterator for SubscriptionSet {
    type Item = Subscription;
    type IntoIter = std::vec::IntoIter<Subscription>;

    fn into_iter(self) -> Self::IntoIter {
        self.subscriptions.into_iter()
    }
}

impl Extend<Subscription> for SubscriptionSet {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = Subscription>,
    {
        self.subscriptions.extend(iter);
    }
}

impl FromIterator<Subscription> for SubscriptionSet {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = Subscription>,
    {
        Self {
            subscriptions: iter.into_iter().collect(),
        }
    }
}

impl std::fmt::Debug for SubscriptionSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriptionSet")
            .field("len", &self.subscriptions.len())
            .finish()
    }
}
