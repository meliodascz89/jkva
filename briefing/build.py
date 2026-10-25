#!/usr/bin/env python3
"""Build the v0.1 briefing from the question data.

Inputs:
  briefing/q/*.json          question sections (source of truth)
  briefing/00-preamble.cs.md prose preamble (Czech)

Outputs:
  briefing/BRIEFING-0.1.cs.md      Markdown briefing (Czech), GitHub friendly
  briefing/questionnaire.html      self-contained interactive questionnaire
  briefing/ANSWERS-template.md     plain answer sheet

Run:  python3 briefing/build.py
"""
from __future__ import annotations

import json
import re
import sys
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parent
QDIR = ROOT / "q"
PREAMBLE = ROOT / "00-preamble.cs.md"

# Questions that matter most if the reader only has ten minutes.
QUICK_PATH = ["Q01", "Q04", "Q07", "Q08", "Q12", "Q17", "Q22", "Q39", "Q62", "Q67", "Q84", "Q90", "Q95"]


def load_answers() -> dict[str, list[str]]:
    """Parse ANSWERS.md (Qnn = A,B) if present."""
    import re as _re
    path = ROOT / "ANSWERS.md"
    answers: dict[str, list[str]] = {}
    if not path.exists():
        return answers
    for line in path.read_text(encoding="utf-8").splitlines():
        m = _re.match(r"^(Q\d+[a-z]?)\s*=\s*(.*)$", line.strip())
        if not m:
            continue
        qid, raw = m.group(1), m.group(2).strip()
        if raw:
            answers[qid] = [x.strip() for x in raw.split(",") if x.strip()]
    return answers


def load_sections() -> list[dict]:
    sections = []
    for path in sorted(QDIR.glob("*.json")):
        data = json.loads(path.read_text(encoding="utf-8"))
        data["_file"] = path.name
        sections.append(data)
    sections.sort(key=lambda s: s["order"])
    return sections


def validate(sections: list[dict]) -> None:
    errors: list[str] = []
    seen: set[str] = set()
    for sec in sections:
        for q in sec["questions"]:
            qid = q["id"]
            if qid in seen:
                errors.append(f"duplicate question id {qid}")
            seen.add(qid)
            if q["type"] not in ("single", "multi"):
                errors.append(f"{qid}: bad type {q['type']}")
            recs = [o["key"] for o in q["options"] if o.get("rec")]
            if q["type"] == "single" and len(recs) != 1:
                errors.append(f"{qid}: expected exactly 1 recommended option, got {recs}")
            if q["type"] == "multi" and not recs:
                errors.append(f"{qid}: multi question without any recommended option")
            keys = [o["key"] for o in q["options"]]
            if len(keys) != len(set(keys)):
                errors.append(f"{qid}: duplicate option keys")
    missing = [q for q in QUICK_PATH if q not in seen]
    if missing:
        errors.append(f"QUICK_PATH references unknown ids: {missing}")
    if errors:
        print("VALIDATION FAILED", file=sys.stderr)
        for e in errors:
            print("  -", e, file=sys.stderr)
        sys.exit(1)


def slug(text: str) -> str:
    text = unicodedata.normalize("NFKD", text.lower())
    text = "".join(c for c in text if not unicodedata.combining(c))
    text = re.sub(r"[^\w\s-]", "", text, flags=re.UNICODE)
    return re.sub(r"[\s_]+", "-", text).strip("-")


def rec_option(q: dict) -> list[str]:
    return [o["key"] for o in q["options"] if o.get("rec")]


