use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt,
};

use gpui::{App, Task};

use crate::{
    array::FormItemId,
    control::{ControlId, ControlLifetime},
    error::{ValidationIssue, ValidationMessage, ValidationReport, ValidationSource},
    path::FieldPath,
    schema::FormModelSchema,
    trigger::ValidationTrigger,
};

#[derive(Clone, Debug, Default)]
pub struct NoValidationContext;

#[derive(Clone, Debug, PartialEq)]
pub struct AsyncValidationIssue {
    pub code: Cow<'static, str>,
    pub message: ValidationMessage,
}

impl AsyncValidationIssue {
    pub fn new(code: impl Into<Cow<'static, str>>, message: ValidationMessage) -> Self {
        Self {
            code: code.into(),
            message,
        }
    }
}

pub trait ValidationContextValue: Clone + 'static {}

impl<T> ValidationContextValue for T where T: Clone + 'static {}

#[derive(Clone, Copy, Debug)]
pub struct ValidationContext<'a, C = NoValidationContext>
where
    C: ValidationContextValue,
{
    pub external: &'a C,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationScope {
    Form,
    Field(FieldPath),
    Group(FieldPath),
    ArrayItem { path: FieldPath, id: FormItemId },
}

impl ValidationScope {
    pub fn includes(&self, path: Option<&FieldPath>) -> bool {
        match (self, path) {
            (Self::Form, _) => true,
            (Self::Field(expected), Some(path)) => {
                expected.starts_with(path) || path.starts_with(expected)
            }
            (Self::Group(group), Some(path)) => group.starts_with(path) || path.starts_with(group),
            (Self::ArrayItem { path: array, id }, Some(path)) => {
                let item = array.join_item(*id);
                item.starts_with(path) || path.starts_with(&item)
            }
            _ => false,
        }
    }
}

pub trait ValidationAdapter<Model>: Default + 'static {
    type Context: ValidationContextValue;

    fn validate(
        &self,
        model: &Model,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        context: ValidationContext<'_, Self::Context>,
        cx: &App,
    ) -> ValidationAdapterReport;
}

#[derive(Clone, Debug, Default)]
pub struct NoopValidationAdapter;

