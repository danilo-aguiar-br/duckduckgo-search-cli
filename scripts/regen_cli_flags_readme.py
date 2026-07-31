#!/usr/bin/env python3
"""Regenerate README flag tables from live CLI --help so EN/PT never diverge.

Usage (from repo root, with duckduckgo-search-cli 1.0.2 on PATH):
  python3 scripts/regen_cli_flags_readme.py
  # writes /tmp/README*.new + docs/generated/* then apply with atomwrite:
  atomwrite --workspace . write README.md < /tmp/README.md.new
  atomwrite --workspace . write README.pt-BR.md < /tmp/README.pt-BR.md.new
  atomwrite --workspace . write docs/generated/cli-flags-inventory.json < /tmp/cli-flags-inventory.json
  atomwrite --workspace . write docs/generated/flags_en.md < /tmp/flags_en.md
  atomwrite --workspace . write docs/generated/flags_pt.md < /tmp/flags_pt.md

Product version SSOT: Cargo.toml / binary --version (must be 1.0.2 for release docs).
"""
from __future__ import annotations
import json, re, subprocess, sys
from pathlib import Path

ANSI = re.compile(r"\x1b\[[0-9;]*m")
SKIP = {"help", "version"}
KNOWN_DEFAULT = {
    "num": "15", "lang": "pt", "country": "br", "endpoint": "html", "vertical": "all",
    "safe-search": "moderate", "identity-profile": "auto", "format": "auto",
    "timeout": "15", "parallel": "5", "pages": "1", "retries": "2",
    "global-timeout": "180", "cancel-grace-secs": "5", "wire-keys": "en",
    "fetch-content-cap": "4", "max-content-length": "10000", "per-host-limit": "2",
    "chrome-session-retries": "2", "max-sub-queries": "3",
}
NONE_FLAGS = {
    "output","queries-file","time-filter","seed","config","fields","select","filter","limit",
    "sort","dedupe-by","truncate-content","max-output-bytes","proxy","chrome-path","cookies-path",
    "config-home","ui-lang","base-url-html","base-url-lite","base-url-serp","dump-news-html",
    "name","file","sub-queries-file","budget-tokens","synth-format","depth","aggregate",
    "sub-query-strategy",
}

def run_help(args: list[str]) -> str:
    r = subprocess.run(["duckduckgo-search-cli", *args, "--help"], capture_output=True, text=True)
    out = r.stdout if r.stdout.strip() else r.stderr
    return ANSI.sub("", out)

def parse_flags(help_text: str) -> list[dict]:
    flags, lines, i, section = [], help_text.splitlines(), 0, ""
    while i < len(lines):
        line = lines[i]
        if re.match(r"^[A-Za-z][A-Za-z0-9 /&_+-]*:\s*$", line) and not line.startswith(" "):
            section = line.rstrip(":").strip(); i += 1; continue
        m = re.match(r"^ {2,}(?:-([a-zA-Z]), )?--([a-z0-9-]+)(\.\.\.| <[^>]+>)?\s*$", line)
        if not m:
            i += 1; continue
        short, long, arg = m.group(1), m.group(2), (m.group(3) or "").strip()
        desc_lines, default = [], None
        i += 1
        while i < len(lines):
            l = lines[i]
            if re.match(r"^ {2,}(?:-([a-zA-Z]), )?--([a-z0-9-]+)", l): break
            if re.match(r"^[A-Za-z][A-Za-z0-9 /&_+-]*:\s*$", l) and not l.startswith(" "): break
            if re.match(r"^[A-Z][A-Z0-9 /&_+-]+:\s*$", l): break
            s = l.strip()
            if s.startswith("[default: ") and s.endswith("]"):
                default = s[len("[default: "):-1]
            elif "Possible values:" in l:
                i += 1
                while i < len(lines) and (lines[i].startswith(" ") or lines[i].strip() == ""):
                    s2 = lines[i].strip()
                    if s2.startswith("[default: ") and s2.endswith("]"):
                        default = s2[len("[default: "):-1]
                    if re.match(r"^ {2,}(?:-([a-zA-Z]), )?--([a-z0-9-]+)", lines[i]): break
                    if re.match(r"^[A-Za-z].*:\s*$", lines[i]) and not lines[i].startswith(" "): break
                    i += 1
                continue
            elif re.match(r"^ {8,}", l):
                t = s
                if t and not t.startswith("[") and not t.startswith("Possible") and not re.match(r"^- [a-z0-9_.-]+:", t):
                    desc_lines.append(t)
            i += 1
        desc = re.sub(r"\s+", " ", " ".join(desc_lines)).strip()
        first = re.split(r"(?<=[.])\s+", desc)[0] if desc else ""
        if len(first) > 180: first = first[:177] + "…"
        flag_disp = f"`-{short}`, `--{long}`" if short else f"`--{long}`"
        flags.append({"long": long, "flag_disp": flag_disp, "default": default, "desc": first or desc[:180], "desc_full": desc})
    return flags

