// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-light pure transform — agent-native reduction on a JSON value.
//! Agent-native reduction for envelopes that are not search results.
//!
//! # The defect this module closes
//!
//! `--fields`, `--filter`, `--limit`, `--sort`, `--dedupe-by`, `--count-only`
//! and `--truncate-content` were implemented per CONCRETE TYPE, on
//! [`crate::types::SearchOutput`] and its two relatives. They are declared on
//! the ROOT argument set, so `doctor`, `commands`, `schema`, `locale` and
//! `config` accepted every one of them, exited `0`, and emitted a byte-for-byte
//! unchanged envelope. Measured on v1.0.3: `doctor --fields type` produced 2523
//! bytes against a 2524-byte baseline — the one byte was the trailing newline.
//!
//! A flag that parses and does nothing is worse than a flag that is rejected.
//! The caller believes a budget is in force and never learns otherwise, and an
//! agent has no way to discover the truth short of diffing byte counts.
//!
//! # The rule
//!
//! Every operation either DOES something or REFUSES loudly. There is no third
//! outcome. Which of the two applies is a property of the envelope, published
//! as data in [`SURFACES`] and reachable by an agent through `commands`:
//!
//! - Operations that reorder, filter or count ROWS need an array of rows. An
//!   envelope without one refuses them with exit `2`.
//! - Operations that project or shorten work on any JSON object, so they apply
//!   everywhere.
//!
//! # Why `--fields` means something different here
//!
//! On a search envelope `--fields` names RESULT columns, validated against an
//! allowlist ([`crate::output::FieldSet`]). Introspection envelopes have no
//! result rows, so that allowlist is meaningless for them. Here `--fields`
//! takes dotted PATHS into the envelope and is validated against the real
//! document rather than a hand-written list: a path that matches nothing is an
//! error naming the keys that do exist. The two readings never collide,
//! because no surface has both shapes.

use crate::error::CliError;
use serde_json::{Map, Value};

/// The reduction knobs an invocation asked for, grouped once.
///
/// `--max-output-bytes` is deliberately absent: it is enforced for every
/// surface at [`crate::output::emit`]'s single stdout choke point and needs no
/// per-envelope decision.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentOps {
    /// `--fields` / `--select`: dotted paths to keep.
    pub fields: Option<String>,
    /// `--filter`: `key=value`, `key!=value` or `key~substring` over rows.
    pub filter: Option<String>,
    /// `--limit`: keep at most N rows.
    pub limit: Option<u32>,
    /// `--sort`: order rows by a row key, `KEY[:asc|desc]`.
    pub sort: Option<String>,
    /// `--dedupe-by`: drop later rows repeating a row key.
    pub dedupe_by: Option<String>,
    /// `--count-only`: replace the rows with their count.
    pub count_only: bool,
    /// `--truncate-content`: cap every string at N Unicode scalars.
    pub truncate_content: Option<u32>,
}

impl AgentOps {
    /// Collect the knobs an invocation set on the root argument group.
    ///
    /// `--select` is the documented alias of `--fields`; clap already refuses
    /// both at once, so taking the first `Some` is total.
    #[must_use]
    pub fn from_root(args: &crate::cli::CliArgs) -> Self {
        Self::from_group(&args.agent)
    }

    /// Collect the knobs from the shared clap group.
    ///
    /// Reading the group instead of nine loose fields is what lets the same
    /// code serve the root path and `deep-research` without a second copy.
    #[must_use]
    pub fn from_group(args: &crate::cli::AgentOpsArgs) -> Self {
        Self {
            fields: args.fields_or_select(),
            filter: args.result_filter.clone(),
            limit: args.result_limit,
            sort: args.sort.clone(),
            dedupe_by: args.dedupe_by.clone(),
            count_only: args.count_only,
            truncate_content: args.truncate_content,
        }
    }