impl<Model: 'static> ValidationAdapter<Model> for NoopValidationAdapter {
    type Context = NoValidationContext;

    fn validate(
        &self,
        _model: &Model,
        _trigger: ValidationTrigger,
        _scope: ValidationScope,
        _context: ValidationContext<'_, Self::Context>,
        _cx: &App,
    ) -> ValidationAdapterReport {
        ValidationAdapterReport::default()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValidationAdapterReport {
    issues: Vec<ValidationIssue>,
}

impl ValidationAdapterReport {
    pub fn new(issues: Vec<ValidationIssue>) -> Self {
        Self { issues }
    }

    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn issues(&self) -> &[ValidationIssue] {
        &self.issues
    }

    pub fn push(&mut self, issue: ValidationIssue) {
        self.issues.push(issue);
    }

    pub fn into_issues(self) -> Vec<ValidationIssue> {
        self.issues
    }
}

#[doc(hidden)]
pub fn normalize_adapter_report<Model>(
    model: &Model,
    trigger: ValidationTrigger,
    scope: &ValidationScope,
    report: ValidationAdapterReport,
) -> Vec<ValidationIssue>
where
    Model: FormModelSchema,
{
    report
        .into_issues()
        .into_iter()
        .filter_map(|issue| {
            if issue.source == ValidationSource::Internal {
                return Some(issue);
            }

            let Some(path) = issue.path.as_ref() else {
                return scope.includes(None).then_some(issue);
            };
            let schema = match model.schema_at_path(path.segments()) {
                Ok(schema) => schema,
                Err(reason) => {
                    return Some(
                        ValidationIssue::form(
                            trigger,
                            ValidationSource::Internal,
                            "form_schema_path_resolution",
                            ValidationMessage::key("gpui-form-error-internal"),
                        )
                        .with_param("path", path.to_string())
                        .with_param("reason", reason.to_string()),
                    );
                }
            };
            (scope.includes(Some(path)) && schema.triggers().includes(trigger)).then_some(issue)
        })
        .collect()
}

#[derive(Clone, Debug)]
struct ControlIssue {
    lifetime: ControlLifetime,
    issue: ValidationIssue,
}

struct AsyncValidationEntry {
    generation: u64,
    task: Option<Task<()>>,
    issue: Option<ValidationIssue>,
}

#[derive(Default)]
pub struct FormValidationRuntime {
    generated_issues: Vec<ValidationIssue>,
    adapter_issues: Vec<ValidationIssue>,
    control_issues: BTreeMap<ControlId, ControlIssue>,
    async_generation: u64,
    async_entries: BTreeMap<(FieldPath, Cow<'static, str>), AsyncValidationEntry>,
}

impl FormValidationRuntime {
    pub fn report(&self) -> ValidationReport {
        let mut issues = self.generated_issues.clone();
        issues.extend(self.adapter_issues.iter().cloned());
        issues.extend(
            self.async_entries
                .values()
                .filter_map(|entry| entry.issue.clone()),
        );
        issues.extend(
            self.control_issues
                .values()
                .filter(|entry| entry.lifetime.is_alive())
                .map(|entry| entry.issue.clone()),
        );
        ValidationReport::new(issues)
    }

    #[doc(hidden)]
    pub fn replace_generated(&mut self, scope: &ValidationScope, next: Vec<ValidationIssue>) {
        self.generated_issues
            .retain(|issue| !scope.includes(issue.path.as_ref()));
        self.generated_issues.extend(next);
    }

    #[doc(hidden)]
    pub fn replace_adapter(&mut self, next: Vec<ValidationIssue>) {
        self.adapter_issues = next;
    }

    pub fn clear(&mut self) {
        self.generated_issues.clear();
        self.adapter_issues.clear();
        self.control_issues.clear();
        self.async_entries.clear();
    }

    pub fn clear_for_model_replacement(&mut self) {
        self.generated_issues.clear();
        self.adapter_issues.clear();
        self.async_entries.clear();
    }

    pub(crate) fn set_control_issue(
        &mut self,
        id: ControlId,
        lifetime: ControlLifetime,
        issue: ValidationIssue,
    ) {
        self.control_issues
            .insert(id, ControlIssue { lifetime, issue });
    }

    pub(crate) fn clear_control_issue(&mut self, id: ControlId) -> bool {
        self.control_issues.remove(&id).is_some()
    }

    pub fn is_validating(&self) -> bool {
        self.async_entries
            .values()
            .any(|entry| entry.task.is_some())
    }

    pub fn is_validating_at(&self, path: &FieldPath) -> bool {
        self.async_entries.iter().any(|((pending_path, _), entry)| {
            (pending_path.starts_with(path) || path.starts_with(pending_path))
                && entry.task.is_some()
        })
    }

    pub(crate) fn next_async_generation(&mut self) -> u64 {
        self.async_generation = self
            .async_generation
            .checked_add(1)
            .expect("async validation generation overflow");
        self.async_generation
    }

    pub(crate) fn set_async_task(
        &mut self,
        path: FieldPath,
        source: Cow<'static, str>,
        generation: u64,
        task: Task<()>,
    ) {
        self.async_entries.insert(
            (path, source),
            AsyncValidationEntry {
                generation,
                task: Some(task),
                issue: None,
            },
        );
    }

    pub(crate) fn cancel_async(&mut self, path: &FieldPath, source: &str) -> bool {
        self.async_entries
            .remove(&(path.clone(), Cow::Owned(source.to_owned())))
            .is_some()
    }

    #[doc(hidden)]
    pub fn invalidate_path(&mut self, path: &FieldPath) {
        self.generated_issues.retain(|issue| {
            issue.path.as_ref().is_none_or(|issue_path| {
                !issue_path.starts_with(path) && !path.starts_with(issue_path)
            })
        });
        self.async_entries.retain(|(entry_path, _), _| {
            !entry_path.starts_with(path) && !path.starts_with(entry_path)
        });
    }

    pub(crate) fn finish_async(
        &mut self,
        path: &FieldPath,
        source: &str,
        generation: u64,
        issue: Option<ValidationIssue>,
    ) -> bool {
        let key = (path.clone(), Cow::Owned(source.to_owned()));
        let Some(entry) = self.async_entries.get_mut(&key) else {
            return false;
        };
        if entry.generation != generation {
            return false;
        }
        entry.task = None;
        entry.issue = issue;
        true
    }
}

#[doc(hidden)]
pub trait StructuralValidate {
    fn structural_issues(
        &self,
        base: &FieldPath,
        trigger: ValidationTrigger,
        scope: &ValidationScope,
        issues: &mut Vec<ValidationIssue>,
    );
}

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
        !self
    }
}

impl<K, V, S> RequiredValue for HashMap<K, V, S> {
    fn is_missing(&self) -> bool {
        self.is_empty()
    }
}

impl<K, V> RequiredValue for BTreeMap<K, V> {
    fn is_missing(&self) -> bool {
        self.is_empty()
    }
}