# Keep curated descriptions in sibling JSON if present; else use help first sentence.
ROOT = Path(__file__).resolve().parents[1] if Path(__file__).name.endswith(".py") else Path(".")
# When run as /tmp script, detect repo
if not (ROOT / "Cargo.toml").exists():
    ROOT = Path.cwd()

def load_overrides(lang: str) -> dict:
    p = ROOT / "docs" / "generated" / f"flag-desc-{lang}.json"
    if p.exists():
        return json.loads(p.read_text(encoding="utf-8"))
    return {}

def default_disp(f, lang="en"):
    long = f["long"]
    d = f["default"] if f["default"] is not None else KNOWN_DEFAULT.get(long)
    if long == "vertical" and d == "all": return "**`all`** (v0.9.8)"
    if long == "wire-keys" and d == "en": return "**`en`** (v1.0.2)"
    if long == "fetch-content-cap" and str(d) == "4": return "**`4`** (v1.0.2)"
    if long == "max-sub-queries" and str(d) == "3": return "**`3`** (v1.0.2)"
    if long == "fetch-content":
        return "**on** (v0.9.8)" if lang == "en" else "**ligado** (v0.9.8)"
    if d is not None: return f"`{d}`"
    if long in NONE_FLAGS: return "(none)" if lang == "en" else "(nenhum)"
    return "off"

def desc(f, lang, ov):
    if f["long"] in ov: return ov[f["long"]]
    return f["desc"] or f["desc_full"] or ("(see `--help`)" if lang == "en" else "(veja `--help`)")

def md_table(flags, lang, ov):
    h = ("| Flag | Default | Description |\n| ---- | ------- | ----------- |" if lang == "en"
         else "| Flag | Padrão | Descrição |\n| ---- | ------ | --------- |")
    rows = [h]
    for f in flags:
        rows.append(f"| {f['flag_disp']} | {default_disp(f, lang)} | {desc(f, lang, ov)} |")
    return "\n".join(rows)