    /// Whether any reduction was requested at all (fast path).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Reduction knobs for this process, installed once after clap and XDG resolve.
///
/// # Why `OnceLock` and not `RwLock<Option<_>>`
///
/// The knobs are written exactly once, by `run` immediately after argument
/// resolution, and read once per emitted envelope. `RwLock<Option<AgentOps>>`
/// modelled a value that could change at any time, and paid for that fiction
/// on every read: `process_agent_ops` cloned seven fields — up to five heap
/// strings — to hand back an owned copy the caller only borrowed from.
///
/// `OnceLock` states the real invariant. Install-once is enforced rather than
/// merely intended, the read is a plain pointer load with no lock, and the
/// caller gets a `&'static` reference with nothing to clone. This is a
/// one-shot binary: born, execute, die. The knobs cannot change mid-life
/// because there is no mid-life.
static PROCESS_OPS: std::sync::OnceLock<AgentOps> = std::sync::OnceLock::new();

/// The empty knob set, for invocations that requested no reduction.
///
/// A `const` rather than `AgentOps::default()` so it can back a `'static`
/// reference without allocating or lazily initialising anything.
const NO_OPS: AgentOps = AgentOps {
    fields: None,
    filter: None,
    limit: None,
    sort: None,
    dedupe_by: None,
    count_only: false,
    truncate_content: None,
};

/// Install the reduction knobs for this process.
///
/// Later calls are ignored: the knobs are resolved once, before any envelope
/// is emitted, so a second install would mean two different reductions in one
/// invocation and there is no correct answer to which should win.
pub fn set_process_agent_ops(ops: AgentOps) {
    let _ = PROCESS_OPS.set(ops);
}

/// The reduction knobs in force, or the empty set when none were installed.
#[must_use]
pub fn process_agent_ops() -> &'static AgentOps {
    PROCESS_OPS.get().unwrap_or(&NO_OPS)
}

/// What an envelope can honour, and where its rows live.
///
/// Declared per surface rather than inferred. Inference looks tempting until
/// you meet an envelope with two arrays: `doctor` carries both `checks` and
/// `failed_checks`, and `config effective` carries both `allowed_keys` and
/// `precedence`. Guessing which one the operator meant is exactly the kind of
/// invented semantics this module exists to avoid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvelopeShape {
    /// Surface name as the operator typed it; used verbatim in refusals.
    pub surface: &'static str,
    /// Key holding the row array, when the envelope has one.
    pub rows: Option<&'static str>,
    /// Discriminator key, always preserved through projection and counting.
    pub discriminator: Option<&'static str>,
    /// Keys whose values are contract IDENTITY and must survive `--truncate-content`.
    ///
    /// # Why the discriminator alone was not enough
    ///
    /// v1.0.4 exempted only the discriminator, on the reasoning that an
    /// envelope which no longer matches its own `const` is unroutable. That
    /// reasoning is right and it was applied to one key out of several.
    /// `schema --truncate-content 12` turned `invoke` from
    /// `duckduckgo-search-cli schema --name search-output` into
    /// `duckduckgo-s`, and `id` from `search-output` into `searc` — a command
    /// line that no longer runs and a name that `schema --name` rejects.
    ///
    /// The rule is one sentence: a string the agent hands back to a program is
    /// IDENTITY, not content. Config keys go back into `config set`, locale
    /// tags into `--ui-lang`, schema ids into `schema --name`. Shortening any
    /// of them produces output that looks valid and is not, which is the exact
    /// class this module exists to close.
    ///
    /// Declared per surface rather than inferred from a global key blocklist:
    /// `name` is identity on a doctor check and plain content on a search
    /// result, and only the surface knows which it is.
    pub identity: &'static [&'static str],
    /// Whether this envelope carries any prose for `--truncate-content` to shorten.
    ///
    /// # Why a surface can have nothing to truncate
    ///
    /// Exempting identity keys created a second way to be a no-op. On
    /// `config list` EVERY string is an identifier — the allowed key names go
    /// back into `config set`, the config path goes to the filesystem — so
    /// once identity was protected, `--truncate-content 4` returned 1138 bytes
    /// against a 1138-byte baseline. Byte-identical at exit 0 is precisely the
    /// shape of the defect this module exists to abolish, and the fact that
    /// this instance was CORRECT does not help the caller tell them apart.
    ///
    /// Declaring it statically means the surface refuses instead, and
    /// `commands` can publish the truth: an agent learns `--truncate-content`
    /// is unavailable here rather than discovering it by diffing byte counts.
    /// Static, not data-dependent: `config list` must answer the same way on
    /// an empty config as on a populated one.
    pub truncatable: bool,
}

