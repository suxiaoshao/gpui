use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    fmt,
    marker::PhantomData,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{
    FieldSchema, FormSchema, ValidationMessage, ValidationRequest, ValidationSink,
    ValidationSource, Validator,
    schema::SchemaVisitor,
    topology::{CanonicalAddress, TopologyIndex},
};

pub enum GardeRule {
    LengthLowerThan {
        min: usize,
    },
    LengthGreaterThan {
        max: usize,
    },
    RangeLowerThan {
        min: Cow<'static, str>,
    },
    RangeGreaterThan {
        max: Cow<'static, str>,
    },
    CreditCardInvalid {
        reason: garde::i18n::InvalidCreditCard,
    },
    PatternNoMatch {
        pattern: Cow<'static, str>,
    },
    ContainsMissing {
        pattern: Cow<'static, str>,
    },
    UrlInvalid {
        reason: garde::i18n::InvalidUrl,
    },
    PrefixMissing {
        pattern: Cow<'static, str>,
    },
    SuffixMissing {
        pattern: Cow<'static, str>,
    },
    PhoneNumberInvalid {
        reason: garde::i18n::InvalidPhoneNumber,
    },
    IpInvalid {
        kind: garde::i18n::IpKind,
    },
    MatchesFieldMismatch {
        field: Cow<'static, str>,
    },
    EmailInvalid {
        reason: garde::i18n::InvalidEmail,
    },
    AsciiInvalid,
    AlphanumericInvalid,
    RequiredNotSet,
}

pub trait GardeMessageProvider: 'static {
    fn message(rule: GardeRule) -> ValidationMessage;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultGardeMessageProvider;

impl GardeMessageProvider for DefaultGardeMessageProvider {
    fn message(rule: GardeRule) -> ValidationMessage {
        use garde::i18n::I18n as _;

        let handler = garde::i18n::DefaultI18n;
        let message = match rule {
            GardeRule::LengthLowerThan { min } => handler.length_lower_than(min),
            GardeRule::LengthGreaterThan { max } => handler.length_greater_than(max),
            GardeRule::RangeLowerThan { min } => handler.range_lower_than(&min),
            GardeRule::RangeGreaterThan { max } => handler.range_greater_than(&max),
            GardeRule::CreditCardInvalid { reason } => handler.credit_card_invalid(reason),
            GardeRule::PatternNoMatch { pattern } => handler.pattern_no_match(&pattern),
            GardeRule::ContainsMissing { pattern } => handler.contains_missing(&pattern),
            GardeRule::UrlInvalid { reason } => handler.url_invalid(reason),
            GardeRule::PrefixMissing { pattern } => handler.prefix_missing(&pattern),
            GardeRule::SuffixMissing { pattern } => handler.suffix_missing(&pattern),
            GardeRule::PhoneNumberInvalid { reason } => handler.phone_number_invalid(reason),
            GardeRule::IpInvalid { kind } => handler.ip_invalid(kind),
            GardeRule::MatchesFieldMismatch { field } => handler.matches_field_mismatch(&field),
            GardeRule::EmailInvalid { reason } => handler.email_invalid(reason),
            GardeRule::AsciiInvalid => handler.ascii_invalid(),
            GardeRule::AlphanumericInvalid => handler.alphanumeric_invalid(),
            GardeRule::RequiredNotSet => handler.required_not_set(),
        };
        ValidationMessage::literal(message)
    }
}

const ENVELOPE: &str = "\0gpui-form:garde-message:v2:";
static NEXT_MESSAGE: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static MESSAGES: RefCell<HashMap<u64, ValidationMessage>> = RefCell::new(HashMap::new());
}

fn store_message(message: ValidationMessage) -> Cow<'static, str> {
    let id = NEXT_MESSAGE.fetch_add(1, Ordering::Relaxed);
    MESSAGES.with_borrow_mut(|messages| {
        messages.insert(id, message);
    });
    Cow::Owned(format!("{ENVELOPE}{id}"))
}

fn take_message(message: &str) -> Result<Option<ValidationMessage>, &'static str> {
    let Some(id) = message.strip_prefix(ENVELOPE) else {
        return Ok(None);
    };
    let id = id
        .parse::<u64>()
        .map_err(|_| "invalid Garde message envelope identity")?;
    MESSAGES
        .with_borrow_mut(|messages| messages.remove(&id))
        .map(Some)
        .ok_or("expired Garde message envelope")
}

pub fn garde_error(message: ValidationMessage) -> garde::Error {
    garde::Error::new(store_message(message))
}

struct MessageI18n<P>(PhantomData<fn() -> P>);