impl<T, S> RequiredValue for HashSet<T, S> {
    fn is_missing(&self) -> bool {
        self.is_empty()
    }
}

impl<T> RequiredValue for BTreeSet<T> {
    fn is_missing(&self) -> bool {
        self.is_empty()
    }
}

pub fn required_issue(path: FieldPath, trigger: ValidationTrigger) -> ValidationIssue {
    ValidationIssue::field(
        path,
        trigger,
        ValidationSource::Required,
        "required",
        ValidationMessage::key("gpui-form-error-required"),
    )
}

pub trait GardePathMapper {
    fn map_garde_path(&self, path: &str) -> Result<FieldPath, GardePathError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GardePathError {
    UnknownField {
        path: String,
    },
    InvalidIndex {
        path: String,
        value: String,
    },
    IndexOutOfBounds {
        path: String,
        index: usize,
        len: usize,
    },
    InvalidItemId {
        path: String,
        index: usize,
    },
    DuplicateItemId {
        path: String,
        index: usize,
    },
}

impl fmt::Display for GardePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownField { path } => write!(f, "unknown field in Garde path `{path}`"),
            Self::InvalidIndex { path, value } => {
                write!(f, "invalid array index `{value}` in Garde path `{path}`")
            }
            Self::IndexOutOfBounds { path, index, len } => write!(
                f,
                "array index {index} is out of bounds for length {len} in Garde path `{path}`"
            ),
            Self::InvalidItemId { path, index } => write!(
                f,
                "array item {index} has no valid stable id for Garde path `{path}`"
            ),
            Self::DuplicateItemId { path, index } => write!(
                f,
                "array item {index} has a duplicate stable id for Garde path `{path}`"
            ),
        }
    }
}

impl std::error::Error for GardePathError {}

#[cfg(feature = "garde-adapter")]
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

#[cfg(feature = "garde-adapter")]
pub trait GardeMessageProvider: 'static {
    fn message(rule: GardeRule) -> ValidationMessage;
}

#[cfg(feature = "garde-adapter")]
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultGardeMessageProvider;

#[cfg(feature = "garde-adapter")]
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

#[cfg(feature = "garde-adapter")]
const GARDE_MESSAGE_ENVELOPE_NAMESPACE: &str = "\0gpui-form:garde-message:";
#[cfg(feature = "garde-adapter")]
const GARDE_MESSAGE_ENVELOPE_V1: &str = "\0gpui-form:garde-message:v1:";

#[cfg(feature = "garde-adapter")]
fn encode_garde_message(message: &ValidationMessage) -> String {
    let mut payload = Vec::new();
    match message {
        ValidationMessage::Key { key, params } => {
            payload.push(0);
            encode_garde_string(&mut payload, key);
            encode_garde_len(&mut payload, params.len());
            for (key, value) in params {
                encode_garde_string(&mut payload, key);
                match value {
                    crate::error::ErrorParamValue::String(value) => {
                        payload.push(0);
                        encode_garde_string(&mut payload, value);
                    }
                    crate::error::ErrorParamValue::Integer(value) => {
                        payload.push(1);
                        payload.extend_from_slice(&value.to_be_bytes());
                    }
                    crate::error::ErrorParamValue::Unsigned(value) => {
                        payload.push(2);
                        payload.extend_from_slice(&value.to_be_bytes());
                    }
                    crate::error::ErrorParamValue::Float(value) => {
                        payload.push(3);
                        payload.extend_from_slice(&value.to_bits().to_be_bytes());
                    }
                    crate::error::ErrorParamValue::Bool(value) => {
                        payload.push(4);
                        payload.push(u8::from(*value));
                    }
                }
            }
        }
        ValidationMessage::Literal(message) => {
            payload.push(1);
            encode_garde_string(&mut payload, message);
        }
    }

    let mut envelope = String::with_capacity(GARDE_MESSAGE_ENVELOPE_V1.len() + payload.len() * 2);
    envelope.push_str(GARDE_MESSAGE_ENVELOPE_V1);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in payload {
        envelope.push(HEX[usize::from(byte >> 4)] as char);
        envelope.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    envelope
}

#[cfg(feature = "garde-adapter")]
fn encode_garde_len(payload: &mut Vec<u8>, len: usize) {
    let len = u64::try_from(len).expect("Garde message envelope length exceeds u64");
    payload.extend_from_slice(&len.to_be_bytes());
}

#[cfg(feature = "garde-adapter")]
fn encode_garde_string(payload: &mut Vec<u8>, value: &str) {
    encode_garde_len(payload, value.len());
    payload.extend_from_slice(value.as_bytes());
}

#[cfg(feature = "garde-adapter")]
enum DecodedGardeMessage {
    NotEnvelope,
    Message(ValidationMessage),
    Malformed(&'static str),
}

#[cfg(feature = "garde-adapter")]
fn decode_garde_message(message: &str) -> DecodedGardeMessage {
    if !message.starts_with(GARDE_MESSAGE_ENVELOPE_NAMESPACE) {
        return DecodedGardeMessage::NotEnvelope;
    }
    let Some(payload) = message.strip_prefix(GARDE_MESSAGE_ENVELOPE_V1) else {
        return DecodedGardeMessage::Malformed("unsupported Garde message envelope version");
    };
    let payload = match decode_garde_hex(payload) {
        Ok(payload) => payload,
        Err(reason) => return DecodedGardeMessage::Malformed(reason),
    };
    match decode_garde_payload(&payload) {
        Ok(message) => DecodedGardeMessage::Message(message),
        Err(reason) => DecodedGardeMessage::Malformed(reason),
    }
}

#[cfg(feature = "garde-adapter")]
fn decode_garde_hex(encoded: &str) -> Result<Vec<u8>, &'static str> {
    if encoded.len() % 2 != 0 {
        return Err("Garde message envelope has an odd hexadecimal payload length");
    }
    let mut decoded = Vec::with_capacity(encoded.len() / 2);
    for pair in encoded.as_bytes().chunks_exact(2) {
        let high = decode_garde_hex_digit(pair[0])?;
        let low = decode_garde_hex_digit(pair[1])?;
        decoded.push((high << 4) | low);
    }
    Ok(decoded)
}

#[cfg(feature = "garde-adapter")]
fn decode_garde_hex_digit(value: u8) -> Result<u8, &'static str> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err("Garde message envelope contains non-hexadecimal data"),
    }
}