impl EnvelopeShape {
    /// Shape of an envelope whose rows live under `rows`.
    #[must_use]
    pub const fn with_rows(
        surface: &'static str,
        rows: &'static str,
        discriminator: &'static str,
    ) -> Self {
        Self {
            surface,
            rows: Some(rows),
            discriminator: Some(discriminator),
            identity: &[],
            truncatable: true,
        }
    }

    /// Shape of an envelope that has no row array.
    #[must_use]
    pub const fn rowless(surface: &'static str, discriminator: &'static str) -> Self {
        Self {
            surface,
            rows: None,
            discriminator: Some(discriminator),
            identity: &[],
            truncatable: true,
        }
    }

    /// Declare the contract-identity keys of this envelope.
    ///
    /// The discriminator is protected regardless and does not need repeating,
    /// though listing it is harmless and keeps each entry readable on its own.
    #[must_use]
    pub const fn identified_by(mut self, identity: &'static [&'static str]) -> Self {
        self.identity = identity;
        self
    }

    /// Declare that this envelope is all identifiers, with no prose to shorten.
    #[must_use]
    pub const fn without_content(mut self) -> Self {
        self.truncatable = false;
        self
    }

    /// Whether `key` names contract identity on this envelope.
    #[must_use]
    fn is_identity(&self, key: &str) -> bool {
        Some(key) == self.discriminator || self.identity.contains(&key)
    }
}

/// Published capability matrix, one entry per introspection surface (SSOT).
///
/// `commands` exposes this so an agent learns the contract instead of
/// discovering it by collecting exit codes.
pub const SURFACES: &[EnvelopeShape] = &[
    EnvelopeShape::with_rows("doctor", "checks", "type")
        .identified_by(&["type", "id", "status", "check"]),
    EnvelopeShape::with_rows("schema", "schemas", "type").identified_by(&[
        "type",
        "id",
        "invoke",
        "discriminator",
        "discriminator_key",
    ]),
    // Locale is BCP-47 tags and the flag names that set them: identifiers all
    // the way down, with no prose a caller could want shortened.
    EnvelopeShape::with_rows("locale", "available", "type")
        .identified_by(&["type", "available", "active", "resolved", "requested"])
        .without_content(),
    // Config surfaces are key names and filesystem paths. Every string here
    // goes back into `config set` or into the filesystem.
    EnvelopeShape::with_rows("config list", "allowed_keys", "type")
        .identified_by(&["type", "allowed_keys", "config_file", "config_directory"])
        .without_content(),
    EnvelopeShape::with_rows("config effective", "allowed_keys", "type")
        .identified_by(&[
            "type",
            "allowed_keys",
            "config_file",
            "source",
            "precedence",
        ])
        .without_content(),
    EnvelopeShape::with_rows("init-config", "files", "type").identified_by(&[
        "type",
        "path",
        "config_file",
        "config_directory",
    ]),
    // The command tree is nested under `root`, not a flat array of rows.
    EnvelopeShape::rowless("commands", "type").identified_by(&["type", "name", "invoke"]),
    EnvelopeShape::rowless("config path", "type")
        .identified_by(&["type", "config_file", "config_directory"])
        .without_content(),
    EnvelopeShape::rowless("config get", "type")
        .identified_by(&["type", "key"])
        .without_content(),
    EnvelopeShape::rowless("config set", "type")
        .identified_by(&["type", "key", "action", "config_file"])
        .without_content(),
    EnvelopeShape::rowless("config unset", "type")
        .identified_by(&["type", "key", "action", "config_file"])
        .without_content(),
    // The two surfaces an agent actually spends its budget on. They were
    // absent until 2026-08-10, so `commands --json` published a thirteen-row
    // matrix that omitted search and deep-research — the operator asks the
    // binary what it supports and is told about `locale` but not about the
    // command it came for.
    //
    // Both honour all eight operators. Search rows are `SearchResult`, which
    // carries `content`; deep-research rows are aggregated and contentless,
    // but the envelope also carries `synthesis.body`, which is the largest
    // prose blob this CLI emits — see `agent_ops::apply_truncate_content_deep`.
    EnvelopeShape::with_rows("buscar", "results", "type").identified_by(&[
        "type",
        "query",
        "url",
        "display_url",
    ]),
    EnvelopeShape::with_rows("deep-research", "results", "kind").identified_by(&[
        "kind",
        "query",
        "url",
        "display_url",
        "format",
    ]),
    PROBE_SHAPE,
    PROBE_DEEP_SHAPE,
];