impl<P> Default for MessageI18n<P> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<P: GardeMessageProvider> MessageI18n<P> {
    fn message(rule: GardeRule) -> Cow<'static, str> {
        store_message(P::message(rule))
    }
}

impl<P: GardeMessageProvider> garde::i18n::I18n for MessageI18n<P> {
    fn length_lower_than(&self, min: usize) -> Cow<'static, str> {
        Self::message(GardeRule::LengthLowerThan { min })
    }
    fn length_greater_than(&self, max: usize) -> Cow<'static, str> {
        Self::message(GardeRule::LengthGreaterThan { max })
    }
    fn range_lower_than(&self, min: &dyn fmt::Display) -> Cow<'static, str> {
        Self::message(GardeRule::RangeLowerThan {
            min: Cow::Owned(min.to_string()),
        })
    }
    fn range_greater_than(&self, max: &dyn fmt::Display) -> Cow<'static, str> {
        Self::message(GardeRule::RangeGreaterThan {
            max: Cow::Owned(max.to_string()),
        })
    }
    fn credit_card_invalid(&self, reason: garde::i18n::InvalidCreditCard) -> Cow<'static, str> {
        Self::message(GardeRule::CreditCardInvalid { reason })
    }
    fn pattern_no_match(&self, pattern: &dyn fmt::Display) -> Cow<'static, str> {
        Self::message(GardeRule::PatternNoMatch {
            pattern: Cow::Owned(pattern.to_string()),
        })
    }
    fn contains_missing(&self, pattern: &dyn fmt::Display) -> Cow<'static, str> {
        Self::message(GardeRule::ContainsMissing {
            pattern: Cow::Owned(pattern.to_string()),
        })
    }
    fn url_invalid(&self, reason: garde::i18n::InvalidUrl) -> Cow<'static, str> {
        Self::message(GardeRule::UrlInvalid { reason })
    }
    fn prefix_missing(&self, pattern: &dyn fmt::Display) -> Cow<'static, str> {
        Self::message(GardeRule::PrefixMissing {
            pattern: Cow::Owned(pattern.to_string()),
        })
    }
    fn suffix_missing(&self, pattern: &dyn fmt::Display) -> Cow<'static, str> {
        Self::message(GardeRule::SuffixMissing {
            pattern: Cow::Owned(pattern.to_string()),
        })
    }
    fn phone_number_invalid(&self, reason: garde::i18n::InvalidPhoneNumber) -> Cow<'static, str> {
        Self::message(GardeRule::PhoneNumberInvalid { reason })
    }
    fn ip_invalid(&self, kind: garde::i18n::IpKind) -> Cow<'static, str> {
        Self::message(GardeRule::IpInvalid { kind })
    }
    fn matches_field_mismatch(&self, field: &dyn fmt::Display) -> Cow<'static, str> {
        Self::message(GardeRule::MatchesFieldMismatch {
            field: Cow::Owned(field.to_string()),
        })
    }
    fn email_invalid(&self, reason: garde::i18n::InvalidEmail) -> Cow<'static, str> {
        Self::message(GardeRule::EmailInvalid { reason })
    }
    fn ascii_invalid(&self) -> Cow<'static, str> {
        Self::message(GardeRule::AsciiInvalid)
    }
    fn alphanumeric_invalid(&self) -> Cow<'static, str> {
        Self::message(GardeRule::AlphanumericInvalid)
    }
    fn required_not_set(&self) -> Cow<'static, str> {
        Self::message(GardeRule::RequiredNotSet)
    }
}

pub struct GardeValidator<T, P = DefaultGardeMessageProvider>
where
    T: garde::Validate,
{
    context: T::Context,
    marker: PhantomData<fn() -> P>,
}

impl<T, P> GardeValidator<T, P>
where
    T: garde::Validate,
{
    pub fn new(context: T::Context) -> Self {
        Self {
            context,
            marker: PhantomData,
        }
    }

    pub fn context(&self) -> &T::Context {
        &self.context
    }
}

