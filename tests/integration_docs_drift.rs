// SPDX-License-Identifier: MIT OR Apache-2.0
//! Drift guard between the compiled CLI surface and the documented one.
//!
//! # Why a ruler and not a generator
//!
//! Until v1.0.4 this job belonged to `scripts/regen_cli_flags_readme.py`: 228
//! lines of Python in a repository whose stated contract is self-contained and
//! Rust-native, in a project where Python is forbidden. Worse than the language
//! was the shape. A generator only helps the person who remembers to run it,
//! and nobody had: the committed inventory was produced by binary 1.0.2 and
//! listed 66 root flags while the binary shipped 72. The documentation
//! described a product that no longer existed, and nothing said so.
//!
//! A ruler cannot be forgotten. It runs in `cargo test-all` like everything
//! else, and it fails with the exact set of flags that drifted.
//!
//! # Updating the snapshot
//!
//! `docs/generated/cli-flags-inventory.json` is data, so it needs a writer.
//! That writer is the `#[ignore]`d test at the bottom rather than an env var,
//! because this project forbids product environment variables and a test
//! harness should not teach a habit the product refuses:
//!
//! ```text
//! cargo test --all-features --locked --test integration_docs_drift -- \
//!     --ignored regenerate_flag_inventory
//! ```

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[path = "common/language.rs"]
mod language;

/// Repository root, resolved from the manifest rather than the cwd.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `--help` text of the binary under test, for the given argv prefix.
fn help_text(args: &[&str]) -> String {
    let mut cmd =
        assert_cmd::Command::cargo_bin("duckduckgo-search-cli").expect("compiled binary exists");
    cmd.args(args).arg("--help");
    let out = cmd.output().expect("help runs");
    // clap writes long help to stdout; keep stderr as a fallback so a future
    // change of stream does not silently empty this ruler.
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if stdout.trim().is_empty() {
        String::from_utf8_lossy(&out.stderr).into_owned()
    } else {
        stdout
    }
}

/// Long flags a help text advertises, minus the two clap always adds.
fn long_flags(help: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let bytes: Vec<char> = help.chars().collect();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] == '-' && bytes[i + 1] == '-' && bytes[i + 2].is_ascii_lowercase() {
            let mut j = i + 2;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == '-') {
                j += 1;
            }
            let name: String = bytes[i + 2..j].iter().collect();
            let name = name.trim_end_matches('-').to_string();
            if !name.is_empty() {
                out.insert(name);
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out.remove("help");
    out.remove("version");
    out
}

/// A line with its ANSI SGR sequences removed.
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        // Consume `[ ... m`; a malformed sequence simply ends at the line end.
        for c in chars.by_ref() {
            if c.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
}

/// Long flags a help text DECLARES, reading only the option column.
///
/// # Why this exists next to [`long_flags`]
///
/// [`long_flags`] scans an entire text, which is correct for a prose document:
/// a README mentions a flag wherever it likes. Pointed at `--help` it is wrong,
/// because clap's DESCRIPTIONS are prose too. `--chrome-headless` is described
/// as "Force headless Chrome (`--headless=new`)", and `--num` carries the tip
/// line "a similar argument exists: '--name'". The scanner read both as flags,
/// so `docs/generated/cli-flags-inventory.json` published `headless` and `name`
/// in `root_longs` and counted them in `root_count`. The binary rejects both
/// with exit 2.
///
/// The phantom ruler could not see it: it compared the generated tables against
/// this same extractor, so the artifact and its guard shared one defect and
/// agreed. A ruler that derives its expectation from the thing it measures
/// cannot disagree with it. This function reads the option column only, and
/// [`every_documented_root_flag_is_accepted_by_clap`] settles the question with
/// the binary instead of with another parse of the same text.
fn declared_flags(help: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in help.lines() {
        // clap emits SGR sequences even when stdout is a pipe, so the option
        // column starts with ESC rather than with `-`. Dropping them first is
        // what makes the "line begins with a dash" test mean what it says.
        let plain = strip_ansi(line);
        let trimmed = plain.trim_start();
        if !trimmed.starts_with('-') {
            continue;
        }
        // clap puts the description after two or more spaces, or on the next
        // line entirely. Either way the option column ends at the first gap.
        let spec = trimmed.split("  ").next().unwrap_or(trimmed);
        out.extend(long_flags(spec));
    }
    out
}

/// Long flags of the ROOT command, which is what the README tables document.
fn live_root_flags() -> BTreeSet<String> {
    declared_flags(&help_text(&[]))
}

/// Crate version reported by the binary under test.
fn live_version() -> String {
    let out = assert_cmd::Command::cargo_bin("duckduckgo-search-cli")
        .expect("compiled binary exists")
        .arg("--version")
        .output()
        .expect("version runs");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_string()
}

fn inventory_path() -> PathBuf {
    repo_root().join("docs/generated/cli-flags-inventory.json")
}

fn read_inventory() -> serde_json::Value {
    let raw = std::fs::read_to_string(inventory_path()).expect("flag inventory exists");
    serde_json::from_str(&raw).expect("flag inventory is JSON")
}

/// The published inventory must describe the binary that is actually built.
#[test]
fn flag_inventory_matches_the_compiled_binary() {
    let inventory = read_inventory();
    let documented: BTreeSet<String> = inventory["root_longs"]
        .as_array()
        .expect("root_longs is an array")
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    let live = live_root_flags();

    let missing: Vec<&String> = live.difference(&documented).collect();
    let stale: Vec<&String> = documented.difference(&live).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "docs/generated/cli-flags-inventory.json has drifted from the binary.\n\
         Undocumented (in the binary, absent from the inventory): {missing:?}\n\
         Stale (in the inventory, gone from the binary): {stale:?}\n\
         Regenerate with:\n  cargo test --all-features --locked --test \
         integration_docs_drift -- --ignored regenerate_flag_inventory"
    );

    assert_eq!(
        inventory["version"].as_str().unwrap_or_default(),
        live_version(),
        "the inventory records a different crate version than the binary reports"
    );
    assert_eq!(
        inventory["root_count"].as_u64().unwrap_or_default() as usize,
        live.len(),
        "root_count disagrees with root_longs"
    );
}

/// Every flag the binary advertises must appear in BOTH READMEs.
///
/// The two files are separate SSOTs — English prose lives only in `README.md`
/// and Portuguese only in `README.pt-BR.md` — so a flag documented in one and
/// forgotten in the other is a real defect for half the readers.
#[test]
fn every_live_flag_is_documented_in_both_readmes() {
    let live = live_root_flags();
    for readme in ["README.md", "README.pt-BR.md"] {
        let text = std::fs::read_to_string(repo_root().join(readme)).expect("readme exists");
        let undocumented: Vec<&String> = live
            .iter()
            .filter(|flag| !text.contains(&format!("--{flag}")))
            .collect();
        assert!(
            undocumented.is_empty(),
            "{readme} does not mention these flags the binary advertises: {undocumented:?}"
        );
    }
}