/// Shape of the `--probe` envelope.
///
/// # Why this is a named `const` and not just a row of [`SURFACES`]
///
/// The probe emits from nineteen call sites in `crate::probe`, which needs the
/// shape by value. Looking it up by name would either panic on a typo or need
/// a fallback that silently disagrees with the published matrix. Naming it
/// once and referencing it from both places means the capability an agent
/// reads and the capability the code enforces cannot drift apart.
///
/// # Why the probe family is here at all
///
/// The v1.0.4 plan named "the probe path" as a conversion target and it was
/// skipped while the class was declared closed. Measured on v1.0.4:
/// `--count-only`, `--limit 1`, `--fields status` and `--truncate-content 5`
/// each returned 633 bytes against a 633-byte baseline, at exit 0 — the exact
/// accepted-and-ignored behaviour this module exists to abolish, on the health
/// check surface an agent reaches for first.
pub const PROBE_SHAPE: EnvelopeShape = EnvelopeShape::rowless("--probe", "type").identified_by(&[
    "type",
    "status",
    "endpoint",
    "error_code",
    "url",
]);

/// Shape of the `--probe-deep` envelope. See [`PROBE_SHAPE`].
///
/// `cascade_reason` is identity: it is an interstitial IDENTIFIER an agent
/// branches on, not prose, and `cloudflare_turnstile` truncated to `cloudf`
/// matches nothing.
pub const PROBE_DEEP_SHAPE: EnvelopeShape = EnvelopeShape::rowless("--probe-deep", "type")
    .identified_by(&[
        "type",
        "status",
        "endpoint",
        "error_code",
        "url",
        "cascade_reason",
    ]);

/// The declared shape of `surface`, or `None` when the name is not published.
///
/// Callers used to hand-build their own [`EnvelopeShape`], which meant the row
/// key lived in two places: here, where the capability matrix is published for
/// agents, and again at the emit site. Two copies of the same fact drift, and
/// the drift is invisible until an operator hits the surface whose copy is
/// stale. Looking the shape up makes [`SURFACES`] the only definition.
#[must_use]
pub fn shape_for(surface: &str) -> Option<&'static EnvelopeShape> {
    SURFACES.iter().find(|s| s.surface == surface)
}

/// Names of the operations that need a row array, in application order.
const ROW_OPS: &[&str] = &[
    "--filter",
    "--sort",
    "--dedupe-by",
    "--limit",
    "--count-only",
];

/// Build a refusal carrying both renderings of one message.
///
/// Every refusal in this module goes through here, so no site can accidentally
/// emit a raw English `format!` again — which is exactly how the refusal text
/// stayed monolingual through v1.0.4 in a binary that ships `--ui-lang`.
fn refusal(code: &'static str, msg: crate::i18n::Message, pairs: &[(&str, &str)]) -> CliError {
    let (english, localized) = crate::i18n::bilingual(msg, pairs);
    CliError::AgentOpsRefused {
        code,
        english,
        localized,
    }
}

/// Refusal for an operation the envelope cannot express.
fn refuse(op: &str, shape: &EnvelopeShape) -> CliError {
    refusal(
        crate::error::codes::UNSUPPORTED_OPERATION,
        crate::i18n::Message::AgentOpsRowOpUnsupported,
        &[("op", op), ("surface", shape.surface)],
    )
}

