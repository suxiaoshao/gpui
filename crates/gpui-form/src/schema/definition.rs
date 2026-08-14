use std::marker::PhantomData;

use crate::ValidationTrigger;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ValidationTriggers {
    pub mount: bool,
    pub change: bool,
    pub blur: bool,
    pub external: bool,
    pub submit: bool,
}

impl ValidationTriggers {
    pub const ALL: Self = Self {
        mount: true,
        change: true,
        blur: true,
        external: true,
        submit: true,
    };

    pub const fn includes(self, trigger: ValidationTrigger) -> bool {
        match trigger {
            ValidationTrigger::Mount => self.mount,
            ValidationTrigger::Change => self.change,
            ValidationTrigger::Blur => self.blur,
            ValidationTrigger::External => self.external,
            ValidationTrigger::Submit => self.submit,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldSchema {
    name: &'static str,
    required: bool,
    triggers: ValidationTriggers,
}

impl FieldSchema {
    #[doc(hidden)]
    pub const fn new(name: &'static str, required: bool, triggers: ValidationTriggers) -> Self {
        Self {
            name,
            required,
            triggers,
        }
    }

    pub const fn name(self) -> &'static str {
        self.name
    }

    pub const fn is_required(self) -> bool {
        self.required
    }

    pub const fn triggers(self) -> ValidationTriggers {
        self.triggers
    }
}

pub struct RootDef<Root> {
    marker: PhantomData<fn() -> Root>,
}

impl<Root> Copy for RootDef<Root> {}

impl<Root> Clone for RootDef<Root> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Root> RootDef<Root> {
    #[doc(hidden)]
    pub const fn __new() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

pub struct FieldDef<Owner, T> {
    schema: FieldSchema,
    read: fn(&Owner) -> &T,
    read_mut: fn(&mut Owner) -> &mut T,
}

impl<Owner, T> Copy for FieldDef<Owner, T> {}

impl<Owner, T> Clone for FieldDef<Owner, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Owner, T> FieldDef<Owner, T> {
    #[doc(hidden)]
    pub const fn __new(
        schema: FieldSchema,
        read: fn(&Owner) -> &T,
        read_mut: fn(&mut Owner) -> &mut T,
    ) -> Self {
        Self {
            schema,
            read,
            read_mut,
        }
    }

    pub const fn schema(self) -> FieldSchema {
        self.schema
    }

    pub(crate) const fn name(self) -> &'static str {
        self.schema.name()
    }

    pub(crate) const fn read(self) -> fn(&Owner) -> &T {
        self.read
    }

    pub(crate) const fn read_mut(self) -> fn(&mut Owner) -> &mut T {
        self.read_mut
    }
}

pub struct ChildDef<Owner, Child> {
    name: &'static str,
    read: fn(&Owner) -> &Child,
    read_mut: fn(&mut Owner) -> &mut Child,
}

impl<Owner, Child> Copy for ChildDef<Owner, Child> {}

impl<Owner, Child> Clone for ChildDef<Owner, Child> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Owner, Child> ChildDef<Owner, Child> {
    #[doc(hidden)]
    pub const fn __new(
        name: &'static str,
        read: fn(&Owner) -> &Child,
        read_mut: fn(&mut Owner) -> &mut Child,
    ) -> Self {
        Self {
            name,
            read,
            read_mut,
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        self.name
    }

    pub(crate) const fn read(self) -> fn(&Owner) -> &Child {
        self.read
    }

    pub(crate) const fn read_mut(self) -> fn(&mut Owner) -> &mut Child {
        self.read_mut
    }
}

pub struct ItemsDef<Owner, Item> {
    name: &'static str,
    read: fn(&Owner) -> &Vec<Item>,
    read_mut: fn(&mut Owner) -> &mut Vec<Item>,
}

impl<Owner, Item> Copy for ItemsDef<Owner, Item> {}

impl<Owner, Item> Clone for ItemsDef<Owner, Item> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Owner, Item> ItemsDef<Owner, Item> {
    #[doc(hidden)]
    pub const fn __new(
        name: &'static str,
        read: fn(&Owner) -> &Vec<Item>,
        read_mut: fn(&mut Owner) -> &mut Vec<Item>,
    ) -> Self {
        Self {
            name,
            read,
            read_mut,
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        self.name
    }

    pub(crate) const fn read(self) -> fn(&Owner) -> &Vec<Item> {
        self.read
    }

    pub(crate) const fn read_mut(self) -> fn(&mut Owner) -> &mut Vec<Item> {
        self.read_mut
    }
}

pub struct CaseDef<Enum, Payload> {
    name: &'static str,
    read: fn(&Enum) -> Option<&Payload>,
    read_mut: fn(&mut Enum) -> Option<&mut Payload>,
}

impl<Enum, Payload> Copy for CaseDef<Enum, Payload> {}

impl<Enum, Payload> Clone for CaseDef<Enum, Payload> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Enum, Payload> CaseDef<Enum, Payload> {
    #[doc(hidden)]
    pub const fn __new(
        name: &'static str,
        read: fn(&Enum) -> Option<&Payload>,
        read_mut: fn(&mut Enum) -> Option<&mut Payload>,
    ) -> Self {
        Self {
            name,
            read,
            read_mut,
        }
    }

    pub const fn name(self) -> &'static str {
        self.name
    }

    pub(crate) const fn read(self) -> fn(&Enum) -> Option<&Payload> {
        self.read
    }

    pub(crate) const fn read_mut(self) -> fn(&mut Enum) -> Option<&mut Payload> {
        self.read_mut
    }
}