/// Every documentation file that claims to be a flag reference, and every flag.
///
/// # The class this closes
///
/// `every_live_flag_is_documented_in_both_readmes` names two files and measures
/// ROOT flags only. Both narrowings were invisible, and both cost something.
///
/// `llms-full.txt` — the artifact an agent loads when it wants the whole product
/// in context — sat 27 flags behind the binary and stated `--global-timeout`
/// default `60` in three places when the real default is `180`. Every gate was
/// green the entire time, because no ruler had that file in scope.
///
/// The subcommand narrowing is the same shape one level down: `--synth-format`,
/// `--aggregate`, `--depth` and fourteen others live only under their
/// subcommand, so a root-only ruler could never see them go undocumented.
///
/// This is the ninth instance of the family this repository keeps meeting: a
/// guard that picks its own scope measures only that scope, and reports green
/// for everything outside it.
#[test]
fn every_documented_flag_reference_covers_the_live_surface() {
    /// Files whose contract is to name every flag the binary advertises.
    ///
    /// `llms.txt` and `llms.pt-BR.txt` are deliberately absent: their contract
    /// is the llmstxt.org discovery STUB, which must stay short. Forcing the
    /// full surface into them would pit one written rule against another.
    /// `docs/AGENTS.md` joined in v1.0.5. It is the agent CONTRACT, and it was
    /// organised as one section per release — a delta log. A flag that shipped
    /// and never changed again was named once in its release section or, if it
    /// shipped quietly, never. The audit measured twenty live flags missing
    /// from it while every documentation gate was green, because no gate had
    /// ever pointed at `docs/`.
    const FLAG_REFERENCES: &[&str] = &[
        "README.md",
        "README.pt-BR.md",
        "llms-full.txt",
        "docs/AGENTS.md",
        "docs/AGENTS.pt-BR.md",
    ];

    let mut live = live_root_flags();
    for sub in SUBCOMMANDS_WITH_OWN_FLAGS {
        live.extend(declared_flags(&help_text(&[sub])));
    }
    assert!(
        live.len() > 80,
        "the live flag surface collapsed to {} entries, which means the extractor \
         stopped seeing help text rather than that the CLI shrank",
        live.len()
    );

    for reference in FLAG_REFERENCES {
        let text = std::fs::read_to_string(repo_root().join(reference))
            .unwrap_or_else(|e| panic!("{reference} is readable: {e}"));
        let undocumented: Vec<&String> = live
            .iter()
            .filter(|flag| !text.contains(&format!("--{flag}")))
            .collect();
        assert!(
            undocumented.is_empty(),
            "{reference} does not mention these flags the binary advertises: {undocumented:?}\n\n\
             Every file in FLAG_REFERENCES promises the whole surface. If a flag \
             genuinely does not belong there, the honest fix is to narrow the \
             promise in prose, not to leave the reader a shorter product."
        );
    }
}

/// Subcommands that publish flags absent from the root help.
///
/// Kept next to its only two consumers so the two cannot disagree about which
/// subcommands carry their own surface.
const SUBCOMMANDS_WITH_OWN_FLAGS: &[&str] =
    &["deep-research", "doctor", "init-config", "schema", "man"];

/// Switches that belong to another program and are quoted as such.
///
/// `--headless=new` is Chrome's own switch, named inside the description of our
/// `--chrome-headless`. Removing the mention would make the table less useful;
/// pretending the mention is our flag made the guard wrong for months.
const FOREIGN_SWITCHES: &[(&str, &str)] = &[(
    "headless",
    "Chrome's `--headless=new`, quoted in the description of our --chrome-headless",
)];

/// Does clap accept this long flag on the root command?
///
/// An unknown trailing token forces argv parsing to fail before any work runs,
/// so this answers the question without a search, a browser, or a socket.
fn clap_accepts_root_flag(flag: &str) -> bool {
    const SENTINEL: &str = "--this-token-is-not-a-flag";
    let long = format!("--{flag}");
    let out = assert_cmd::Command::cargo_bin("duckduckgo-search-cli")
        .expect("compiled binary exists")
        .args([long.as_str(), SENTINEL])
        .output()
        .expect("argv parsing runs");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    !text.contains(&format!("unexpected argument '{long}'"))
}

/// The generated flag tables must not advertise flags the binary rejects.
///
/// # Why this asks the binary rather than the help text
///
/// The first version compared the tables against a parse of `--help`, and it
/// was wrong in both directions at once. `--region` and `--max-concurrency` are
/// hidden clap aliases: real, accepted, absent from the option column, so the
/// guard called two working flags phantoms. And `headless`, lifted out of a
/// description, was a phantom the guard could never see, because the extractor
/// it trusted had invented it.
///
/// Acceptance by clap is the only definition that a reader copying a flag out
/// of the table actually cares about.
#[test]
fn generated_flag_tables_have_no_phantom_flags() {
    let exempt: BTreeSet<&str> = FOREIGN_SWITCHES.iter().map(|(f, _)| *f).collect();
    let mut checked = 0usize;

    for generated in ["docs/generated/flags_en.md", "docs/generated/flags_pt.md"] {
        let path = repo_root().join(generated);
        if !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("generated table is readable");
        let mut phantoms = Vec::new();
        for flag in long_flags(&text) {
            if exempt.contains(flag.as_str()) {
                continue;
            }
            checked += 1;
            if !clap_accepts_root_flag(&flag) && !subcommand_declares(&flag) {
                phantoms.push(flag);
            }
        }
        assert!(
            phantoms.is_empty(),
            "{generated} documents flags the binary rejects: {phantoms:?}\n\n\
             Each was passed to the binary and clap answered \"unexpected \
             argument\". Delete the row, or add the token to FOREIGN_SWITCHES \
             with the program it really belongs to."
        );
    }

    assert!(
        checked > 40,
        "only {checked} flags were checked, so the tables were empty or unreadable"
    );
}

/// Is this flag published by one of the subcommands rather than by the root?
fn subcommand_declares(flag: &str) -> bool {
    SUBCOMMANDS_WITH_OWN_FLAGS
        .iter()
        .any(|sub| declared_flags(&help_text(&[sub])).contains(flag))
}