/// Apply every requested reduction to `value`, or refuse.
///
/// Order is fixed and matches the search path: filter, sort, dedupe, limit,
/// count-only, fields, truncate. Filtering before limiting is the only order
/// under which `--limit` means "at most N of the rows I asked for".
///
/// # Errors
///
/// [`CliError::InvalidConfig`] when a row operation is requested on a rowless
/// envelope, when a `--fields` path matches nothing, or when a row operation
/// names a key the rows do not have.
pub fn apply_generic(
    value: &mut Value,
    ops: &AgentOps,
    shape: &EnvelopeShape,
) -> Result<(), CliError> {
    if ops.is_empty() {
        return Ok(());
    }

    let wants_rows = ops.filter.is_some()
        || ops.sort.is_some()
        || ops.dedupe_by.is_some()
        || ops.limit.is_some()
        || ops.count_only;

    if wants_rows {
        let Some(rows_key) = shape.rows else {
            let op = first_requested_row_op(ops);
            return Err(refuse(op, shape));
        };
        apply_row_ops(value, ops, shape, rows_key)?;
    }

    if let Some(ref spec) = ops.fields {
        project_paths(value, spec, shape)?;
    }
    if let Some(max) = ops.truncate_content {
        if !shape.truncatable {
            return Err(refusal(
                crate::error::codes::UNSUPPORTED_OPERATION,
                crate::i18n::Message::AgentOpsNothingToTruncate,
                &[("surface", shape.surface)],
            ));
        }
        truncate_strings(value, max as usize, shape);
    }
    Ok(())
}

/// The first row operation the caller asked for, for a precise refusal.
fn first_requested_row_op(ops: &AgentOps) -> &'static str {
    if ops.filter.is_some() {
        ROW_OPS[0]
    } else if ops.sort.is_some() {
        ROW_OPS[1]
    } else if ops.dedupe_by.is_some() {
        ROW_OPS[2]
    } else if ops.limit.is_some() {
        ROW_OPS[3]
    } else {
        ROW_OPS[4]
    }
}

/// Filter, sort, dedupe, limit and count the row array in place.
fn apply_row_ops(
    value: &mut Value,
    ops: &AgentOps,
    shape: &EnvelopeShape,
    rows_key: &str,
) -> Result<(), CliError> {
    let Some(obj) = value.as_object_mut() else {
        return Err(refusal(
            crate::error::codes::ENVELOPE_DEFECT,
            crate::i18n::Message::AgentOpsEnvelopeNotObject,
            &[("surface", shape.surface)],
        ));
    };
    let Some(Value::Array(rows)) = obj.get_mut(rows_key) else {
        return Err(refusal(
            crate::error::codes::ENVELOPE_DEFECT,
            crate::i18n::Message::AgentOpsRowsKeyMissing,
            &[("surface", shape.surface), ("rows", rows_key)],
        ));
    };

    if let Some(ref expr) = ops.filter {
        let pred = RowPredicate::parse(expr)?;
        rows.retain(|row| pred.matches(row));
    }
    if let Some(ref spec) = ops.sort {
        sort_rows(rows, spec, rows_key)?;
    }
    if let Some(ref key) = ops.dedupe_by {
        dedupe_rows(rows, key);
    }
    if let Some(limit) = ops.limit {
        rows.truncate(limit as usize);
    }

    if ops.count_only {
        let count = rows.len();
        let mut compact = Map::new();
        if let Some(disc) = shape.discriminator {
            if let Some(v) = obj.get(disc) {
                compact.insert(disc.to_string(), v.clone());
            }
        }
        compact.insert("count".to_string(), Value::from(count));
        *value = Value::Object(compact);
    }
    Ok(())
}

/// `--filter` predicate over one row: `key=v`, `key!=v` or `key~substring`.
enum RowPredicate {
    Eq(String, String),
    Ne(String, String),
    Contains(String, String),
}

impl RowPredicate {
    fn parse(raw: &str) -> Result<Self, CliError> {
        let invalid = || {
            refusal(
                crate::error::codes::INVALID_FILTER,
                crate::i18n::Message::AgentOpsFilterInvalid,
                &[("expr", raw)],
            )
        };
        if let Some((k, v)) = raw.split_once("!=") {
            return non_empty(k, v)
                .ok_or_else(invalid)
                .map(|(k, v)| Self::Ne(k, v));
        }
        if let Some((k, v)) = raw.split_once('~') {
            return non_empty(k, v)
                .ok_or_else(invalid)
                .map(|(k, v)| Self::Contains(k, v));
        }
        if let Some((k, v)) = raw.split_once('=') {
            return non_empty(k, v)
                .ok_or_else(invalid)
                .map(|(k, v)| Self::Eq(k, v));
        }
        Err(invalid())
    }

