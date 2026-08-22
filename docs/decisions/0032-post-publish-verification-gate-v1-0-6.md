# ADR-0032 — The release gate ends at the registry, not at the package (v1.0.6)

- Status: Accepted (2026-08-21)
- Related: ADR-0028 (local cross-platform gate), ADR-0029 (optional HTTP stack, no C toolchain), GAP-REL-001, GAP-REL-002
- Decisor: lead
- Context: a macOS user ran `cargo install duckduckgo-search-cli` and got `E0432`, eleven days after the fix for that exact error had been published


## Context

Every gate in `NO_CI.md` answers the same question in a different way: *does the
tree compile?* `cargo publish --dry-run --locked` extends it by one step: *is
the package well-formed?* The chain then stops.

The question a user actually experiences is neither of those. It is: **does the
version the registry serves compile?** Nothing in the project answered it.

### What that gap produced
- v1.0.1 (2026-07-20) shipped an ungated `use` of a `#[cfg(target_os = "linux")]`
  item. `cargo install` failed with `E0432` on macOS and Windows.
- v1.0.2 (2026-07-31) shipped the same class in the same module, after the
  `browser.rs` split moved the use site across a module boundary.
- The fix landed and ADR-0028 added three local cross-target gates, which do
  catch the class. That part worked.
- v1.0.5 was published on 2026-08-10 — and then yanked.

Yank is not a no-op. `max_stable_version` is *derived* from the yank state of
every version, so retiring the fixed release silently promoted the broken
v1.0.2 back to being the default. Measured on 2026-08-21: `max_stable_version`
was `1.0.2` with `yanked: false`, while `1.0.5` carried `yanked: true`.

A registry mutation reordered version resolution, reintroduced a defect the
project had already fixed, and produced no signal anywhere — because no gate
runs after `cargo publish`, and none runs after `cargo yank` at all.

### The precedent that was ignored

The `[0.9.6]` entry of `CHANGELOG.md` records the same class in a release that
did not compile
on Windows MSVC, and concludes with `Yank optional; source fix is this patch`.
Treating the removal of a broken artifact as optional is what let the artifact
keep shipping. This is the second occurrence, not the first.


## Decision

Add `src/bin/verify_published.rs`, a maintainer-only binary that queries the
crates.io API and fails when the registry does not serve this crate's version,
or when a version measured to be broken is still installable.

It runs after every `cargo publish` and after every `cargo yank`. The second
half is the one that was missing.

### It is a binary, not a subcommand

Release tooling is not a product capability. A `verify-published` verb on the
search CLI would inflate the agent-facing surface with something no user of the
tool can act on. `src/bin/gen_man.rs` already establishes the auxiliary-binary
pattern in this repository, so this follows it.

`required-features = ["release-gate"]` keeps it out of the default build
entirely.

### It adds no dependency

`crates_io_api` 0.12.0 is the obvious choice and was rejected after measurement.
It pulls `reqwest` and a TLS stack as non-optional dependencies, and
`Cargo.toml` documents that `reqwest` is the sole entry point of `aws-lc-sys`
(C) into this graph. ADR-0029 made the HTTP stack optional precisely so the
default `chrome` profile stays pure Rust and cross-compiles without a C
toolchain — which is what makes `cargo check-windows` and `cargo check-macos`
work at all, and what `tests/integration_toolchain_boundary.rs` measures.

Adding a crates.io client to verify portability would have broken portability.

The `release-gate` feature therefore reuses the same optional `reqwest` and
`rustls` the test harness already carries. No new crate, no change to
`deny.toml`, no movement in the toolchain boundary.

### It must identify itself

Measured 2026-08-21 against the live API:

- no User-Agent → HTTP 403
- `curl/8.0` → HTTP 403
- a User-Agent naming the tool and a contact → HTTP 200

A gate sending a generic User-Agent would read its own 403 as "crate not found"
and report a false all-clear. The binary sends an identifying User-Agent and
treats 403 and 404 as distinct outcomes, with 403 mapping to
`RATE_LIMITED_OR_BLOCKED` (exit 3) rather than to any kind of success.

### Broken versions are named with evidence, not inferred

"Something older is still live" is true of every crate on the registry and is
therefore noise. The binary carries `KNOWN_BROKEN_VERSIONS`, where each entry
records the version and the measurement that condemned it — for v1.0.1 and
v1.0.2, the file, line and violation count reported by
`scripts/portability-lint.sh` run against their published source.

An entry leaves that list when the version is yanked, never because it looks
old. A gate that fires on noise gets switched off, and a gate that is switched
off is the state this ADR exists to prevent.


## Consequences
- `NO_CI.md` gains `verify_published` as the gate that runs *after* publish and
  after every yank, and states that yank is a registry mutation requiring
  verification. The `Yank optional` wording is replaced by an explicit rule.
- Not having remote CI does not remove the need to verify the published
  artifact. ADR-0028 reached the same conclusion for cross-platform checking;
  this extends it one step further out, to the registry itself.
- The gate needs network access and is therefore a maintainer step, never a
  precondition for building or testing the crate.
- It reports rather than mutates. `cargo publish` and `cargo yank` stay manual
  and deliberate, because both are irreversible.


## What this ADR does not claim
- It does not verify that a published version *runs*, only that the registry
  serves the expected version and that no version measured broken is live.
- It does not download and re-lint published source. That would need a `tar`
  dependency; `scripts/portability-lint.sh` already covers extracted source and
  is what produced the evidence in `KNOWN_BROKEN_VERSIONS`.
- No binary of this project has ever been *executed* on macOS or Windows. The
  gates prove compilation. That limit is stated in `NO_CI.md` and is unchanged.