/// Every cargo alias must be named in both testing guides.
///
/// # Why this is not decoration
///
/// The project has no CI, so `.cargo/config.toml` is the whole pipeline: the
/// seventeen aliases there ARE the gates. `docs/TESTING.md` is the document a
/// contributor opens to learn how to run them, and in v1.0.5 it named ZERO of
/// them — it spoke of `cargo test`, `cargo check`, `cargo clippy`, plus
/// `nextest` and `llvm-cov` invocations that are not how this repository is
/// gated. A reader following the testing guide would never run
/// `check-windows`, `check-macos`, `lint-nohttp` or `docs-nohttp`, which are
/// precisely the gates that exist because v1.0.2 shipped without compiling on
/// macOS or Windows.
#[test]
fn every_cargo_alias_is_documented_in_the_testing_guide() {
    let config = std::fs::read_to_string(repo_root().join(".cargo/config.toml"))
        .expect(".cargo/config.toml is readable");

    let aliases: BTreeSet<String> = config
        .lines()
        .skip_while(|l| l.trim() != "[alias]")
        .skip(1)
        .take_while(|l| !l.trim_start().starts_with('['))
        .filter_map(|l| l.split_once(" = "))
        .map(|(name, _)| name.trim().to_string())
        .filter(|n| !n.is_empty() && !n.starts_with('#'))
        .collect();
    assert!(
        aliases.len() >= 15,
        "only {} aliases were parsed from .cargo/config.toml, so the [alias] \
         section moved or the parser broke: {aliases:?}",
        aliases.len()
    );

    let mut missing = Vec::new();
    for guide in ["docs/TESTING.md", "docs/TESTING.pt-BR.md"] {
        let text = std::fs::read_to_string(repo_root().join(guide))
            .unwrap_or_else(|e| panic!("{guide} is readable: {e}"));
        for alias in &aliases {
            if !text.contains(&format!("cargo {alias}")) {
                missing.push(format!("{guide}: cargo {alias}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "these gates exist but the testing guide never names them:\n  {}\n\n\
         With no CI, an undocumented alias is a gate nobody runs.",
        missing.join("\n  ")
    );
}

/// Documents whose JSON examples describe the CURRENT envelope.
///
/// `MIGRATION*` and `decisions/` are absent on purpose: both record what a past
/// release emitted, and rewriting them would destroy the only value they have.
const CURRENT_STATE_DOCUMENTS: &[&str] = &[
    "docs/AGENTS.md",
    "docs/AGENTS.pt-BR.md",
    "docs/AGENTS-GUIDE.md",
    "docs/AGENTS-GUIDE.pt-BR.md",
    // Added 2026-08-21. Its absence is why the wire drift below could rot with
    // every gate green: nine rules (R12, R13, R14, R15, R17, R33, R34, R43 and
    // the whole v0.8.9 section) told agents to read `.titulo` and `.posicao`,
    // while line 8 of that same file already declared English keys the default
    // since v1.0.2. Under the EN default those recipes return `null` in
    // silence, which is the worst failure mode an agent can be handed — no
    // error, no exit code, just an empty field. 56 lines were corrected.
    "docs/AGENT_RULES.md",
    "docs/COOKBOOK.md",
    "docs/COOKBOOK.pt-BR.md",
    "docs/HOW_TO_USE.md",
    "docs/HOW_TO_USE.pt-BR.md",
    "docs/INTEGRATIONS.md",
    "docs/INTEGRATIONS.pt-BR.md",
];

/// Quoted keys that belong to something other than this CLI's envelope.
const KEYS_FROM_ELSEWHERE: &[(&str, &str)] = &[
    (
        "metadados",
        "legacy PT wire alias, reachable with --wire-keys pt",
    ),
    (
        "role",
        "OpenAI chat message shape, in an integration example",
    ),
    (
        "tools",
        "OpenAI tool-calling shape, and a shell script's own report",
    ),
    ("parameters", "OpenAI function-schema shape"),
    ("run", "Continue `config.json` slash-command field"),
    (
        "all_present",
        "output of a dependency-check script written in the cookbook",
    ),
    ("found", "same dependency-check script"),
    (
        "web",
        "a jaq-CONSTRUCTED object in a dual-vertical recipe, not a wire key",
    ),
    ("vertical", "a jaq-constructed grouping key, not a wire key"),
];

/// Every JSON key a current-state document quotes must exist in a schema.
///
/// # Why the `jaq` ruler was not enough
///
/// [`every_jaq_path_in_the_documentation_exists_in_a_schema`] reads paths that
/// OPEN a `jaq` program. It could not see a key sitting inside a pretty-printed
/// response example, and that is where three renamed fields were still living
/// in v1.0.5: `cascata_motivo` and `sugestao_mitigacao`, renamed in v1.0.3 to
/// `cascade_reason` and `mitigation_suggestion` — the published schema never
/// declared the Portuguese ones — plus `title_original`, which the wire has
/// always called `original_title`, and `retentativas`, whose own schema entry
/// records that "the English wire always emitted `retries`, so the old name
/// never matched".
///
/// Twelve documents showed a reader an envelope the binary does not emit.
#[test]
fn every_quoted_json_key_in_current_documentation_exists_in_a_schema() {
    let mut known = schema_keys();
    known.extend(KEYS_FROM_ELSEWHERE.iter().map(|(k, _)| (*k).to_string()));

    let mut phantoms = Vec::new();
    let mut cited = 0usize;
    for doc in CURRENT_STATE_DOCUMENTS {
        let text = std::fs::read_to_string(repo_root().join(doc))
            .unwrap_or_else(|e| panic!("{doc} is readable: {e}"));
        for key in quoted_json_keys(&text) {
            cited += 1;
            if !known.contains(&key) {
                phantoms.push(format!("{doc}: \"{key}\""));
            }
        }
    }

    assert!(
        cited > 100,
        "only {cited} quoted keys were found, so this ruler measured almost nothing"
    );
    phantoms.sort();
    phantoms.dedup();
    assert!(
        phantoms.is_empty(),
        "these documented JSON keys exist in no published schema:\n  {}\n\n\
         A renamed field leaves the example looking right and the parse coming \
         back empty. Fix the key, or add it to KEYS_FROM_ELSEWHERE with the \
         shape it really belongs to.",
        phantoms.join("\n  ")
    );
}

/// Every key harvested from the published schemas.
fn schema_keys() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let dir = repo_root().join("docs/schemas");
    for entry in std::fs::read_dir(&dir).expect("docs/schemas is readable") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_some_and(|x| x == "json") {
            let raw = std::fs::read_to_string(&path).expect("schema is readable");
            let value: serde_json::Value = serde_json::from_str(&raw).expect("schema is JSON");
            collect_object_keys(&value, &mut out);
        }
    }
    assert!(
        out.len() > 100,
        "only {} keys were harvested from docs/schemas",
        out.len()
    );
    out
}

/// Lowercase identifiers quoted as JSON KEYS, i.e. followed by a colon.
fn quoted_json_keys(text: &str) -> BTreeSet<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = BTreeSet::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '"' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < chars.len()
            && (chars[j].is_ascii_lowercase() || chars[j].is_ascii_digit() || chars[j] == '_')
        {
            j += 1;
        }
        // Require the closing quote, then a colon, so a quoted VALUE is skipped.
        if j > i + 3 && j < chars.len() && chars[j] == '"' {
            let after = chars[j + 1..].iter().position(|c| !c.is_whitespace());
            if after.map(|k| chars[j + 1 + k]) == Some(':') {
                out.insert(chars[i + 1..j].iter().collect());
            }
        }
        i = j.max(i + 1);
    }
    out
}