    fn matches(&self, row: &Value) -> bool {
        let (key, want, kind) = match self {
            Self::Eq(k, v) => (k, v, 0u8),
            Self::Ne(k, v) => (k, v, 1),
            Self::Contains(k, v) => (k, v, 2),
        };
        let have = row.get(key).map(scalar_to_string);
        match (kind, have) {
            // A row without the key never matches a positive test, and always
            // satisfies a negative one — absence is not equality.
            (0, Some(h)) => h == *want,
            (0, None) => false,
            (1, Some(h)) => h != *want,
            (1, None) => true,
            (_, Some(h)) => h.contains(want.as_str()),
            (_, None) => false,
        }
    }
}

/// Trim and reject empty halves of a `key<op>value` pair.
fn non_empty(k: &str, v: &str) -> Option<(String, String)> {
    let (k, v) = (k.trim(), v.trim());
    (!k.is_empty() && !v.is_empty()).then(|| (k.to_string(), v.to_string()))
}

/// Compare a JSON scalar as the text an operator would have typed.
fn scalar_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Order rows by a row key, `KEY` or `KEY:asc` / `KEY:desc`.
fn sort_rows(rows: &mut [Value], spec: &str, rows_key: &str) -> Result<(), CliError> {
    let (key, desc) = match spec.split_once(':') {
        Some((k, "desc")) => (k.trim(), true),
        Some((k, "asc")) => (k.trim(), false),
        Some((_, other)) => {
            return Err(refusal(
                crate::error::codes::INVALID_SORT,
                crate::i18n::Message::AgentOpsSortDirectionInvalid,
                &[("direction", other)],
            ));
        }
        None => (spec.trim(), false),
    };
    if key.is_empty() {
        return Err(refusal(
            crate::error::codes::INVALID_SORT,
            crate::i18n::Message::AgentOpsSortKeyMissing,
            &[],
        ));
    }
    if !rows.is_empty() && !rows.iter().any(|r| r.get(key).is_some()) {
        return Err(refusal(
            crate::error::codes::INVALID_SORT,
            crate::i18n::Message::AgentOpsSortKeyUnknown,
            &[
                ("key", key),
                ("rows", rows_key),
                ("available", &available_row_keys(rows)),
            ],
        ));
    }
    rows.sort_by(|a, b| {
        let (x, y) = (a.get(key), b.get(key));
        let ord = compare_json(x, y);
        if desc {
            ord.reverse()
        } else {
            ord
        }
    });
    Ok(())
}

/// Total order over the JSON scalars a row key can hold.
///
/// Numbers compare numerically, everything else lexicographically, and a
/// missing key always sorts last so it cannot masquerade as the smallest value.
fn compare_json(a: Option<&Value>, b: Option<&Value>) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(nx), Some(ny)) => nx.partial_cmp(&ny).unwrap_or(Ordering::Equal),
            _ => scalar_to_string(x).cmp(&scalar_to_string(y)),
        },
    }
}

/// Row keys present anywhere in the array, for an actionable error message.
fn available_row_keys(rows: &[Value]) -> String {
    let mut keys: Vec<&str> = rows
        .iter()
        .filter_map(Value::as_object)
        .flat_map(|o| o.keys().map(String::as_str))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    if keys.is_empty() {
        "(rows are not objects)".to_string()
    } else {
        keys.join(", ")
    }
}

/// Drop later rows that repeat an earlier value of `key`.
///
/// Rows missing the key are always kept: they have nothing to be a duplicate
/// of, and silently collapsing them would lose data the operator never asked
/// to lose.
fn dedupe_rows(rows: &mut Vec<Value>, key: &str) {
    let mut seen = std::collections::HashSet::new();
    rows.retain(|row| match row.get(key) {
        Some(v) => seen.insert(scalar_to_string(v)),
        None => true,
    });
}

