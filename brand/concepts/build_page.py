import base64
import io
import importlib
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent
PROJECT = ROOT.parent.parent
FONTS = pathlib.Path("C:/Users/stillmvd/Projects/Kockers DS/fonts")
OUT = ROOT / "logo-page.html"
WAVES = ["wave1", "wave2", "wave3", "wave4"]
INKSCAPE = "C:/Program Files/Inkscape/bin/inkscape.com"

sys.path.insert(0, str(ROOT))

CHECK = ("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 20 20'>"
         "<path d='M5.5 10.5l3 3 6-6.5' fill='none' stroke='white' stroke-width='2' stroke-linecap='round' "
         "stroke-linejoin='round'/></svg>")


def font_face(weight, file):
    data = base64.b64encode((FONTS / file).read_bytes()).decode()
    return (f'@font-face{{font-family:"Kockers Sans";font-weight:{weight};font-style:normal;font-display:swap;'
            f'src:url(data:font/woff2;base64,{data}) format("woff2")}}')


CSS = """
:root{--ground:#f4f4f6;--cosmic:#ffffff;--raised:#ebebee;--line:#dedee2;--hover-strong:#c9c9cf;--fg:#17171a;
  --dim:#56565e;--accent:#9a3fc2;--accent-ink:#ffffff;--paper:#ffffff;--paper-line:#dedee2}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--ground:#141416;--cosmic:#1c1c1f;
  --raised:#242427;--line:#2e2e33;--hover-strong:#4a4a52;--fg:#ececef;--dim:#a2a2a9;--accent:#e9a8ff;
  --accent-ink:#2c1038;--paper-line:#2e2e33}}
:root[data-theme="dark"]{--ground:#141416;--cosmic:#1c1c1f;--raised:#242427;--line:#2e2e33;--hover-strong:#4a4a52;
  --fg:#ececef;--dim:#a2a2a9;--accent:#e9a8ff;--accent-ink:#2c1038;--paper-line:#2e2e33}
*{box-sizing:border-box}
body{margin:0;background:var(--ground);color:var(--fg);font-family:"Kockers Sans","Segoe UI",system-ui,sans-serif;
  font-size:15px;line-height:1.55;font-synthesis-weight:none;padding-inline:16px}
.wrap{max-width:1120px;margin:0 auto;padding-block:40px 132px;display:grid;gap:32px}
header{display:grid;gap:8px}
h1{margin:0;font-size:24px;font-weight:700;letter-spacing:-.01em;line-height:1.2}
.brief{margin:0;color:var(--dim);max-width:72ch}
.wave{display:grid;gap:24px;background:var(--cosmic);border-radius:28px;padding:28px}
.wave-head{display:grid;gap:6px}
h2{margin:0;font-size:19px;font-weight:700;line-height:1.25;text-wrap:balance}
.prompt{margin:0;color:var(--dim);max-width:72ch}
.chosen{margin:0;font-weight:500}
.chosen b{color:var(--accent);font-weight:700}
h3{margin:0;font-size:12px;font-weight:500;letter-spacing:.08em;text-transform:uppercase;color:var(--dim)}
.group{display:grid;gap:12px}
.grid{display:grid;gap:14px;grid-template-columns:repeat(auto-fill,minmax(min(100%,250px),1fr))}
.grid.wide{grid-template-columns:repeat(auto-fill,minmax(min(100%,520px),1fr))}
.card{display:grid;gap:10px;align-content:start}
.tiles{display:grid;gap:10px;grid-template-columns:repeat(auto-fit,minmax(min(100%,230px),1fr))}
.pick{display:grid;gap:6px;cursor:pointer}
.pick input{position:absolute;opacity:0;pointer-events:none;width:1px;height:1px}
.tile{position:relative;background:var(--paper);border:1px solid var(--paper-line);border-radius:22px;
  padding:22px 16px 14px;display:grid;gap:16px;justify-items:center;
  transition:border-color .16s cubic-bezier(.2,.8,.2,1),box-shadow .16s cubic-bezier(.2,.8,.2,1)}
.tile::after{content:"";position:absolute;top:12px;right:12px;width:22px;height:22px;border-radius:50%;
  border:1.5px solid #c9c9cf;background:#fff no-repeat center/20px}
.pick:hover .tile{border-color:var(--hover-strong)}
.pick input:checked + .tile{border-color:var(--accent);box-shadow:0 0 0 1px var(--accent)}
.pick input:checked + .tile::after{border-color:#9a3fc2;background-color:#9a3fc2;background-image:url("__CHECK__")}
.pick input:focus-visible + .tile{outline:2px solid var(--fg);outline-offset:3px}
.pick.static{cursor:default}
.pick.static .tile::after{display:none}
.tile .big{width:120px;height:120px}
.sizes{display:flex;align-items:flex-end;gap:16px}
.size{display:grid;gap:5px;justify-items:center;font-size:11px;color:#6b6b73;font-variant-numeric:tabular-nums}
.tile-cap{font-size:13px;color:var(--dim);padding-inline:6px}
.name{display:flex;gap:10px;align-items:baseline}
.code{font-weight:700;color:var(--accent);font-variant-numeric:tabular-nums}
.title{font-weight:700}
.note{margin:0;color:var(--dim);font-size:14px;max-width:68ch}
.build{display:grid;gap:12px}
.build-row{display:grid;gap:10px;grid-template-columns:repeat(auto-fit,minmax(min(100%,300px),1fr))}
.build-tile{border-radius:22px;padding:26px 16px 16px;display:grid;gap:18px;justify-items:center}
.build-tile .big{width:168px;height:168px}
.build-tile.light{background:#fff;color:#161618;border:1px solid var(--paper-line)}
.build-tile.dark{background:#141416;color:#ececef;border:1px solid #2e2e33}
.build-tile.dark .size{color:#8a8a92}
.lockup{width:100%;max-width:380px;height:auto}
.summary{margin:0;padding:16px 18px;border-radius:18px;background:var(--raised)}
.bar{position:fixed;left:16px;right:16px;bottom:calc(16px + env(safe-area-inset-bottom,0px));max-width:1120px;
  margin-inline:auto;display:flex;align-items:center;gap:12px;padding:8px 8px 8px 22px;border-radius:999px;
  background:var(--raised);border:1px solid var(--line);box-shadow:0 10px 30px rgb(0 0 0 / 18%)}
.bar-text{flex:1;min-width:0;font-weight:500;user-select:all;overflow-wrap:anywhere}
.bar button{font:inherit;font-weight:700;border:0;border-radius:999px;padding:10px 18px;cursor:pointer;
  background:var(--accent);color:var(--accent-ink);white-space:nowrap}
.bar button:disabled{background:var(--line);color:var(--dim);cursor:default}
.bar.can-send #copy{background:transparent;color:var(--fg);box-shadow:inset 0 0 0 1px var(--line)}
.bar #send:disabled{background:var(--accent);color:var(--accent-ink);opacity:.6;cursor:progress}
.bar button:focus-visible{outline:2px solid var(--fg);outline-offset:2px}
@media (max-width:560px){.wave{padding:18px;border-radius:22px}.bar{border-radius:22px;padding-left:16px}}
@media (prefers-reduced-motion:reduce){.tile{transition:none}}
""".replace("__CHECK__", CHECK)