/// Every root flag the inventory publishes must be one clap actually accepts.
///
/// # Why parsing help twice is not evidence
///
/// [`flag_inventory_matches_the_compiled_binary`] compares the published
/// inventory with [`live_root_flags`], and [`generated_flag_tables_have_no_phantom_flags`]
/// compares the two tables with the same function. Three green gates, one
/// source: whatever the extractor got wrong, all three agreed on. It got two
/// things wrong — `headless`, lifted from the description of `--chrome-headless`,
/// and `name`, lifted from clap's "a similar argument exists: '--name'" tip —
/// and `docs/generated/cli-flags-inventory.json` shipped both in `root_longs`
/// with `root_count` at 70 instead of 68.
///
/// This ruler asks the binary instead of the text. A flag is real when clap
/// does not call it unexpected. Nothing here runs a search: an unknown trailing
/// token guarantees argv parsing fails first, so the process exits before any
/// Chrome, network, or file work begins.
#[test]
fn every_documented_root_flag_is_accepted_by_clap() {
    /// A token clap cannot know, used to force a parse error with zero work.
    const SENTINEL: &str = "--this-token-is-not-a-flag";

    let documented: BTreeSet<String> = read_inventory()["root_longs"]
        .as_array()
        .expect("root_longs is an array")
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    assert!(
        documented.len() > 40,
        "the inventory published only {} root flags, which is too few to be the \
         real surface — the extractor probably broke",
        documented.len()
    );

    let mut rejected = Vec::new();
    for flag in &documented {
        let long = format!("--{flag}");
        let out = assert_cmd::Command::cargo_bin("duckduckgo-search-cli")
            .expect("compiled binary exists")
            .args([long.as_str(), SENTINEL])
            .output()
            .expect("argv parsing runs");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        if text.contains(&format!("unexpected argument '{long}'")) {
            rejected.push(long);
        }
    }

    // Non-vacuity: the sentinel must itself be rejected, or the loop above is
    // measuring a binary that accepts anything and every flag looks real.
    let control = assert_cmd::Command::cargo_bin("duckduckgo-search-cli")
        .expect("compiled binary exists")
        .arg(SENTINEL)
        .output()
        .expect("argv parsing runs");
    let control_text = format!(
        "{}{}",
        String::from_utf8_lossy(&control.stdout),
        String::from_utf8_lossy(&control.stderr)
    );
    assert!(
        control_text.contains(&format!("unexpected argument '{SENTINEL}'")),
        "the control token was not rejected, so this ruler proves nothing about \
         the flags above. clap's wording changed — update the needle."
    );

    assert!(
        rejected.is_empty(),
        "the inventory publishes root flags the binary rejects: {rejected:?}\n\n\
         A documented flag that exits 2 is worse than an undocumented one: the \
         reader copies it and gets a usage error. Regenerate with:\n  \
         cargo test --all-features --locked --test integration_docs_drift -- \
         --ignored regenerate_flag_inventory"
    );
}

/// Top-level JSON paths a document may cite without the schemas knowing them.
const KNOWN_FOREIGN_PATHS: &[(&str, &str)] = &[
    (
        "resultados",
        "legacy PT wire alias, reachable with --wire-keys pt",
    ),
    ("noticias", "legacy PT wire alias"),
    ("metadados", "legacy PT wire alias"),
    ("buscas", "legacy PT wire alias for the multi-query root"),
    ("quantidade_resultados", "legacy PT wire alias"),
    ("sintese", "legacy PT deserialize alias of `synthesis`"),
    (
        "choices",
        "OpenAI response shape, in an integration example",
    ),
];

/// Every JSON path a document tells the reader to `jaq` must exist somewhere.
///
/// # The defect this closes
///
/// `deep-research --synthesize` serialises its report under `synthesis`, with
/// `sintese` as a deserialize alias. Seven documents told the reader to run
/// `jaq -r '.synth'`, which is the Rust FIELD name — it never crossed the wire.
/// `jaq` answers `null` and exits 0, so the recipe fails silently and the agent
/// reports an empty synthesis rather than a broken command.
///
/// Prose about a field cannot be spell-checked, but a path inside `jaq '...'`
/// is an instruction, and instructions can be measured against the published
/// schemas.
#[test]
fn every_jaq_path_in_the_documentation_exists_in_a_schema() {
    let mut known = schema_keys();
    known.extend(KNOWN_FOREIGN_PATHS.iter().map(|(k, _)| (*k).to_string()));

    let mut phantoms: Vec<String> = Vec::new();
    let mut cited = 0usize;
    for doc in documentation_files() {
        let text = std::fs::read_to_string(&doc).expect("document is readable");
        for line in text.lines() {
            if !line.contains("jaq ") {
                continue;
            }
            for path in jaq_root_paths(line) {
                cited += 1;
                if !known.contains(&path) {
                    phantoms.push(format!("{}: .{path}", name_of(&doc)));
                }
            }
        }
    }

    assert!(
        cited > 20,
        "only {cited} jaq paths were found across the documentation, so this \
         ruler is measuring an empty set"
    );
    phantoms.sort();
    phantoms.dedup();
    assert!(
        phantoms.is_empty(),
        "these documented jaq paths exist in no published schema:\n  {}\n\n\
         A wrong path does not fail loudly: jaq prints null and exits 0. Fix the \
         path, or add it to KNOWN_FOREIGN_PATHS with where it really comes from.",
        phantoms.join("\n  ")
    );
}

/// Documents whose `jaq` recipes are instructions to a reader.
fn documentation_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for name in [
        "README.md",
        "README.pt-BR.md",
        "llms-full.txt",
        "llms.txt",
        "llms.pt-BR.txt",
    ] {
        out.push(repo_root().join(name));
    }
    if let Ok(entries) = std::fs::read_dir(repo_root().join("docs")) {
        out.extend(
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "md")),
        );
    }
    // Skills ship the same recipes to a different reader. Leaving them outside
    // this list is how `.synth` and `.title_original` survived here after every
    // doc was corrected: the guard had quietly chosen a narrower scope.
    out.extend(packaged_skills());
    out
}

