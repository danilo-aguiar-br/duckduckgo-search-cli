# ADR-0027 — Wire JSON English default (ships in product `v1.0.2`)

Status: Accepted  
Date: 2026-07-30  
Product version: `1.0.2` (filename: `0027-wire-en-default-v1-0-2.md`; legacy path `…v2-0-0.md` is a redirect stub)  
Supersedes default of: ADR-0023 (PT serialize on pre-wire-EN 1.x)  
Related: GAP-E2E-V14-JSON-WIRE-PT-KEYS, agent-native CLI rules  

## Context

Pre-ADR-0027 agent stdout used PORTUGUESE keys (`resultados`, `titulo`, `metadados`, …)
with English `serde` aliases for deserialize only (ADR-0023). Product rules require
English code and agent-native contracts; EN keys reduce LLM token friction.

## Decision
1. Wire EN default ships in product version 1.0.2 (do NOT brand product as 2.0.0; draft branch labels are not the product version).
2. Serialize default = English (`results`, `title`, `metadata`, …).
3. DESERIALIZE continues to accept Portuguese aliases so legacy fixtures and
   PT skills can parse historical JSON.
4. FieldSet / filter still accept BOTH PT and EN tokens at CLI parse time;
   projected JSON emits EN keys.
5. Implemented (V30): `--wire-keys en|pt` and XDG `wire_keys` remap EN→PT
   at the emit boundary. Default remains English. Domain structs always serialize EN.

## Consequences
- Breaking: every skill/schema/jq using PT keys must migrate.
- Positive: agent-native EN contract; aligns code field names with wire.
- Mitigation: PT deserialize aliases; MIGRATION.md + CHANGELOG BREAKING.

## Verification
- `cargo test --lib` wire serialize tests assert EN keys.
- Live SERP: stdout contains `"results"` not `"resultados"`.
- `cargo test --test e2e_cli` green.