def render_options_md(q: dict, chosen: set[str]) -> str:
    out = []
    for o in q["options"]:
        label = o["label"]
        mark = "x" if o["key"] in chosen else " "
        line = f"- [{mark}] **{o['key']}** — {label}"
        if o.get("rec"):
            line += " — ✅ **doporučeno**"
        if o["key"] in chosen:
            line += " · **zvoleno**"
        out.append(line)
        if o.get("note"):
            out.append(f"  - {o['note']}")
    if q.get("other"):
        out.append("- [ ] **jiné** — ______________________")
    if chosen:
        recs = {o["key"] for o in q["options"] if o.get("rec")}
        same = chosen == recs if q["type"] == "single" else chosen == recs
        verdict = "shoda s doporučením" if same else "⚠ odchylka od doporučení"
        out.append("")
        out.append(f"**Zvoleno:** {', '.join(sorted(chosen))} · *{verdict}*")
    return "\n".join(out)


def build_markdown(sections: list[dict], answers: dict[str, list[str]]) -> str:
    total = sum(len(s["questions"]) for s in sections)
    parts: list[str] = []
    if PREAMBLE.exists():
        parts.append(PREAMBLE.read_text(encoding="utf-8").rstrip())
    parts.append("\n---\n")
    parts.append("## Obsah\n")
    for sec in sections:
        anchor = slug(f"{sec['order']}-{sec['title']}")
        parts.append(f"{sec['order']}. [{sec['title']}](#{anchor}) — {len(sec['questions'])} otázek")
    parts.append("")
    parts.append(f"Celkem **{total} otázek** v {len(sections)} sekcích.")
    parts.append("\n---\n")

    for sec in sections:
        anchor = slug(f"{sec['order']}-{sec['title']}")
        parts.append(f'<a id="{anchor}"></a>\n')
        parts.append(f"## {sec['order']}. {sec['title']}\n")
        if sec.get("intro"):
            parts.append(sec["intro"].strip() + "\n")
        for q in sec["questions"]:
            kind = "vyber jednu" if q["type"] == "single" else "vyber vše, co platí"
            parts.append(f"### {q['id']} — {q['title']}\n")
            parts.append(f"*{kind}*\n")
            parts.append(f"**Proč to řešíme:** {q['why'].strip()}\n")
            parts.append(render_options_md(q, set(answers.get(q['id'], []))) + "\n")
            parts.append("**Poznámka:** ______________________________________________\n")
        parts.append("---\n")

    # Appendix: quick path + answer sheet.
    by_id = {q["id"]: q for s in sections for q in s["questions"]}
    parts.append("## Příloha A — Rychlá cesta (13 otázek, které rozhodují nejvíc)\n")
    parts.append(
        "Pokud nechceš vyplňovat všech "
        f"{total} otázek, odpověz na těchto {len(QUICK_PATH)} a zbytek nech na doporučených "
        "hodnotách. Já je potom potvrdím v DECISIONS.md jako „přijato implicitně\".\n"
    )
    for qid in QUICK_PATH:
        q = by_id[qid]
        recs = ",".join(rec_option(q))
        parts.append(f"- **{qid}** — {q['title']} _(doporučeno: {recs})_")
    parts.append("")

    parts.append("## Příloha B — Odpovědní arch\n")
    parts.append("Zkopíruj a vyplň. Formát: `Q01 = A`, u vícenásobných `Q40 = A,B,C,D`.\n")
    parts.append("```text")
    for sec in sections:
        parts.append(f"# {sec['order']}. {sec['title']}")
        for q in sec["questions"]:
            parts.append(f"{q['id']} = ")
    parts.append("```\n")
    parts.append("## Příloha C — Co se stane po vyplnění\n")
    parts.append(
        "1. Z odpovědí vygeneruji `DECISIONS.md` (ADR-001…ADR-nnn) s odůvodněním a dopady.\n"
        "2. Založím veřejný repozitář: workspace, CI, dokumentaci EN+CS, šablony a roadmapu.\n"
        "3. Postavím M0 walking skeleton a rozdělím práci agentům v Agent Teamu.\n"
        "4. Od každého milníku bude existovat měřitelný výstup (screenshot, benchmark, WPT číslo).\n"
    )
    return "\n".join(parts)