/// Keep only the dotted paths in `spec`, plus the discriminator.
///
/// The discriminator always survives. Projecting it away would leave an agent
/// holding a document it can no longer route, which is a worse outcome than
/// the handful of bytes the key costs.
fn project_paths(value: &mut Value, spec: &str, shape: &EnvelopeShape) -> Result<(), CliError> {
    let paths: Vec<&str> = spec
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if paths.is_empty() {
        return Err(refusal(
            crate::error::codes::INVALID_FIELDS_PATH,
            crate::i18n::Message::AgentOpsFieldsEmpty,
            &[],
        ));
    }

    let mut out = Map::new();
    if let (Some(disc), Some(obj)) = (shape.discriminator, value.as_object()) {
        if let Some(v) = obj.get(disc) {
            out.insert(disc.to_string(), v.clone());
        }
    }
    // MEASURED AND REJECTED (v1.0.5): a single-segment path names a whole
    // top-level subtree and `value` is replaced wholesale two lines below, so
    // the subtree could be MOVED out of the source map instead of deep-cloned.
    // It was implemented and then backed out, because moving mutates `value`
    // before the later paths are resolved: on `--fields checks,nonexistent`,
    // `path_error` would then report the surviving keys with `checks` already
    // gone and send the reader looking in the wrong place. Restoring the
    // diagnostic would cost a second resolution pass over the tree.
    //
    // Envelopes here measure tens of KiB and a projection clones at most a
    // handful of subtrees, so the trade is a worse error message on a contract
    // surface in exchange for an allocation nobody can perceive. Declined
    // deliberately; the allocation win that WAS free went into
    // `truncate_strings`, which ran once per string field.
    for path in &paths {
        let picked = pick_path(value, path).ok_or_else(|| path_error(value, path, shape))?;
        merge_path(&mut out, path, picked);
    }
    *value = Value::Object(out);
    Ok(())
}

/// Explain a `--fields` miss at the level where the path actually broke.
///
/// Reporting the top-level keys for `checks.id` sends the reader looking in the
/// wrong place: `checks` exists, and the real mistake is one level down, where
/// the column is `name`. This walks the resolvable prefix and lists what is
/// available exactly there.
fn path_error(value: &Value, path: &str, shape: &EnvelopeShape) -> CliError {
    let mut cursor = value;
    let mut walked: Vec<&str> = Vec::new();
    for segment in path.split('.') {
        let next = match cursor {
            Value::Object(map) => map.get(segment),
            Value::Array(items) => items
                .first()
                .and_then(Value::as_object)
                .and_then(|o| o.get(segment)),
            _ => None,
        };
        match next {
            Some(child) => {
                walked.push(segment);
                cursor = child;
            }
            None => {
                let available = keys_at(cursor);
                return if walked.is_empty() {
                    refusal(
                        crate::error::codes::INVALID_FIELDS_PATH,
                        crate::i18n::Message::AgentOpsFieldsPathUnknownTop,
                        &[
                            ("path", path),
                            ("segment", segment),
                            ("surface", shape.surface),
                            ("available", &available),
                        ],
                    )
                } else {
                    refusal(
                        crate::error::codes::INVALID_FIELDS_PATH,
                        crate::i18n::Message::AgentOpsFieldsPathUnknownNested,
                        &[
                            ("path", path),
                            ("segment", segment),
                            ("parent", &walked.join(".")),
                            ("available", &available),
                        ],
                    )
                };
            }
        }
    }
    refusal(
        crate::error::codes::INVALID_FIELDS_PATH,
        crate::i18n::Message::AgentOpsFieldsPathInvalid,
        &[("path", path), ("surface", shape.surface)],
    )
}

/// Keys readable at a cursor, descending one array level to reach row columns.
fn keys_at(value: &Value) -> String {
    let obj = match value {
        Value::Object(map) => Some(map),
        Value::Array(items) => items.first().and_then(Value::as_object),
        _ => None,
    };
    obj.map_or_else(
        || "(no keys at this level)".to_string(),
        |o| o.keys().cloned().collect::<Vec<_>>().join(", "),
    )
}