def main() -> int:
    ver = subprocess.check_output(["duckduckgo-search-cli", "--version"], text=True).strip()
    root = [f for f in parse_flags(run_help([])) if f["long"] not in SKIP]
    deep = [f for f in parse_flags(run_help(["deep-research"])) if f["long"] not in SKIP]
    doctor = [f for f in parse_flags(run_help(["doctor"])) if f["long"] not in SKIP]
    init = [f for f in parse_flags(run_help(["init-config"])) if f["long"] not in SKIP]
    schema = [f for f in parse_flags(run_help(["schema"])) if f["long"] not in SKIP]
    man = [f for f in parse_flags(run_help(["man"])) if f["long"] not in SKIP]
    rl = {f["long"] for f in root}
    deep_only = [f for f in deep if f["long"] not in rl]
    doctor_only = [f for f in doctor if f["long"] not in rl]
    init_only = [f for f in init if f["long"] not in rl]
    schema_only = [f for f in schema if f["long"] not in rl]
    man_only = [f for f in man if f["long"] not in rl]
    en_ov, pt_ov = load_overrides("en"), load_overrides("pt")

    def build(lang, ov):
        n = len(root)
        if lang == "en":
            note = (f"> **SSOT:** generated from `duckduckgo-search-cli --help` and subcommand `--help` "
                    f"on binary **v1.0.2** ({n} root flags + deep/doctor/init/schema/man exclusives). "
                    f"Prefer `commands` / `schema` for low-token agent discovery. "
                    f"Portuguese prose lives **only** in [`README.pt-BR.md`](README.pt-BR.md) — this file is English-only.\n")
            parts = ["### Flags\n\n", note, "\n#### Root / default search (complete inventory from `--help`)\n\n", md_table(root, "en", ov), "\n"]
            if deep_only: parts += ["\n#### `deep-research` only (in addition to global root flags)\n\n", md_table(deep_only, "en", ov), "\n"]
            if doctor_only: parts += ["\n#### `doctor` only\n\n", md_table(doctor_only, "en", ov), "\n"]
            if init_only: parts += ["\n#### `init-config` only\n\n", md_table(init_only, "en", ov), "\n"]
            if schema_only: parts += ["\n#### `schema` only\n\n", md_table(schema_only, "en", ov), "\n"]
            if man_only: parts += ["\n#### `man` only\n\n", md_table(man_only, "en", ov), "\n"]
            return "".join(parts)
        note = (f"> **SSOT:** gerado a partir de `duckduckgo-search-cli --help` e `--help` dos subcomandos "
                f"no binário **v1.0.2** ({n} flags da raiz + exclusivas de deep/doctor/init/schema/man). "
                f"Prefira `commands` / `schema` para descoberta de agente com baixo custo de tokens. "
                f"O inglês mora **somente** em [`README.md`](README.md) — este arquivo é o SSOT em português.\n")
        parts = ["### Flags\n\n", note, "\n#### Raiz / busca padrão (inventário completo de `--help`)\n\n", md_table(root, "pt", ov), "\n"]
        if deep_only: parts += ["\n#### Só `deep-research` (além das flags globais da raiz)\n\n", md_table(deep_only, "pt", ov), "\n"]
        if doctor_only: parts += ["\n#### Só `doctor`\n\n", md_table(doctor_only, "pt", ov), "\n"]
        if init_only: parts += ["\n#### Só `init-config`\n\n", md_table(init_only, "pt", ov), "\n"]
        if schema_only: parts += ["\n#### Só `schema`\n\n", md_table(schema_only, "pt", ov), "\n"]
        if man_only: parts += ["\n#### Só `man`\n\n", md_table(man_only, "pt", ov), "\n"]
        return "".join(parts)

    en_block, pt_block = build("en", en_ov), build("pt", pt_ov)
    Path("/tmp/flags_en.md").write_text(en_block, encoding="utf-8")
    Path("/tmp/flags_pt.md").write_text(pt_block, encoding="utf-8")

    # Patch README.md flags + strip PT monolith if any
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    m = re.search(r"^### Flags\n", readme, re.M)
    if not m:
        print("ERROR: ### Flags not found in README.md", file=sys.stderr); return 1
    start = m.start()
    m2 = re.search(r"^## News Vertical", readme[start:], re.M)
    if not m2:
        print("ERROR: ## News Vertical anchor missing", file=sys.stderr); return 1
    readme = readme[:start] + en_block.rstrip() + "\n\n" + readme[start + m2.start():]
    m3 = re.search(r"\n## Português\n", readme)
    if m3:
        head = readme[:m3.start()].rstrip()
        if "License:" not in head[-200:]:
            head += "\n\nLicense: MIT OR Apache-2.0."
        head += ("\n\n---\n\n**Português:** Brazilian Portuguese documentation lives **only** in "
                 "[`README.pt-BR.md`](README.pt-BR.md) (PT SSOT — no bilingual monolith embedded in this English README).\n")
        readme = head
    Path("/tmp/README.md.new").write_text(readme, encoding="utf-8")

    pt_readme = (ROOT / "README.pt-BR.md").read_text(encoding="utf-8")
    m = re.search(r"^### Flags\n", pt_readme, re.M) or re.search(r"^## Flags Disponíveis\n", pt_readme, re.M)
    if not m:
        print("ERROR: Flags section not found in README.pt-BR.md", file=sys.stderr); return 1
    start = m.start()
    rest = pt_readme[start:]
    end = None
    for pat in [r"\n## Vertical de Notícias", r"\n## News Vertical", r"\n## Schema", r"\n## Formatos",
                r"\n## Variáveis", r"\n### Variáveis", r"\n### Formatos", r"\n### Códigos", r"\n## Schema JSON"]:
        mm = re.search(pat, rest)
        if mm and (end is None or mm.start() < end):
            end = mm.start()
    if end is None:
        mm = re.search(r"\n## [A-ZÀ-Ú]", rest[40:])
        if not mm:
            print("ERROR: cannot find end of PT flags", file=sys.stderr); return 1
        end = mm.start() + 40
    block = pt_block
    if pt_readme[start:].startswith("## Flags"):
        block = pt_block.replace("### Flags\n", "## Flags Disponíveis\n", 1)
    pt_readme = pt_readme[:start] + block.rstrip() + "\n" + pt_readme[start + end:]
    Path("/tmp/README.pt-BR.md.new").write_text(pt_readme, encoding="utf-8")

    inv = {
        "version": "1.0.2",
        "binary": ver,
        "root_count": len(root),
        "root_longs": [f["long"] for f in root],
        "deep_only": [f["long"] for f in deep_only],
        "doctor_only": [f["long"] for f in doctor_only],
        "init_only": [f["long"] for f in init_only],
        "schema_only": [f["long"] for f in schema_only],
        "man_only": [f["long"] for f in man_only],
    }
    Path("/tmp/cli-flags-inventory.json").write_text(json.dumps(inv, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"OK binary={ver} root={len(root)} deep_only={len(deep_only)} total_unique={len(root)+len(deep_only)+len(doctor_only)+len(init_only)+len(schema_only)+len(man_only)}")
    print("Wrote /tmp/README.md.new /tmp/README.pt-BR.md.new /tmp/flags_{en,pt}.md /tmp/cli-flags-inventory.json")
    print("Apply with atomwrite write (see docstring).")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