def build_answers_template(sections: list[dict]) -> str:
    lines = ["# Odpovědi na briefing v0.1", "", "Formát: `Q01 = A`, multi: `Q40 = A,B,C`", ""]
    for sec in sections:
        lines.append(f"## {sec['order']}. {sec['title']}")
        lines.append("")
        for q in sec["questions"]:
            recs = ",".join(rec_option(q))
            lines.append(f"# {q['title']}  (doporučeno: {recs})")
            lines.append(f"{q['id']} = ")
            lines.append("")
    return "\n".join(lines)


HTML_TEMPLATE = r"""<!DOCTYPE html>
<html lang="cs">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>__TITLE__</title>
<style>
  :root {
    --bg: #fbfbfd; --panel: #ffffff; --text: #1d1d1f; --muted: #6e6e73;
    --line: #e5e5ea; --accent: #0a84ff; --rec: #248a3d; --rec-bg: #eaf7ee;
    --warn: #b25000; --chip: #f2f2f7; --shadow: 0 1px 2px rgba(0,0,0,.05), 0 8px 24px rgba(0,0,0,.06);
  }
  @media (prefers-color-scheme: dark) {
    :root {
      --bg: #0b0b0d; --panel: #151518; --text: #f5f5f7; --muted: #a1a1a6;
      --line: #2c2c31; --accent: #0a84ff; --rec: #30d158; --rec-bg: #12301c;
      --warn: #ff9f0a; --chip: #1f1f24; --shadow: 0 1px 2px rgba(0,0,0,.4), 0 8px 24px rgba(0,0,0,.5);
    }
  }
  * { box-sizing: border-box; }
  body { margin: 0; background: var(--bg); color: var(--text);
    font: 15px/1.6 -apple-system, BlinkMacSystemFont, "Segoe UI", Inter, sans-serif; }
  header.top { position: sticky; top: 0; z-index: 20; background: color-mix(in srgb, var(--bg) 86%, transparent);
    backdrop-filter: blur(14px); border-bottom: 1px solid var(--line); }
  .top-inner { max-width: 1180px; margin: 0 auto; padding: 12px 20px; display: flex; gap: 16px; align-items: center; flex-wrap: wrap; }
  .brand { font-weight: 650; letter-spacing: -.01em; }
  .brand small { display: block; font-weight: 400; color: var(--muted); font-size: 12px; }
  .progress { flex: 1 1 220px; min-width: 160px; height: 7px; background: var(--chip); border-radius: 99px; overflow: hidden; }
  .progress > div { height: 100%; width: 0; background: var(--accent); transition: width .25s ease; }
  .count { font-variant-numeric: tabular-nums; color: var(--muted); font-size: 13px; white-space: nowrap; }
  button { font: inherit; border: 1px solid var(--line); background: var(--panel); color: var(--text);
    padding: 7px 13px; border-radius: 9px; cursor: pointer; box-shadow: var(--shadow); }
  button:hover { border-color: var(--accent); }
  button.primary { background: var(--accent); border-color: var(--accent); color: #fff; font-weight: 600; }
  button.ghost { box-shadow: none; background: transparent; }
  main { max-width: 1180px; margin: 0 auto; padding: 28px 20px 120px; display: grid; grid-template-columns: 250px minmax(0,1fr); gap: 36px; }
  @media (max-width: 900px) { main { grid-template-columns: 1fr; } nav.side { display: none; } }
  nav.side { position: sticky; top: 76px; align-self: start; font-size: 13.5px; max-height: calc(100vh - 100px); overflow: auto; }
  nav.side a { display: block; padding: 5px 10px; color: var(--muted); text-decoration: none; border-radius: 7px; }
  nav.side a:hover { background: var(--chip); color: var(--text); }
  nav.side a.done { color: var(--rec); }
  nav.side a .n { display: inline-block; width: 26px; color: var(--muted); font-variant-numeric: tabular-nums; }
  h1 { font-size: 27px; line-height: 1.25; letter-spacing: -.02em; margin: 4px 0 8px; }
  .lede { color: var(--muted); margin: 0 0 26px; max-width: 68ch; }
  section.sec { margin: 0 0 42px; scroll-margin-top: 90px; }
  .sec-head { display: flex; gap: 12px; align-items: baseline; border-bottom: 1px solid var(--line); padding-bottom: 8px; margin-bottom: 16px; }
  .sec-head h2 { font-size: 19px; margin: 0; letter-spacing: -.01em; }
  .sec-head .tag { font-size: 12px; color: var(--muted); }
  .sec-intro { color: var(--muted); max-width: 74ch; margin: 0 0 20px; white-space: pre-line; }
  article.q { background: var(--panel); border: 1px solid var(--line); border-radius: 14px; padding: 18px 20px; margin: 0 0 14px; box-shadow: var(--shadow); scroll-margin-top: 90px; }
  article.q.answered { border-color: color-mix(in srgb, var(--rec) 45%, var(--line)); }
  .q-head { display: flex; gap: 10px; align-items: baseline; flex-wrap: wrap; }
  .qid { font: 600 12px/1 ui-monospace, SFMono-Regular, Menlo, monospace; color: var(--accent); background: var(--chip); padding: 5px 7px; border-radius: 6px; }
  .q-title { font-weight: 620; font-size: 16px; letter-spacing: -.01em; }
  .kind { font-size: 12px; color: var(--muted); margin-left: auto; }
  .why { color: var(--muted); font-size: 13.5px; margin: 10px 0 14px; max-width: 82ch; }
  .opts { display: grid; gap: 7px; }
  label.opt { display: grid; grid-template-columns: 20px 1fr; gap: 10px; padding: 10px 12px; border: 1px solid var(--line); border-radius: 10px; cursor: pointer; align-items: start; }
  label.opt:hover { border-color: color-mix(in srgb, var(--accent) 55%, var(--line)); }
  label.opt.rec { background: var(--rec-bg); border-color: color-mix(in srgb, var(--rec) 35%, var(--line)); }
  label.opt input { margin: 3px 0 0; accent-color: var(--accent); width: 16px; height: 16px; }
  .opt-label { font-weight: 560; }
  .opt-note { display: block; color: var(--muted); font-size: 13px; margin-top: 3px; font-weight: 400; }
  code { font: 12.5px ui-monospace, SFMono-Regular, Menlo, monospace; background: var(--chip); padding: 1px 5px; border-radius: 5px; }
  .badge { display: inline-block; font-size: 11px; font-weight: 650; color: var(--rec); background: var(--rec-bg); border: 1px solid color-mix(in srgb, var(--rec) 40%, transparent); padding: 1px 6px; border-radius: 99px; margin-left: 7px; vertical-align: 1px; }
  .other-row { display: flex; gap: 8px; align-items: center; padding: 8px 12px; }
  input[type=text], textarea { width: 100%; font: inherit; color: var(--text); background: var(--bg); border: 1px solid var(--line); border-radius: 8px; padding: 7px 10px; }
  input[type=text]:focus, textarea:focus { outline: 2px solid color-mix(in srgb, var(--accent) 45%, transparent); outline-offset: 1px; }
  details.note { margin-top: 10px; }
  details.note summary { cursor: pointer; color: var(--muted); font-size: 13px; }
  details.note textarea { margin-top: 8px; min-height: 46px; resize: vertical; }
  .toolbar { position: fixed; left: 0; right: 0; bottom: 0; z-index: 20; background: color-mix(in srgb, var(--bg) 90%, transparent); backdrop-filter: blur(14px); border-top: 1px solid var(--line); }
  .toolbar-inner { max-width: 1180px; margin: 0 auto; padding: 10px 20px; display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .toolbar .spacer { flex: 1; }
  .stat { color: var(--muted); font-size: 13px; }
  dialog { border: 1px solid var(--line); border-radius: 16px; padding: 0; max-width: 760px; width: calc(100% - 40px); background: var(--panel); color: var(--text); box-shadow: var(--shadow); }
  dialog::backdrop { background: rgba(0,0,0,.45); backdrop-filter: blur(3px); }
  .dlg-head { padding: 16px 20px; border-bottom: 1px solid var(--line); display: flex; align-items: center; gap: 10px; }
  .dlg-head h3 { margin: 0; font-size: 16px; }
  .dlg-body { padding: 16px 20px; }
  .dlg-body textarea { min-height: 260px; font: 12.5px/1.55 ui-monospace, SFMono-Regular, Menlo, monospace; }
  .dlg-foot { padding: 12px 20px 18px; display: flex; gap: 8px; justify-content: flex-end; }
  .hint { color: var(--muted); font-size: 13px; margin: 6px 0 0; }
  .toast { position: fixed; left: 50%; bottom: 78px; transform: translateX(-50%) translateY(10px); background: var(--text); color: var(--bg); padding: 8px 14px; border-radius: 99px; font-size: 13px; opacity: 0; pointer-events: none; transition: all .2s; z-index: 40; }
  .toast.show { opacity: 1; transform: translateX(-50%) translateY(0); }
  @media print { header.top, .toolbar, nav.side { display: none; } main { display: block; } article.q { break-inside: avoid; } }
</style>
</head>
<body>
<header class="top">
  <div class="top-inner">
    <div class="brand">__TITLE__<small>interaktivní dotazník · odpovědi se ukládají lokálně v prohlížeči</small></div>
    <div class="progress"><div id="bar"></div></div>
    <span class="count" id="count">0 / 0</span>
    <button class="ghost" id="btn-defaults">Použít doporučené</button>
    <button class="ghost" id="btn-reset">Vyčistit</button>
    <button class="primary" id="btn-export">Export odpovědí</button>
  </div>
</header>

<main>
  <nav class="side" id="nav"></nav>
  <div id="content">
    <h1>__TITLE__</h1>
    <p class="lede">__LEDE__</p>
    <div id="sections"></div>
  </div>
</main>

<div class="toolbar">
  <div class="toolbar-inner">
    <span class="stat" id="stat"></span>
    <span class="spacer"></span>
    <button class="ghost" onclick="window.scrollTo({top:0,behavior:'smooth'})">Nahoru</button>
    <button class="primary" id="btn-export2">Export odpovědí</button>
  </div>
</div>

<dialog id="dlg">
  <div class="dlg-head">
    <h3>Odpovědi</h3>
    <span class="stat" id="dlg-stat"></span>
    <span class="spacer" style="flex:1"></span>
    <button class="ghost" id="dlg-close">Zavřít</button>
  </div>
  <div class="dlg-body">
    <textarea id="export-area" spellcheck="false"></textarea>
    <p class="hint">Zkopíruj text a pošli mi ho zpět, nebo ulož jako <code>ANSWERS.md</code>. Import: vlož sem text a klikni na „Načíst“.</p>
  </div>
  <div class="dlg-foot">
    <button class="ghost" id="dlg-copy">Kopírovat</button>
    <button class="ghost" id="dlg-download">Uložit .md</button>
    <button class="primary" id="dlg-load">Načíst z textu</button>
  </div>
</dialog>

<div class="toast" id="toast"></div>

<script>
const DATA = __DATA__;
const KEY = "kva-briefing-v01";
const state = { answers: {}, notes: {}, other: {} };

function normalizeAnswers(src) {
  const out = {};
  for (const q of allQuestions()) {
    const v = (src || {})[q.id];
    if (!v || !v.length) continue;
    out[q.id] = q.type === "multi" ? v.slice() : v[0];
  }
  return out;
}
function load() {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) { state.answers = normalizeAnswers(DATA.answers); return; }
    const obj = JSON.parse(raw);
    state.answers = obj.answers || {};
    state.notes = obj.notes || {};
    state.other = obj.other || {};
  } catch (e) { console.warn("load failed", e); }
}
function save() {
  localStorage.setItem(KEY, JSON.stringify(state));
  updateProgress();
}
function esc(s) {
  return String(s).replace(/[&<>"']/g, c => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c]));
}
// Tiny inline-markdown: **bold** and `code`, applied after escaping.
function md(s) {
  return esc(s)
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/`([^`]+)`/g, "<code>$1</code>");
}
function allQuestions() { return DATA.sections.flatMap(s => s.questions); }
function isAnswered(q) {
  const a = state.answers[q.id];
  if (q.type === "multi") return Array.isArray(a) && a.length > 0;
  return !!a;
}
function updateProgress() {
  const qs = allQuestions();
  const done = qs.filter(isAnswered).length;
  const pct = qs.length ? Math.round(done * 100 / qs.length) : 0;
  document.getElementById("bar").style.width = pct + "%";
  document.getElementById("count").textContent = done + " / " + qs.length;
  document.getElementById("stat").textContent = done + " z " + qs.length + " zodpovězeno";
  for (const sec of DATA.sections) {
    const el = document.querySelector('nav.side a[href="#' + sec.anchor + '"]');
    if (!el) continue;
    el.classList.toggle("done", sec.questions.every(isAnswered));
  }
  document.querySelectorAll("article.q").forEach(el => {
    const q = allQuestions().find(x => x.id === el.dataset.qid);
    el.classList.toggle("answered", q ? isAnswered(q) : false);
  });
}
function render() {
  const content = document.getElementById("content");
  const nav = document.getElementById("nav");
  const wrap = document.getElementById("sections");
  DATA.sections.forEach(sec => {
    const a = document.createElement("a");
    a.href = "#" + sec.anchor;
    a.innerHTML = '<span class="n">' + sec.order + '.</span>' + md(sec.title) +
      ' <span class="n" style="width:auto">(' + sec.questions.length + ')</span>';
    nav.appendChild(a);

    const s = document.createElement("section");
    s.className = "sec"; s.id = sec.anchor;
    s.innerHTML = '<div class="sec-head"><h2>' + sec.order + ". " + md(sec.title) + '</h2><span class="tag">' + sec.questions.length + ' otázek</span></div>' +
      '<p class="sec-intro">' + md(sec.intro).replace(/\n/g, "<br>") + "</p>";

    sec.questions.forEach(q => {
      const art = document.createElement("article");
      art.className = "q"; art.dataset.qid = q.id; art.id = q.id;
      const kind = q.type === "multi" ? "vyber vše, co platí" : "vyber jednu";
      let html = '<div class="q-head"><span class="qid">' + q.id + '</span><span class="q-title">' + md(q.title) +
        '</span><span class="kind">' + kind + '</span></div>' +
        '<p class="why"><strong>Proč:</strong> ' + md(q.why) + "</p>" +
        '<div class="opts" role="group" aria-label="' + esc(q.title) + '">';
      q.options.forEach(o => {
        const checked = q.type === "multi"
          ? (state.answers[q.id] || []).includes(o.key)
          : state.answers[q.id] === o.key;
        html += '<label class="opt' + (o.rec ? " rec" : "") + '">' +
          '<input type="' + (q.type === "multi" ? "checkbox" : "radio") + '" name="' + q.id + '" value="' + o.key + '"' +
          (checked ? " checked" : "") + '>' +
          '<span><span class="opt-label">' + o.key + " — " + md(o.label) +
          (o.rec ? '<span class="badge">doporučeno</span>' : "") + "</span>" +
          (o.note ? '<span class="opt-note">' + md(o.note) + "</span>" : "") +
          "</span></label>";
      });
      html += "</div>";
      if (q.other) {
        html += '<div class="other-row"><span class="qid">jiné</span><input type="text" data-other="' + q.id +
          '" placeholder="vlastní varianta…" value="' + esc(state.other[q.id] || "") + '"></div>';
      }
      html += '<details class="note"' + (state.notes[q.id] ? " open" : "") + "><summary>Poznámka</summary>" +
        '<textarea data-note="' + q.id + '" placeholder="cokoli, co chceš doplnit…">' + esc(state.notes[q.id] || "") + "</textarea></details>";
      art.innerHTML = html;
      s.appendChild(art);
    });
    wrap.appendChild(s);
  });

  document.getElementById("content").addEventListener("change", onInput);
  document.getElementById("content").addEventListener("input", onInput);
}
function onInput(ev) {
  const t = ev.target;
  if (t.type === "radio" || t.type === "checkbox") {
    const q = allQuestions().find(x => x.id === t.name);
    if (!q) return;
    if (q.type === "multi") {
      const set = new Set(state.answers[q.id] || []);
      t.checked ? set.add(t.value) : set.delete(t.value);
      state.answers[q.id] = [...set];
    } else {
      state.answers[q.id] = t.value;
    }
    save();
  } else if (t.dataset.other) {
    state.other[t.dataset.other] = t.value; save();
  } else if (t.dataset.note) {
    state.notes[t.dataset.note] = t.value; save();
  }
}
function exportText() {
  const lines = ["# Odpovědi na briefing v0.1", ""];
  const deviations = [];
  for (const sec of DATA.sections) {
    lines.push("## " + sec.order + ". " + sec.title, "");
    for (const q of sec.questions) {
      const a = state.answers[q.id];
      const recs = q.options.filter(o => o.rec).map(o => o.key);
      if (q.type === "multi") {
        const arr = Array.isArray(a) ? a : [];
        lines.push("# " + q.title);
        lines.push(q.id + " = " + arr.join(","));
        if (arr.length && JSON.stringify([...arr].sort()) !== JSON.stringify([...recs].sort())) {
          deviations.push(q.id + " (" + arr.join(",") + " vs " + recs.join(",") + ")");
        }
      } else {
        lines.push("# " + q.title);
        lines.push(q.id + " = " + (a || ""));
        if (a && !recs.includes(a)) deviations.push(q.id + " (" + a + " vs " + recs.join(",") + ")");
      }
      if (state.other[q.id]) lines.push(q.id + "-jine = " + state.other[q.id]);
      if (state.notes[q.id]) lines.push(q.id + "-poznamka = " + state.notes[q.id].replace(/\n/g, " "));
      lines.push("");
    }
  }
  lines.push("## Odchylky od doporučení", "");
  lines.push(deviations.length ? deviations.map(d => "- " + d).join("\n") : "Žádné — vše podle doporučení.");
  lines.push("");
  return lines.join("\n");
}
function toast(msg) {
  const t = document.getElementById("toast");
  t.textContent = msg; t.classList.add("show");
  setTimeout(() => t.classList.remove("show"), 1600);
}
function openDialog() {
  const dlg = document.getElementById("dlg");
  document.getElementById("export-area").value = exportText();
  const qs = allQuestions();
  const done = qs.filter(isAnswered).length;
  document.getElementById("dlg-stat").textContent = done + "/" + qs.length;
  dlg.showModal();
}
function parseImport(text) {
  const answers = {}; const notes = {}; const other = {};
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    const m = line.match(/^(Q\d+)\s*=\s*(.*)$/);
    if (m) {
      const q = allQuestions().find(x => x.id === m[1]);
      if (!q) continue;
      const val = m[2].trim();
      answers[q.id] = q.type === "multi" ? val.split(",").map(s => s.trim()).filter(Boolean) : val;
      continue;
    }
    const mo = line.match(/^(Q\d+)-jine\s*=\s*(.*)$/);
    if (mo) { other[mo[1]] = mo[2]; continue; }
    const mn = line.match(/^(Q\d+)-poznamka\s*=\s*(.*)$/);
    if (mn) { notes[mn[1]] = mn[2]; }
  }
  state.answers = answers; state.notes = notes; state.other = other;
  save(); location.reload();
}
document.addEventListener("DOMContentLoaded", () => {
  document.getElementById("btn-defaults").onclick = () => {
    if (!confirm("Nastavit doporučené odpovědi u všech nezodpovězených otázek?")) return;
    for (const q of allQuestions()) {
      if (isAnswered(q)) continue;
      state.answers[q.id] = q.type === "multi" ? q.options.filter(o => o.rec).map(o => o.key) : q.options.find(o => o.rec).key;
    }
    save(); location.reload();
  };
  document.getElementById("btn-reset").onclick = () => {
    if (!confirm("Vymazat všechny odpovědi?")) return;
    localStorage.removeItem(KEY); state.answers = {}; state.notes = {}; state.other = {};
    location.reload();
  };
  document.getElementById("btn-export").onclick = openDialog;
  document.getElementById("btn-export2").onclick = openDialog;
  document.getElementById("dlg-close").onclick = () => document.getElementById("dlg").close();
  document.getElementById("dlg-copy").onclick = async () => {
    try { await navigator.clipboard.writeText(exportText()); toast("Zkopírováno"); }
    catch (e) { toast("Kopírování selhalo — vyber text ručně"); }
  };
  document.getElementById("dlg-download").onclick = () => {
    const blob = new Blob([exportText()], {type: "text/markdown;charset=utf-8"});
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob); a.download = "ANSWERS.md"; a.click();
    URL.revokeObjectURL(a.href);
  };
  document.getElementById("dlg-load").onclick = () => {
    const t = document.getElementById("export-area").value;
    if (!t.includes("Q")) { toast("Text neobsahuje odpovědi"); return; }
    parseImport(t);
  };
  load(); render(); updateProgress();
});
</script>
</body>
</html>
"""