impl<T, P> Validator<T> for GardeValidator<T, P>
where
    T: FormSchema + garde::Validate,
    T::Context: 'static,
    P: GardeMessageProvider,
{
    fn validate(
        &self,
        model: &T,
        request: ValidationRequest<'_, T>,
        out: &mut ValidationSink<'_, T>,
    ) {
        let result = garde::i18n::with_i18n(MessageI18n::<P>::default(), || {
            garde::Validate::validate_with(model, &self.context)
        });
        let Err(report) = result else { return };

        let paths = schema_paths(model, request.topology);
        for (path, error) in report.into_inner() {
            let external = path.to_string();
            let message = match take_message(error.message()) {
                Ok(Some(message)) => message,
                Ok(None) => ValidationMessage::literal(error.message().to_owned()),
                Err(reason) => {
                    out.push_with_source(
                        CanonicalAddress::default(),
                        ValidationSource::Internal,
                        "garde_message_envelope",
                        ValidationMessage::key("gpui-form-error-internal")
                            .with_param("path", external)
                            .with_param("reason", reason),
                    );
                    continue;
                }
            };
            if external.is_empty() {
                out.push_with_source(
                    CanonicalAddress::default(),
                    ValidationSource::Validator(Cow::Borrowed("garde")),
                    "garde",
                    message,
                );
                continue;
            }
            let Some((address, schema)) = paths.get(&external) else {
                out.push_with_source(
                    CanonicalAddress::default(),
                    ValidationSource::Internal,
                    "garde_path_mapping",
                    ValidationMessage::key("gpui-form-error-internal").with_param("path", external),
                );
                continue;
            };
            if request.includes_address(address) && schema.triggers().includes(request.trigger()) {
                out.push_with_source(
                    address.clone(),
                    ValidationSource::Validator(Cow::Borrowed("garde")),
                    "garde",
                    message,
                );
            }
        }
    }
}

fn schema_paths<M: FormSchema>(
    model: &M,
    topology: &TopologyIndex,
) -> HashMap<String, (CanonicalAddress, FieldSchema)> {
    let mut visitor = PathVisitor {
        topology,
        address: CanonicalAddress::default(),
        external: String::new(),
        aliases: Vec::new(),
        paths: HashMap::new(),
    };
    model.__visit(&mut visitor);
    visitor.paths
}

struct PathVisitor<'a> {
    topology: &'a TopologyIndex,
    address: CanonicalAddress,
    external: String,
    aliases: Vec<String>,
    paths: HashMap<String, (CanonicalAddress, FieldSchema)>,
}

impl PathVisitor<'_> {
    fn field_name(prefix: &str, name: &str) -> String {
        if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}.{name}")
        }
    }

    fn nested(&self, address: CanonicalAddress, external: String) -> Self {
        Self {
            topology: self.topology,
            address,
            external,
            aliases: self.aliases.clone(),
            paths: HashMap::new(),
        }
    }

    fn absorb(&mut self, nested: Self) {
        self.paths.extend(nested.paths);
    }
}

impl SchemaVisitor for PathVisitor<'_> {
    fn field(&mut self, schema: FieldSchema, _missing: bool) {
        let address = self.address.field(schema.name());
        let external = Self::field_name(&self.external, schema.name());
        self.paths
            .insert(external.clone(), (address.clone(), schema));
        for alias in &self.aliases {
            let alias = Self::field_name(alias, schema.name());
            self.paths.insert(alias, (address.clone(), schema));
        }
    }

    fn child(&mut self, name: &'static str, visit: &mut dyn FnMut(&mut dyn SchemaVisitor)) {
        let mut nested = self.nested(
            self.address.field(name),
            Self::field_name(&self.external, name),
        );
        visit(&mut nested);
        self.absorb(nested);
    }

    fn optional(
        &mut self,
        name: &'static str,
        present: bool,
        visit: &mut dyn FnMut(&mut dyn SchemaVisitor),
    ) {
        if !present {
            return;
        }
        let mut nested = self.nested(
            self.address.field(name).some(),
            Self::field_name(&self.external, name),
        );
        visit(&mut nested);
        self.absorb(nested);
    }

    fn items(
        &mut self,
        name: &'static str,
        len: usize,
        visit: &mut dyn FnMut(usize, &mut dyn SchemaVisitor),
    ) {
        let collection = self.address.field(name);
        let tokens = self
            .topology
            .ensure_items(&collection, len)
            .expect("form identity exhausted after construction");
        let prefix = Self::field_name(&self.external, name);
        for (index, token) in tokens.into_iter().enumerate() {
            let mut nested = self.nested(collection.item(token), format!("{prefix}[{index}]"));
            visit(index, &mut nested);
            self.absorb(nested);
        }
    }

    fn case(&mut self, name: &'static str, visit: &mut dyn FnMut(&mut dyn SchemaVisitor)) {
        let mut nested = self.nested(self.address.case(name), self.external.clone());
        nested.aliases.push(format!("{}[0]", self.external));
        visit(&mut nested);
        self.absorb(nested);
    }

    fn unit_case(&mut self, _name: &'static str) {}
}
