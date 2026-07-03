#![allow(dead_code)]

use crate::ValueFieldStore;

pub fn value_field<T>(_name: &'static str, value: T) -> ValueFieldStore<T>
where
    T: Clone + PartialEq + 'static,
{
    ValueFieldStore::new(value)
}