#[cfg(feature = "garde-adapter")]
fn decode_garde_payload(payload: &[u8]) -> Result<ValidationMessage, &'static str> {
    let mut reader = GardeMessageReader::new(payload);
    let message = match reader.read_byte()? {
        0 => {
            let key = Cow::Owned(reader.read_string()?);
            let param_count = reader.read_len()?;
            let mut params = BTreeMap::new();
            for _ in 0..param_count {
                let key = Cow::Owned(reader.read_string()?);
                let value = match reader.read_byte()? {
                    0 => crate::error::ErrorParamValue::String(Cow::Owned(reader.read_string()?)),
                    1 => crate::error::ErrorParamValue::Integer(reader.read_i64()?),
                    2 => crate::error::ErrorParamValue::Unsigned(reader.read_u64()?),
                    3 => crate::error::ErrorParamValue::Float(f64::from_bits(reader.read_u64()?)),
                    4 => match reader.read_byte()? {
                        0 => crate::error::ErrorParamValue::Bool(false),
                        1 => crate::error::ErrorParamValue::Bool(true),
                        _ => return Err("Garde message envelope contains an invalid boolean"),
                    },
                    _ => return Err("Garde message envelope contains an unknown parameter type"),
                };
                if params.insert(key, value).is_some() {
                    return Err("Garde message envelope contains duplicate parameter keys");
                }
            }
            ValidationMessage::Key { key, params }
        }
        1 => ValidationMessage::Literal(Cow::Owned(reader.read_string()?)),
        _ => return Err("Garde message envelope contains an unknown message type"),
    };
    if !reader.is_empty() {
        return Err("Garde message envelope contains trailing data");
    }
    Ok(message)
}

#[cfg(feature = "garde-adapter")]
struct GardeMessageReader<'a> {
    payload: &'a [u8],
    cursor: usize,
}

#[cfg(feature = "garde-adapter")]
impl<'a> GardeMessageReader<'a> {
    fn new(payload: &'a [u8]) -> Self {
        Self { payload, cursor: 0 }
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], &'static str> {
        let end = self
            .cursor
            .checked_add(len)
            .ok_or("Garde message envelope length overflow")?;
        let value = self
            .payload
            .get(self.cursor..end)
            .ok_or("Garde message envelope ended unexpectedly")?;
        self.cursor = end;
        Ok(value)
    }

    fn read_byte(&mut self) -> Result<u8, &'static str> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_u64(&mut self) -> Result<u64, &'static str> {
        let bytes: [u8; 8] = self
            .read_exact(8)?
            .try_into()
            .expect("Garde message reader requested exactly eight bytes");
        Ok(u64::from_be_bytes(bytes))
    }

    fn read_i64(&mut self) -> Result<i64, &'static str> {
        let bytes: [u8; 8] = self
            .read_exact(8)?
            .try_into()
            .expect("Garde message reader requested exactly eight bytes");
        Ok(i64::from_be_bytes(bytes))
    }

    fn read_len(&mut self) -> Result<usize, &'static str> {
        usize::try_from(self.read_u64()?)
            .map_err(|_| "Garde message envelope length exceeds this platform")
    }

    fn read_string(&mut self) -> Result<String, &'static str> {
        let len = self.read_len()?;
        let value = std::str::from_utf8(self.read_exact(len)?)
            .map_err(|_| "Garde message envelope contains invalid UTF-8")?;
        Ok(value.to_owned())
    }

    fn is_empty(&self) -> bool {
        self.cursor == self.payload.len()
    }
}