def build_html(sections: list[dict], title: str, lede: str, answers: dict[str, list[str]]) -> str:
    data = {"sections": [], "answers": answers}
    for sec in sections:
        data["sections"].append({
            "order": sec["order"],
            "title": sec["title"],
            "intro": sec.get("intro", ""),
            "anchor": slug(f"{sec['order']}-{sec['title']}"),
            "questions": [
                {
                    "id": q["id"], "title": q["title"], "why": q["why"], "type": q["type"],
                    "other": bool(q.get("other")),
                    "options": [
                        {"key": o["key"], "label": o["label"], "note": o.get("note", ""), "rec": bool(o.get("rec"))}
                        for o in q["options"]
                    ],
                }
                for q in sec["questions"]
            ],
        })
    payload = json.dumps(data, ensure_ascii=False).replace("</", "<\\/")
    return (HTML_TEMPLATE
            .replace("__DATA__", payload)
            .replace("__TITLE__", title)
            .replace("__LEDE__", lede))


def main() -> None:
    sections = load_sections()
    validate(sections)
    answers = load_answers()
    total = sum(len(s["questions"]) for s in sections)
    md = build_markdown(sections, answers)
    (ROOT / "BRIEFING-0.1.cs.md").write_text(md, encoding="utf-8")
    (ROOT / "ANSWERS-template.md").write_text(build_answers_template(sections), encoding="utf-8")
    title = "JKva 0.1 — briefing pro vývoj jádra prohlížeče"
    lede = (f"{total} rozhodnutí, která je potřeba udělat před prvním commitem. "
            "Dotazník je předvyplněný tvými odpověďmi — můžeš je změnit a znovu exportovat. "
            "Doporučené varianty jsou označené zeleně.")
    (ROOT / "questionnaire.html").write_text(build_html(sections, title, lede, answers), encoding="utf-8")
    print(f"OK: {total} questions in {len(sections)} sections")
    print(f"  {ROOT / 'BRIEFING-0.1.cs.md'}")
    print(f"  {ROOT / 'questionnaire.html'}")
    print(f"  {ROOT / 'ANSWERS-template.md'}")


if __name__ == "__main__":
    main()