/// Resolve a dotted path, descending into arrays element-wise.
///
/// `checks.id` on `{"checks":[{"id":"a"},{"id":"b"}]}` yields
/// `[{"id":"a"},{"id":"b"}]` rather than `["a","b"]`, so the shape of the
/// envelope survives projection and a consumer's array index still means what
/// it meant before.
fn pick_path(value: &Value, path: &str) -> Option<Value> {
    let (head, rest) = match path.split_once('.') {
        Some((h, r)) => (h, Some(r)),
        None => (path, None),
    };
    match value {
        Value::Object(map) => {
            let child = map.get(head)?;
            match rest {
                None => Some(child.clone()),
                Some(r) => descend(child, r),
            }
        }
        // An EMPTY array projects to an empty array. A non-empty array whose
        // elements all lack the path is a typo and must surface as one —
        // conflating the two turns `--filter` down to zero rows into a bogus
        // "no such key" error.
        Value::Array(items) if items.is_empty() => Some(Value::Array(Vec::new())),
        Value::Array(items) => {
            let projected: Vec<Value> = items.iter().filter_map(|i| pick_path(i, path)).collect();
            (!projected.is_empty()).then_some(Value::Array(projected))
        }
        _ => None,
    }
}

/// Continue a dotted path below the head segment, keeping arrays as arrays.
fn descend(child: &Value, rest: &str) -> Option<Value> {
    match child {
        // See `pick_path`: empty is a legitimate projection, not a typo.
        Value::Array(items) if items.is_empty() => Some(Value::Array(Vec::new())),
        Value::Array(items) => {
            let projected: Vec<Value> = items
                .iter()
                .filter_map(|item| {
                    let leaf = pick_path(item, rest)?;
                    let mut row = Map::new();
                    merge_path(&mut row, rest, leaf);
                    Some(Value::Object(row))
                })
                .collect();
            (!projected.is_empty()).then_some(Value::Array(projected))
        }
        other => pick_path(other, rest),
    }
}

/// Insert `leaf` at `path` in `out`, creating intermediate objects.
fn merge_path(out: &mut Map<String, Value>, path: &str, leaf: Value) {
    match path.split_once('.') {
        None => {
            out.insert(path.to_string(), leaf);
        }
        Some((head, _)) => {
            // `pick_path` already resolved the tail, so the head is the only
            // level that still needs to exist in the projected object.
            out.insert(head.to_string(), leaf);
        }
    }
}

/// Cap every non-identity string in the tree at `max` Unicode scalars.
///
/// # What is exempt, and why the discriminator alone was not
///
/// v1.0.4 exempted the discriminator only: `--truncate-content 8` turned
/// `"config_path"` into `"config_p"`, a value no published schema declares,
/// and an envelope that no longer matches its own `const` is unroutable.
/// The reasoning was right and the scope was one key wide. `schema
/// --truncate-content 12` still turned `invoke` into `duckduckgo-s` and `id`
/// into `searc` — a command line that no longer runs and a name
/// `schema --name` rejects. Both are contract, both are now covered by
/// [`EnvelopeShape::identity`].
///
/// Everything else still shrinks, which is the point: the exemption must not
/// quietly turn the flag into the no-op it was introduced to fix.
///
/// # Why this truncates in place
///
/// `s.chars().take(max).collect()` walked the string twice and allocated a
/// fresh `String` for every field on every envelope. Locating the byte offset
/// of the `max`-th scalar and calling [`String::truncate`] does one pass and
/// no allocation, and cannot split a UTF-8 sequence because `char_indices`
/// only ever yields boundaries.
fn truncate_strings(value: &mut Value, max: usize, shape: &EnvelopeShape) {
    match value {
        Value::String(s) => {
            if let Some((cut, _)) = s.char_indices().nth(max) {
                s.truncate(cut);
            }
        }
        Value::Array(items) => {
            for item in items {
                truncate_strings(item, max, shape);
            }
        }
        Value::Object(map) => {
            for (key, v) in map.iter_mut() {
                if shape.is_identity(key.as_str()) {
                    continue;
                }
                truncate_strings(v, max, shape);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "envelope_ops_tests.rs"]
mod tests;
