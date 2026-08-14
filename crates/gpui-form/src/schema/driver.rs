use super::FieldSchema;

pub trait RequiredValue {
    fn is_missing(&self) -> bool;
}

impl RequiredValue for String {
    fn is_missing(&self) -> bool {
        self.trim().is_empty()
    }
}
impl RequiredValue for str {
    fn is_missing(&self) -> bool {
        self.trim().is_empty()
    }
}
impl<T> RequiredValue for Option<T> {
    fn is_missing(&self) -> bool {
        self.is_none()
    }
}
impl<T> RequiredValue for Vec<T> {
    fn is_missing(&self) -> bool {
        self.is_empty()
    }
}
impl RequiredValue for bool {
    fn is_missing(&self) -> bool {
        !*self
    }
}

#[doc(hidden)]
pub trait SchemaVisitor {
    fn field(&mut self, schema: FieldSchema, missing: bool);
    fn child(&mut self, name: &'static str, visit: &mut dyn FnMut(&mut dyn SchemaVisitor));
    fn optional(
        &mut self,
        name: &'static str,
        present: bool,
        visit: &mut dyn FnMut(&mut dyn SchemaVisitor),
    );
    fn items(
        &mut self,
        name: &'static str,
        len: usize,
        visit: &mut dyn FnMut(usize, &mut dyn SchemaVisitor),
    );
    fn case(&mut self, name: &'static str, visit: &mut dyn FnMut(&mut dyn SchemaVisitor));
    fn unit_case(&mut self, name: &'static str);
}

pub trait FormSchema: Clone + PartialEq + 'static {
    #[doc(hidden)]
    fn __visit(&self, visitor: &mut dyn SchemaVisitor);
}