SCRIPT = """
const bar = document.getElementById("choice");
const copy = document.getElementById("copy");
const active = document.querySelector("[data-active]");
function update() {
  if (!active) { bar.parentElement.hidden = true; return; }
  const picked = [...active.querySelectorAll(".pick input:checked")].map((input) => input.value);
  bar.textContent = `${active.dataset.label}: ${picked.length ? picked.join(" · ") : "—"}`;
  copy.disabled = !picked.length;
  const combo = [...active.querySelectorAll(".card[data-radio]")]
    .map((card) => card.querySelector("input:checked")?.dataset.idx ?? "1").join("-");
  active.querySelectorAll("[data-combo]").forEach((row) => { row.hidden = row.dataset.combo !== combo; });
}
document.addEventListener("change", (event) => { if (event.target.closest(".pick")) update(); });
(async () => {
  const send = document.getElementById("send");
  const box = bar.parentElement;
  if (!window.claude?.use) return;
  const comments = await window.claude.use("comments");
  if (!comments?.sendToClaude) return;
  const sync = async () => {
    const ok = (await comments.canSendToClaude().catch(() => "off")) === "available";
    send.hidden = !ok;
    box.classList.toggle("can-send", ok);
  };
  const say = (text, off = false) => {
    send.textContent = text;
    setTimeout(() => {
      send.textContent = "Отправить";
      send.disabled = false;
      if (off) { send.hidden = true; box.classList.remove("can-send"); }
    }, 3500);
  };
  await sync();
  window.addEventListener("focus", sync);
  let sent = "";
  send.addEventListener("click", async () => {
    const text = bar.textContent.trim();
    if (send.disabled || copy.disabled) return;
    send.disabled = true;
    if (text === sent) return say("Уже отправлено");
    send.textContent = "Отправляю…";
    try {
      await comments.sendToClaude({ anchor: await comments.anchorFor(box), text });
      sent = text;
      say("Отправлено");
    } catch (err) {
      const code = err?.code;
      if (code === "consent_required") return say("Не разрешено");
      if (code === "claude_unavailable") return say("Сессия не слушает");
      say("Не отправилось", ["forbidden", "not_granted", "capability_disabled", "capability_removed"].includes(code));
    }
  });
})();
copy.addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText(bar.textContent);
    copy.textContent = "Скопировано";
  } catch {
    const range = document.createRange();
    range.selectNodeContents(bar);
    const selection = getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
    copy.textContent = "Выделено — Ctrl+C";
  }
  setTimeout(() => { copy.textContent = "Скопировать"; }, 1800);
});
update();
"""