#[cfg(feature = "garde-adapter")]
struct GardeMessageI18n<P> {
    marker: std::marker::PhantomData<fn() -> P>,
}

#[cfg(feature = "garde-adapter")]
impl<P> Default for GardeMessageI18n<P> {
    fn default() -> Self {
        Self {
            marker: std::marker::PhantomData,
        }
    }
}

#[cfg(feature = "garde-adapter")]
impl<P> GardeMessageI18n<P>
where
    P: GardeMessageProvider,
{
    fn message(rule: GardeRule) -> Cow<'static, str> {
        Cow::Owned(encode_garde_message(&P::message(rule)))
    }
}

#[cfg(feature = "garde-adapter")]
impl<P> garde::i18n::I18n for GardeMessageI18n<P>
where
    P: GardeMessageProvider,
{
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

#[cfg(feature = "garde-adapter")]
pub fn garde_error(message: ValidationMessage) -> garde::Error {
    garde::Error::new(encode_garde_message(&message))
}

#[cfg(feature = "garde-adapter")]
#[derive(Clone, Copy, Debug)]
pub struct GardeAdapter<T, P = DefaultGardeMessageProvider> {
    marker: std::marker::PhantomData<fn() -> (T, P)>,
}

#[cfg(feature = "garde-adapter")]
impl<T, P> Default for GardeAdapter<T, P> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "garde-adapter")]
impl<T, P> GardeAdapter<T, P> {
    pub fn new() -> Self {
        Self {
            marker: std::marker::PhantomData,
        }
    }
}

#[cfg(feature = "garde-adapter")]
impl<T, P> ValidationAdapter<T> for GardeAdapter<T, P>
where
    T: garde::Validate + GardePathMapper + 'static,
    T::Context: ValidationContextValue,
    P: GardeMessageProvider,
{
    type Context = T::Context;

    fn validate(
        &self,
        model: &T,
        trigger: ValidationTrigger,
        _scope: ValidationScope,
        context: ValidationContext<'_, Self::Context>,
        _cx: &App,
    ) -> ValidationAdapterReport {
        let result = garde::i18n::with_i18n(GardeMessageI18n::<P>::default(), || {
            garde::Validate::validate_with(model, context.external)
        });
        let Err(report) = result else {
            return ValidationAdapterReport::default();
        };

        let mut issues = Vec::new();
        for (path, error) in report.into_inner() {
            let garde_path = path.to_string();
            let message = match decode_garde_message(error.message()) {
                DecodedGardeMessage::NotEnvelope => {
                    ValidationMessage::literal(error.message().to_owned())
                }
                DecodedGardeMessage::Message(message) => message,
                DecodedGardeMessage::Malformed(reason) => {
                    issues.push(
                        ValidationIssue::form(
                            trigger,
                            ValidationSource::Internal,
                            "garde_message_envelope",
                            ValidationMessage::key("gpui-form-error-internal"),
                        )
                        .with_param("path", garde_path)
                        .with_param("reason", reason),
                    );
                    continue;
                }
            };
            if garde_path.is_empty() {
                issues.push(ValidationIssue::form(
                    trigger,
                    ValidationSource::Garde,
                    "garde",
                    message,
                ));
                continue;
            }

            match model.map_garde_path(&garde_path) {
                Ok(path) => issues.push(ValidationIssue::field(
                    path,
                    trigger,
                    ValidationSource::Garde,
                    "garde",
                    message,
                )),
                Err(reason) => issues.push(
                    ValidationIssue::form(
                        trigger,
                        ValidationSource::Internal,
                        "garde_path_mapping",
                        ValidationMessage::key("gpui-form-error-internal"),
                    )
                    .with_param("path", garde_path)
                    .with_param("reason", reason.to_string()),
                ),
            }
        }
        ValidationAdapterReport::new(issues)
    }
}
