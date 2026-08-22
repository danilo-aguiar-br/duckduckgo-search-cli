# ADR-0033 — Every published document is under an executable ruler (v1.0.6)

- Status: Accepted (2026-08-21)
- Related: ADR-0032 (the release gate ends at the registry), GAP-DOC-001, GAP-REL-001
- Decisor: lead
- Context: eleven documents announced v1.0.5 as the current line while the tree was at v1.0.6, and the list of liars was almost exactly the list of files no test opens


## Context

`tests/integration_docs_drift.rs` reads 22 documents. The repository publishes
44, plus 34 ADRs and two skills. The 22 documents outside that list were not
lightly covered. They were not covered at all.

Those counts are measured, not remembered. `fd -e md -e txt . -d 1` returns 21
at the root and `fd -e md -e txt . docs/ -E decisions` returns 23. An earlier
pass wrote "43" and "63 tests" without recording how it counted, and both
numbers propagated into two other documents before anyone re-measured them.

Measured on 2026-08-21, every one of the following claimed a version the tree
did not have:

- `SECURITY.md` promised support for a release that was no longer the latest
- `docs/CROSS_PLATFORM.md` said v1.0.3 while its Portuguese twin said v1.0.2
- `docs/INSTALL-WINDOWS.md` told Windows users to install, without mentioning
  that the version crates.io served did not compile for them
- `docs/generated/flags_en.md` declared itself generated from binary v1.0.5

That last one is the sharpest case. It calls itself an SSOT generated from
`--help`, so a reader trusts it more than prose — and no generator rewrites it.
The label is maintained by hand while wearing the costume of automation.

### This is the same defect class as GAP-REL-001

There, a configuration existed that no gate compiled. Here, a document exists
that no test reads. In both cases every check was green while the user received
something broken.

A file under a ruler cannot rot, because the build breaks when it lies. A file
outside one rots in silence until a human happens to read it. Coverage is not a
quality of the document; it is a property of the test suite.


## Decision

Every document the project publishes is subject to at least one executable
ruler. A document that no test reads is treated as a defect in the test suite,
not as a document awaiting review.

The ruler that enforces the version label is
`tests/integration_docs_version_ruler.rs`. It walks the tree rather than
consuming a list, because a list cannot report what is missing from itself.
That is the whole point: enumeration is how the previous gap survived.

### What the ruler deliberately does not do

It does not police historical references. `since v0.9.8`, `introduced in
v1.0.2` and a `## [1.0.4]` changelog heading are correct and stay frozen.
Rewriting them would destroy the only value those lines carry. Only a phrase
that CLAIMS to state the current version is measured.

`CHANGELOG`, `docs/MIGRATION` and `gaps.md` are ledgers of the past by design
and are excluded by name. The exclusion list is itself asserted: the ruler
fails if it names a file that does not exist, so a stale exemption cannot hide.

### Markers are the weak point, and the file says so

The ruler recognises a claim by matching a phrase. That list is only as good as
the wordings someone thought to write down, and it has already been wrong twice
in one day:

- `CONTRIBUTING.md:95` read `still current in v1.0.5`, buried mid-sentence. The
  older guard in `integration_root_docs.rs` matches line OPENERS only, so it
  never saw it, and neither did the first draft of this one.
- `docs/generated/flags_en.md:3` read `generated from ... on binary v1.0.5`,
  which matched no marker at all.

Both were closed by adding the phrasing and recording the miss in the file. The
comments are not decoration. They are the record of how the ruler failed, kept
next to the code so the next person does not repeat it.

### A marker that is too broad is worse than one that is too narrow

The first attempt at covering the generated files used `on binary`. It
immediately failed `BENCHMARKS.pt-BR.md:66`, which reads `Medido no binário
v0.7.10 ... a tabela NÃO foi re-medida na v1.0.6` — an honest disclosure that a
measurement is stale.

A ruler that punishes transparency teaches people to delete the disclosure. The
marker was narrowed to `generated from` / `gerado a partir de`, which measures
the ACT of generating rather than the word `binary`.


## Consequences

- Adding a document to the repository adds it to the ruler automatically,
  because the ruler walks the tree
- Exempting a document requires naming it in `HISTORICAL_BY_DESIGN`, which is a
  visible, reviewable act rather than an omission
- A version bump now breaks the build until every current-version claim is
  updated, which is the intended cost
- The bilingual cross-check means a translated pair cannot drift apart
  silently, because a pair can be internally consistent and still contradict
  its own translation
- Nominal coverage is still incomplete: eight documents under `docs/` are read
  by no ruler by name, including both halves of `CROSS_PLATFORM` and
  `INSTALL-WINDOWS`. Tree-walking covers them for the version label only
- Closing that remaining nominal gap is deliberately left open, and is recorded
  here so it is not mistaken for finished work


## Alternatives rejected

- Extending the list in `integration_docs_drift.rs` to 43 entries. Rejected
  because it repeats the mechanism that failed: a list does not report what is
  absent from it.
- A pre-commit hook. Rejected because the project has no CI, so the only gate
  that always runs is the test suite itself.
- Deleting `integration_root_docs.rs` as a duplicate. Rejected because the two
  rulers measure different failures: one is stricter about shape, the other
  about reach. The relationship is documented in both files so neither is
  removed by someone who reads only one.