def svg(inner, cls="", size=None):
    attrs = f' class="{cls}"' if cls else ""
    if size:
        attrs += f' width="{size}" height="{size}"'
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"{attrs} aria-hidden="true">{inner}</svg>'


def tile(mod, uid, spec, interactive, chosen, radio=None, idx=0):
    token, caption, key, in_circle, *rest = spec
    default = bool(rest and rest[0])
    sizes = "".join(f'<span class="size">{svg(mod.variant_svg(f"{uid}-{px}", key, in_circle), size=px)}{px}</span>'
                    for px in (48, 32, 24, 16))
    body = (f'<span class="tile">{svg(mod.variant_svg(f"{uid}-big", key, in_circle), "big")}'
            f'<span class="sizes">{sizes}</span></span>')
    cap = f'<span class="tile-cap">{caption}</span>' if caption else ""
    if interactive:
        kind = f'radio" name="{radio}' if radio else "checkbox"
        checked = " checked" if default else ""
        return (f'<label class="pick"><input type="{kind}" id="{uid}" value="{token}" data-idx="{idx}"{checked}>'
                f'{body}{cap}</label>')
    checked = " checked" if token in chosen else ""
    return f'<div class="pick static"><input type="checkbox" disabled{checked}>{body}{cap}</div>'


def sheet(mod):
    items = [(key, circ) for _, cards in mod.CARDS for card in cards for _, _, key, circ, *_ in card["tiles"]]
    parts = []
    for i, (key, circ) in enumerate(items):
        x = 16 + i * 150
        uid = f"s{i}"
        parts.append(f'<svg x="{x}" y="16" width="128" height="128" viewBox="0 0 64 64">{mod.variant_svg(uid + "b", key, circ)}</svg>')
        for px, sx in ((48, x), (32, x + 58), (24, x + 100)):
            parts.append(f'<svg x="{sx}" y="{160 + 48 - px}" width="{px}" height="{px}" viewBox="0 0 64 64">'
                         f'{mod.variant_svg(uid + str(px), key, circ)}</svg>')
    w = 16 + len(items) * 150
    path = ROOT / f"wave{mod.NUMBER}-sheet.svg"
    path.write_text(f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="228" viewBox="0 0 {w} 228">'
                    f'<rect width="100%" height="100%" fill="#fff"/>{"".join(parts)}</svg>', encoding="utf-8")
    subprocess.run([INKSCAPE, str(path), "--export-type=png", f"--export-filename={path.with_suffix('.png')}",
                    "--export-dpi=96"], check=True, capture_output=True)
    print("sheet", path.with_suffix(".png"))


def wave_section(mod, interactive):
    chosen = mod.CHOSEN or []
    groups = []
    for gi, (group_title, card_list) in enumerate(mod.CARDS):
        cards = []
        wide = any(len(card["tiles"]) > 1 for card in card_list)
        for ci, card in enumerate(card_list):
            radio = f"w{mod.NUMBER}-{gi}-{ci}" if card.get("radio") else None
            tiles = "".join(tile(mod, f"w{mod.NUMBER}-{gi}-{ci}-{ti}", t, interactive, chosen, radio, ti)
                            for ti, t in enumerate(card["tiles"]))
            data = " data-radio" if radio else ""
            cards.append(
                f'<article class="card"{data}><div class="tiles">{tiles}</div>'
                f'<div class="name"><span class="code">{card["code"]}</span><span class="title">{card["name"]}</span></div>'
                f'<p class="note">{card["note"]}</p></article>')
        groups.append(f'<div class="group"><h3>{group_title}</h3>'
                      f'<div class="grid{" wide" if wide else ""}">{"".join(cards)}</div></div>')
    head = f'<h2>{mod.TITLE}</h2>'
    if interactive:
        head += f'<p class="prompt">{mod.PROMPT}</p>'
    else:
        head += f'<p class="chosen">Выбрано: <b>{" · ".join(chosen)}</b></p>'
    attrs = f' data-active data-label="Волна {mod.NUMBER}"' if interactive else ""
    build = mod.preview() if interactive and hasattr(mod, "preview") else ""
    return (f'<section class="wave"{attrs}><div class="wave-head">{head}</div>{build}{"".join(groups)}'
            f'<p class="summary">{mod.SUMMARY}</p></section>')