/// Every `SKILL.md` packaged under `skills/`.
fn packaged_skills() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(repo_root().join("skills")) {
        for entry in entries.filter_map(Result::ok) {
            let candidate = entry.path().join("SKILL.md");
            if candidate.is_file() {
                out.push(candidate);
            }
        }
    }
    out.sort();
    out
}

/// The value of the `description:` line in a skill's YAML frontmatter.
fn skill_description(text: &str) -> Option<&str> {
    text.lines()
        .find_map(|line| line.strip_prefix("description: "))
}

/// A skill is a prompt package, and these caps are its published contract.
///
/// The numbers live here rather than being read back from the files, because a
/// ruler that derives its expectation from the artifact can never disagree
/// with it.
#[test]
fn every_packaged_skill_respects_its_declared_caps() {
    const MAX_WORDS: usize = 4000;
    const MAX_DESCRIPTION_CHARS: usize = 1024;

    let skills = packaged_skills();
    assert!(
        skills.len() >= 2,
        "expected the packaged skills to be discoverable under skills/, found {}",
        skills.len()
    );

    let mut violations = Vec::new();
    for path in &skills {
        let text = std::fs::read_to_string(path).expect("read SKILL.md");
        let label = path.parent().map_or_else(|| name_of(path), name_of);

        let words = text.split_whitespace().count();
        if words > MAX_WORDS {
            violations.push(format!(
                "{label}: {words} words exceeds the {MAX_WORDS} cap"
            ));
        }

        match skill_description(&text) {
            None => violations.push(format!("{label}: no `description:` in the frontmatter")),
            Some(description) => {
                let chars = description.chars().count();
                if chars > MAX_DESCRIPTION_CHARS {
                    violations.push(format!(
                        "{label}: description is {chars} chars, cap is {MAX_DESCRIPTION_CHARS}"
                    ));
                }
                if description.contains(':') {
                    violations.push(format!(
                        "{label}: description value contains a colon, which breaks the YAML scalar"
                    ));
                }
            }
        }

        let fences = text.lines().filter(|l| l.starts_with("```")).count();
        if fences > 0 {
            violations.push(format!(
                "{label}: {fences} fenced code blocks — a skill carries prompts, not code"
            ));
        }

        let deep_headings = text.lines().filter(|l| l.starts_with("#### ")).count();
        if deep_headings > 0 {
            violations.push(format!(
                "{label}: {deep_headings} headings below H3, which the contract forbids"
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "packaged skills broke their own contract:\n  {}",
        violations.join("\n  ")
    );
}

/// One document, one title.
///
/// `docs/INTEGRATIONS.md` carried three H1s: its real title, an `# ENGLISH
/// SECTION` divider, and a `# SECAO EM PORTUGUES` divider that opened a stale
/// duplicate of `docs/INTEGRATIONS.pt-BR.md`. The duplicate still advertised
/// `0.9.8+` and `timeout 30` long after the standalone file had moved on, so
/// the second copy was not merely redundant — it was wrong. A reader arriving
/// at the second H1 has no way to tell which half is current.
#[test]
fn every_documentation_file_has_exactly_one_h1() {
    // A bundle is a concatenation of documents, so more than one title is its
    // whole point. Every entry carries the reason it is not a normal document.
    const BUNDLE_EXEMPT: &[(&str, &str)] = &[(
        "llms-full.txt",
        "concatenates README, HOW_TO_USE, COOKBOOK and INTEGRATIONS behind \
         `# <path>` separators, so one H1 per embedded file is the format",
    )];

    let mut violations = Vec::new();
    for path in documentation_files() {
        if BUNDLE_EXEMPT
            .iter()
            .any(|(file, _)| name_of(&path) == *file)
        {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut in_fence = false;
        let mut titles = Vec::new();
        for (idx, line) in text.lines().enumerate() {
            if line.starts_with("```") {
                in_fence = !in_fence;
                continue;
            }
            // Inside a fence, `# foo` is a shell comment, not a heading.
            if !in_fence && line.starts_with("# ") {
                titles.push(format!("line {}: {}", idx + 1, line.trim()));
            }
        }
        if titles.len() != 1 {
            violations.push(format!(
                "{}: {} H1 headings, expected exactly 1\n      {}",
                name_of(&path),
                titles.len(),
                titles.join("\n      ")
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "a second H1 splits one document into two documents:\n  {}",
        violations.join("\n  ")
    );
}

/// File name of a path, for readable assertion messages.
fn name_of(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// Root identifiers of every `jaq '.foo…'` expression on one line.
fn jaq_root_paths(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 2 < chars.len() {
        // Only a path that OPENS a jaq program is unambiguous; `.foo` further
        // in could be a nested key, and nesting is not what this measures.
        if chars[i] == '\'' && chars[i + 1] == '.' && chars[i + 2].is_ascii_alphabetic() {
            let mut j = i + 2;
            while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            out.push(chars[i + 2..j].iter().collect());
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Every object key anywhere inside a JSON document.
fn collect_object_keys(value: &serde_json::Value, out: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                out.insert(k.clone());
                collect_object_keys(v, out);
            }
        }
        serde_json::Value::Array(items) => {
            for v in items {
                collect_object_keys(v, out);
            }
        }
        serde_json::Value::String(s) => {
            out.insert(s.clone());
        }
        _ => {}
    }
}

/// Every file under `docs/generated/` must have a declared consumer.
///
/// # The class this closes
///
/// v1.0.4 deleted `scripts/regen_cli_flags_readme.py` and left its two INPUT
/// files behind: `flag-desc-en.json` and `flag-desc-pt.json`, curated
/// descriptions for fourteen flags, read by nothing in Rust or shell, dated
/// months before the release, and still advertised as live artifacts in both
/// CHANGELOGs. Nobody noticed because nothing was looking — the drift rulers
/// guarded the inventory and the two flag tables by NAME, and a file that no
/// ruler names is a file no ruler can miss.
///
/// Naming the consumers instead of the files inverts that: adding a generated
/// artifact without saying who reads it fails the build, so the next orphan
/// cannot be created silently rather than merely cleaned up afterwards.
#[test]
fn no_generated_artifact_is_an_orphan() {
    /// Generated file, and what reads it.
    const CONSUMERS: &[(&str, &str)] = &[
        (
            "cli-flags-inventory.json",
            "read by `inventory_matches_the_binary` in this file, and regenerated \
             by the ignored `rewrite_cli_flags_inventory` writer",
        ),
        (
            "flags_en.md",
            "read by `generated_flag_tables_have_no_phantom_flags` in this file",
        ),
        (
            "flags_pt.md",
            "read by `generated_flag_tables_have_no_phantom_flags` in this file",
        ),
    ];

    let dir = repo_root().join("docs/generated");
    let mut orphans = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("docs/generated is readable") {
        let path = entry.expect("dir entry").path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("file name is UTF-8")
            .to_string();
        if !CONSUMERS.iter().any(|(f, _)| *f == name) {
            orphans.push(name);
        }
    }
    assert!(
        orphans.is_empty(),
        "generated artifacts with no declared consumer: {orphans:?}\n\n\
         A generated file whose producer is gone and whose readers are none is \
         documentation that cannot be wrong, because nothing checks it, and \
         cannot be right, because nothing regenerates it. Either wire a reader \
         and name it in CONSUMERS, or delete the file."
    );

    for (file, _) in CONSUMERS {
        assert!(
            dir.join(file).exists(),
            "CONSUMERS names {file}, which no longer exists — remove the row \
             rather than leaving a claim about a file that is gone"
        );
    }
}

/// `html_root_url` must name the version the crate actually is.
///
/// # The class this closes
///
/// The attribute takes a string LITERAL — `env!`/`concat!` are rejected there —
/// so the version has to be typed by hand, and `src/lib.rs` even carries a
/// comment telling the next person to keep it in sync. It drifted anyway: the
/// crate shipped 1.0.5 while the attribute still said 1.0.3, so every
/// cross-crate rustdoc deep link pointed at a release this code is not.
///
/// A comment asking someone to remember is not a mechanism. This is.
#[test]
fn html_root_url_names_the_current_crate_version() {
    let src = std::fs::read_to_string(repo_root().join("src/lib.rs")).expect("lib.rs is readable");
    let declared = src
        .lines()
        .find_map(|line| {
            let rest = line.split("html_root_url").nth(1)?;
            let url = rest.split('"').nth(1)?;
            url.rsplit('/').next().map(str::to_string)
        })
        .expect("src/lib.rs declares html_root_url with a quoted URL");

    assert_eq!(
        declared,
        env!("CARGO_PKG_VERSION"),
        "src/lib.rs pins html_root_url to {declared}, but the crate is {}. \
         Rustdoc deep links from other crates would resolve against the wrong \
         release. Update the literal in the `#![doc(html_root_url = ...)]` \
         attribute; it cannot be computed, which is exactly why it needs a ruler.",
        env!("CARGO_PKG_VERSION")
    );
}

/// The pinned toolchain must BE the declared MSRV, not merely near it.
///
/// # The class this closes
///
/// `rust-toolchain.toml` pins `channel` and `Cargo.toml` declares
/// `rust-version`. Nothing connects them except a comment in the toolchain file
/// asking the next person to "update in lockstep to avoid drift" — the exact
/// shape of instruction that `html_root_url` already proved insufficient.
///
/// The failure is silent and inverted: if `channel` ever drifts ABOVE
/// `rust-version`, every local gate keeps passing while the crate quietly stops
/// building on the MSRV it promises, and the first person to find out is a user
/// on an older toolchain. No gate here uses `+<version>`, so the pin is the
/// only thing making the fourteen gates an MSRV test at all.
#[test]
fn pinned_toolchain_matches_the_declared_msrv() {
    let manifest =
        std::fs::read_to_string(repo_root().join("Cargo.toml")).expect("Cargo.toml is readable");
    let msrv = manifest
        .lines()
        .find_map(|line| {
            let rest = line.trim().strip_prefix("rust-version")?;
            rest.split('"').nth(1).map(str::to_string)
        })
        .expect("Cargo.toml declares rust-version");

    let toolchain = std::fs::read_to_string(repo_root().join("rust-toolchain.toml"))
        .expect("rust-toolchain.toml is readable");
    let channel = toolchain
        .lines()
        .find_map(|line| {
            let rest = line.trim().strip_prefix("channel")?;
            rest.split('"').nth(1).map(str::to_string)
        })
        .expect("rust-toolchain.toml declares channel");

    assert_eq!(
        channel, msrv,
        "rust-toolchain.toml pins channel {channel} but Cargo.toml declares \
         rust-version {msrv}. The gates run on the pinned channel, so a mismatch \
         means the MSRV is asserted in the manifest and never compiled. Move both \
         together or the promise is untested."
    );
}

/// `--version` must name the commit AND whether the tree matched it.
///
/// # The class this closes
///
/// `build.rs` embedded `git rev-parse HEAD` and nothing else, so a binary built
/// from a modified tree reported the same twelve hex digits as a binary built
/// from the pristine commit — byte for byte. That is the normal state during
/// development, which is exactly when an audit runs, so an auditor could not
/// tell which artefact was on the PATH. `rev-parse` answers "what commit does
/// this descend from"; the question being asked is "what bytes did I compile".
#[test]
fn version_label_states_commit_and_tree_state() {
    let out = assert_cmd::Command::cargo_bin("duckduckgo-search-cli")
        .expect("compiled binary exists")
        .arg("--version")
        .output()
        .expect("version runs");
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();

    let sha = text
        .split("(git:")
        .nth(1)
        .and_then(|rest| rest.split(')').next())
        .unwrap_or_else(|| panic!("--version must carry a `(git:...)` label, got {text:?}"));

    let (digits, dirty) = match sha.strip_suffix("-dirty") {
        Some(head) => (head, true),
        None => (sha, false),
    };
    assert!(
        digits == "unknown"
            || (digits.len() == 12 && digits.chars().all(|c| c.is_ascii_hexdigit())),
        "the commit part of {sha:?} is neither `unknown` nor twelve hex digits"
    );

    // The tree state the binary claims must match the tree it was built from.
    let Some(porcelain) = git_porcelain() else {
        return; // No git on this host: nothing to cross-check against.
    };
    assert_eq!(
        dirty,
        !porcelain.is_empty(),
        "the binary reports dirty={dirty} while `git status --porcelain` is \
         {}. A stale answer here means `build.rs` did not re-run, which is what \
         removing its `cargo:rerun-if-changed` lines exists to prevent.",
        if porcelain.is_empty() {
            "empty"
        } else {
            "not empty"
        }
    );
}

/// `git status --porcelain` output, or `None` when git cannot answer.
fn git_porcelain() -> Option<String> {
    use std::process::Stdio;
    std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

/// Files whose Portuguese content IS the product, with the reason.
///
/// An entry is a claim that the Portuguese in that file is data the CLI ships,
/// not prose a maintainer left behind. The tuple requires the reason, so the
/// next entry cannot be added silently.
///
/// # Why these are repo-relative paths and not file names
///
/// This table keyed on the BASENAME until the 2026-08-10 audit. A basename is
/// not unique in a Rust tree, because sibling modules reuse names, so every
/// entry silently exempted its homonyms too. Measured: `news.rs` was written
/// for `src/extraction/news.rs` and was also exempting `src/pipeline/chrome/
/// news.rs`, which carries no lexicon at all; `language.rs` was written for
/// `tests/common/language.rs` and was also exempting `src/i18n/language.rs`.
/// Two production files sat outside the ruler and nobody had decided that.
///
/// The defect is the same one this ruler exists to catch, one level up: an
/// exception written NARROW that the mechanism applies WIDE. A repo-relative
/// path is unique by construction, so the exception can only cover what it
/// names — and [`language_exemptions_match_exactly_one_file`] proves it does.
const LANGUAGE_EXEMPT: &[(&str, &str)] = &[
    (
        "src/i18n/pt_br.rs",
        "IS the pt-BR message catalogue; every string in it is the translation",
    ),
    (
        "src/deep_research/depth.rs",
        "carries the Portuguese stop-word corpus used to decompose a pt-BR \
         query into sub-queries",
    ),
    (
        "src/extraction/news.rs",
        "carries the pt-BR relative-date lexicon (`agora`, `ontem`) the news \
         parser matches against",
    ),
    (
        "tests/common/language.rs",
        "IS the marker tables; its own word lists are data, not prose",
    ),
    (
        "tests/integration_docs_drift.rs",
        "hosts the ruler and quotes offending strings in its own failure cases",
    ),
    (
        "tests/integration_root_docs.rs",
        "hosts the markdown-prose ruler and quotes Portuguese examples",
    ),
];

/// `path` as a repo-relative key with forward slashes on every platform.
///
/// `Path::display` emits `\` on Windows, which would make every table lookup
/// miss there and quietly exempt nothing — a gate that passes for the wrong
/// reason on the one platform nobody runs it on.
fn repo_relative_key(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Whether a file is exempt, and why.
fn language_exemption(path: &Path) -> Option<&'static str> {
    let key = repo_relative_key(path);
    LANGUAGE_EXEMPT
        .iter()
        .find(|(f, _)| *f == key)
        .map(|(_, reason)| *reason)
}

/// Code, comments, assertion prose and identifiers are English.
///
/// # Why the previous version of this ruler could not have found these
///
/// It was named `code_comments_are_english`, its failure message said "code and
/// comments are English in this project", and `looks_portuguese` opened with a
/// three-line guard that returned `false` for any line not starting with `//`.
/// So it measured comments and nothing else, and it had genuinely cleared that
/// scope — zero survivors. One character outside it, in string literals, 307
/// lines were untouched.
///
/// This measures four axes. Comments, as before. String literals in PROSE
/// position, meaning arguments to an assertion or a log macro, because those
/// are addressed to a human. Rust identifiers, which are code by definition.
/// And the hybrid residue of the mechanical sweep that closed this class twice
/// before — `"must NOTria ser data relativa"` contains no Portuguese word any
/// list would hold, because the sweep replaced every word the list knew.
///
/// Fixture DATA is deliberately out of scope. This CLI searches in Brazilian
/// Portuguese; a test that feeds it English-only fixtures would be testing a
/// product nobody ships.
#[test]
fn code_and_prose_are_english() {
    let root = repo_root();
    let roots = [root.join("src"), root.join("tests"), root.join("benches")];
    let roots: Vec<&Path> = roots.iter().map(std::path::PathBuf::as_path).collect();
    let mut offenders: Vec<String> = Vec::new();

    for path in language::rust_files(&roots, &[]) {
        if language_exemption(&path).is_some() {
            continue;
        }
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .display()
            .to_string();
        let text = std::fs::read_to_string(&path).expect("readable rust file");

        for (idx, line) in text.lines().enumerate() {
            let lineno = idx + 1;
            let trimmed = line.trim_start();

            // Axis 1 — comments.
            if trimmed.starts_with("//") && language::looks_portuguese(line, language::CODE_MARKERS)
            {
                offenders.push(format!("{rel}:{lineno}: [comment] {}", line.trim()));
                continue;
            }

            // Axis 3 — identifiers.
            if let Some(name) = language::portuguese_identifier(line) {
                offenders.push(format!("{rel}:{lineno}: [identifier] {name}"));
            }
        }

        // Axis 2 and 4 — prose literals and translation residue.
        for lit in language::prose_literals(&text) {
            if let Some(fragment) = language::corrupted_fragment(&lit.text) {
                offenders.push(format!(
                    "{rel}:{}: [half-translated: {fragment:?}] {:?}",
                    lit.line, lit.text
                ));
                continue;
            }
            if language::looks_portuguese(&lit.text, language::CODE_MARKERS)
                || language::has_portuguese_diacritic(&lit.text)
            {
                offenders.push(format!("{rel}:{}: [prose] {:?}", lit.line, lit.text));
            }
        }
    }

    offenders.sort();
    offenders.dedup();
    assert!(
        offenders.is_empty(),
        "this project writes code, comments, assertion messages and identifiers \
         in English; Portuguese belongs in `gaps.md`, the `*.pt-BR.md` documents \
         and the pt-BR message catalogue.\n\
         Fixture DATA is out of scope on purpose — only strings addressed to a \
         human are listed here.\n\n{}\n\n({} offender(s))",
        offenders.join("\n"),
        offenders.len()
    );
}

/// All four detectors must fire, or the ruler above is decoration.
#[test]
fn portuguese_detector_is_not_vacuous() {
    // Axis 1 — comments.
    assert!(language::looks_portuguese(
        "    // Garante que o caminho Chrome reutilize o esqueleto",
        language::CODE_MARKERS
    ));
    assert!(!language::looks_portuguese(
        "    // Returns the aggregated results for this query",
        language::CODE_MARKERS
    ));

    // Axis 2 — prose literals, and only in the MESSAGE position.
    let asserted = language::prose_literals("assert!(x, \"deve retornar tres\");\n");
    assert_eq!(
        asserted.len(),
        1,
        "an assertion message is prose and must be seen"
    );
    let fixture = language::prose_literals("let title = \"Prefeitura confirma obras\";\n");
    assert!(
        fixture.is_empty(),
        "a fixture is data; this CLI searches in Portuguese and must be free to"
    );
    // The compared VALUES of an equality assertion are data, not prose. This is
    // the case that made fifty-six pt-BR fixtures look like leftovers.
    let compared = language::prose_literals("assert_eq!(parse(\"há 2 horas\"), Some(d));\n");
    assert!(
        compared.is_empty(),
        "the inputs of assert_eq! are the data under test; this CLI parses \
         Portuguese relative dates, so those literals ARE the product"
    );
    let with_message = language::prose_literals(
        "assert_eq!(parse(\"há 2 horas\"), Some(d), \"deve virar duracao\");\n",
    );
    assert_eq!(
        with_message.len(),
        1,
        "the third argument IS the message and must still be seen"
    );

    // Axis 3 — identifiers, with the serde escape hatch intact.
    assert_eq!(
        language::portuguese_identifier("    let conteudo = read(path);"),
        Some("conteudo".to_string())
    );
    assert_eq!(
        language::portuguese_identifier("    let content = read(p);"),
        None
    );
    assert_eq!(
        language::portuguese_identifier("    #[serde(rename = \"quantidade_resultados\")]"),
        None,
        "the pt wire key is contract, not a Portuguese identifier"
    );

    // Axis 4 — the residue no word list can hold.
    assert_eq!(
        language::corrupted_fragment("{s:?} must NOTria ser data relativa"),
        Some("notria")
    );
    assert_eq!(
        language::corrupted_fragment("must not be a relative date"),
        None
    );
}

/// Every language exemption must carry a reason a reader can weigh.
#[test]
fn language_exemptions_are_documented() {
    for (file, reason) in LANGUAGE_EXEMPT {
        assert!(
            reason.len() > 30,
            "{file} carries the reason {reason:?}, too short to tell the next \
             reader why its Portuguese is data rather than prose"
        );
    }
}

/// Each exemption must cover exactly the one file it names.
///
/// # Why an exemption table needs its own ruler
///
/// An exemption is the only construct in a gate that can make the gate quieter,
/// so it is the only construct whose blast radius nobody measures. Keying on
/// the basename made every entry cover its homonyms, and the extra coverage was
/// invisible precisely because it produced no failure — the ruler simply looked
/// at fewer files and stayed green.
///
/// Two assertions, because the entry can go wrong in two directions. Zero
/// matches means the file moved or was renamed and the exemption is now dead
/// weight excusing nothing. More than one match means the key is not unique and
/// the exemption reaches past its stated target.
#[test]
fn language_exemptions_match_exactly_one_file() {
    let root = repo_root();
    let roots = [root.join("src"), root.join("tests"), root.join("benches")];
    let roots: Vec<&Path> = roots.iter().map(std::path::PathBuf::as_path).collect();
    let scanned: Vec<String> = language::rust_files(&roots, &[])
        .iter()
        .map(|p| repo_relative_key(p))
        .collect();

    for (file, _) in LANGUAGE_EXEMPT {
        let hits = scanned.iter().filter(|k| k.as_str() == *file).count();
        assert_eq!(
            hits, 1,
            "the exemption {file:?} matches {hits} scanned files, not 1.\n\n\
             Zero means the file moved and the entry now excuses nothing, so \
             delete it. More than one means the key is ambiguous and the entry \
             is silently exempting files nobody weighed — the exact defect that \
             let `src/pipeline/chrome/news.rs` sit outside this ruler."
        );
    }
}

/// Writer for the snapshot above. Ignored by default; see the module docs.
#[test]
#[ignore = "writes docs/generated/cli-flags-inventory.json; run explicitly"]
fn regenerate_flag_inventory() {
    let live = live_root_flags();
    let mut inventory = read_inventory();
    let obj = inventory.as_object_mut().expect("inventory is an object");

    obj.insert(
        "binary".into(),
        serde_json::Value::String(
            String::from_utf8_lossy(
                &assert_cmd::Command::cargo_bin("duckduckgo-search-cli")
                    .expect("compiled binary exists")
                    .arg("--version")
                    .output()
                    .expect("version runs")
                    .stdout,
            )
            .trim()
            .to_string(),
        ),
    );
    obj.insert("version".into(), serde_json::Value::String(live_version()));
    obj.insert("root_count".into(), serde_json::Value::from(live.len()));
    obj.insert(
        "root_longs".into(),
        serde_json::Value::Array(
            live.iter()
                .map(|f| serde_json::Value::String(f.clone()))
                .collect(),
        ),
    );
    for (key, sub) in [
        ("deep_only", "deep-research"),
        ("doctor_only", "doctor"),
        ("init_only", "init-config"),
        ("schema_only", "schema"),
        ("man_only", "man"),
    ] {
        let exclusive: Vec<serde_json::Value> = declared_flags(&help_text(&[sub]))
            .into_iter()
            .filter(|f| !live.contains(f))
            .map(serde_json::Value::String)
            .collect();
        obj.insert(key.into(), serde_json::Value::Array(exclusive));
    }

    write_pretty(&inventory_path(), &inventory);
}

/// Write JSON with a trailing newline, matching the repository's other files.
fn write_pretty(path: &Path, value: &serde_json::Value) {
    let mut text = serde_json::to_string_pretty(value).expect("inventory serializes");
    text.push('\n');
    std::fs::write(path, text).expect("inventory is writable");
}

/// Every exit code the binary can return must appear in every exit-code table.
///
/// # The class this closes
///
/// `llms-full.txt` carried FOUR exit-code tables that disagreed with each other:
/// one correct, one missing `130` and `143`, one missing `6` and `141`, and a
/// Portuguese one that stopped at `5`. `llms.txt` and `llms.pt-BR.txt` each
/// omitted `130` and `143` while the same files tell the reader to wrap every
/// call in `timeout`, which sends SIGTERM first and therefore produces `143`.
///
/// An agent that treats an undocumented code as "unknown failure" retries a
/// cancellation it asked for. The tables were prose, so nothing compared them
/// to `src/error/exit_codes.rs` — the only place the answer actually lives.
#[test]
fn every_exit_code_appears_in_every_exit_code_table() {
    // SSOT: parsed from the constants rather than restated here, so a new code
    // is undocumented-by-default instead of silently exempt.
    let source = std::fs::read_to_string(repo_root().join("src/error/exit_codes.rs"))
        .expect("exit_codes.rs is readable");
    let codes: BTreeSet<i32> = source
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub const "))
        .filter_map(|l| l.split(": i32 = ").nth(1))
        .filter_map(|l| l.trim_end_matches(';').trim().parse::<i32>().ok())
        .collect();
    assert!(
        codes.len() >= 8,
        "expected the exit-code SSOT to be non-trivial, parsed {codes:?}"
    );

    for doc in ["llms.txt", "llms.pt-BR.txt", "README.md", "README.pt-BR.md"] {
        let text = std::fs::read_to_string(repo_root().join(doc))
            .unwrap_or_else(|e| panic!("{doc} is readable: {e}"));
        // A table row states the code between pipes; matching that shape avoids
        // counting a `--num 5` elsewhere in the prose as documentation of exit 5.
        let missing: Vec<i32> = codes
            .iter()
            .copied()
            .filter(|c| !text.contains(&format!("| `{c}` |")) && !text.contains(&format!("| {c} ")))
            .collect();
        assert!(
            missing.is_empty(),
            "{doc} has an exit-code table that omits {missing:?}\n\n\
             Every code in src/error/exit_codes.rs must appear as a table row. \
             An agent that meets an undocumented code cannot tell a cancellation \
             it requested from a failure it should retry."
        );
    }
}