LOGO = ROOT.parent / "logo"
ICONS = PROJECT / "src-tauri/icons"

FINAL_CSS = """
.tray{display:flex;align-items:center;justify-content:flex-end;gap:14px;height:48px;padding:0 18px;border-radius:14px}
.tray.light{background:#eef0f4;border:1px solid var(--paper-line)}
.tray.dark{background:#1c1c1f;border:1px solid #2e2e33}
.tray i{width:16px;height:16px;border-radius:4px;background:#8a8a92;opacity:.55}
.tray .clock{font-size:12px;line-height:1.2;text-align:right;font-variant-numeric:tabular-nums}
.tray.light .clock{color:#17171a}.tray.dark .clock{color:#ececef}
"""


def ico_png(px):
    from PIL import Image
    im = Image.open(ICONS / "icon.ico")
    im.size = (px, px)
    im.load()
    buf = io.BytesIO()
    im.save(buf, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


def png(path):
    return "data:image/png;base64," + base64.b64encode(path.read_bytes()).decode()


def final_section():
    if not (LOGO / "mark.svg").exists():
        return ""
    mark = (LOGO / "mark.svg").read_text(encoding="utf-8").replace("<svg ", '<svg class="big" ', 1)
    big = "".join(f'<div class="build-tile {theme}">{mark}</div>' for theme in ("light", "dark"))
    icons = "".join(f'<span class="size"><img src="{ico_png(px)}" width="{px}" height="{px}" alt="">{px}</span>'
                    for px in (128, 64, 48, 32, 24))
    icon_row = "".join(f'<div class="build-tile {theme}"><span class="sizes">{icons}</span></div>' for theme in ("light", "dark"))
    trays = "".join(
        f'<div class="tray {theme}"><i></i><i></i>'
        + "".join(f'<img src="{png(ICONS / "tray" / f"{theme}-{state}.png")}" width="16" height="16" alt="">'
                  for state in ("on", "off"))
        + '<i></i><span class="clock">9:41<br>20.10.2026</span></div>'
        for theme in ("light", "dark"))
    return (f'<section class="wave final"><div class="wave-head"><h2>Готовый знак</h2>'
            f'<p class="prompt">A1 — пластинка табло с T, два тона сирени (C2), 94 % поля иконки. Геометрия — '
            f'brand/logo/generate.py: SVG, иконка приложения, трей и знак в окне; до 32 px включительно и в трее '
            f'берётся упрощённый знак с щелью и T шире. В трее — знак во весь значок, рядом выключенный.</p></div>'
            f'<div class="build"><h3>Знак</h3><div class="build-row">{big}</div></div>'
            f'<div class="build"><h3>Иконка приложения</h3><div class="build-row">{icon_row}</div></div>'
            f'<div class="build"><h3>Трей — включён и выключен</h3><div class="build-row">{trays}</div></div></section>')


def main():
    mods = [importlib.import_module(w) for w in WAVES]
    fonts = "".join(font_face(w, f) for w, f in ((400, "KockersSans-Regular.woff2"), (500, "KockersSans-Medium.woff2"),
                                                   (700, "KockersSans-Bold.woff2")))
    sections = "".join(wave_section(m, m.CHOSEN is None) for m in reversed(mods))
    html = (f'<title>Знак Tolk</title><style>{fonts}{CSS}{FINAL_CSS}{"".join(getattr(m, "CSS", "") for m in mods)}</style>'
            f'<main class="wrap"><header><h1>Знак Tolk</h1>'
            f'<p class="brief">Иконка приложения, панель задач, установщик, трей (16 px — крупно, как у соседей), знак в окнах. '
            f'Сверху готовый знак, ниже волны поиска — новая сверху.</p></header>'
            f'{final_section()}{sections}</main>'
            f'<div class="bar" role="status" aria-live="polite"><span class="bar-text" id="choice">—</span>'
            f'<button type="button" id="send" hidden>Отправить</button>'
            f'<button type="button" id="copy">Скопировать</button></div>'
            f'<script>{SCRIPT}</script>')
    OUT.write_text(html, encoding="utf-8")
    if "--sheet" in sys.argv:
        sheet(mods[-1])
    print("ok", OUT, len(html))


if __name__ == "__main__":
    main()
