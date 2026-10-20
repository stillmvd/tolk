import wave3
from wave1 import path_d, rrect

LIGHT_BG = "#ffffff"
DARK_BG = "#1c1c1f"

COLORS = {
    "c1": ("#e9a8ff", "#e9a8ff", None),
    "c2": ("#f3c6ff", "#c77de8", None),
    "c3": ("#2c1038", "#2c1038", "#e9a8ff"),
    "c4": ("#9a3fc2", "#9a3fc2", None),
    "off": ("#8a8a92", "#8a8a92", None),
}


def mark(cid, color, frac, bg):
    top_c, bottom_c, t_c = COLORS[color]
    w = h = 40
    gap, t, r = 3.6, 6, 7
    mid = h / 2
    s = 64 * frac / 40
    o = (64 - 40 * s) / 2
    top = path_d(rrect(0, 0, w, mid - gap / 2, r), True)
    bottom = path_d(rrect(0, mid + gap / 2, w, mid - gap / 2, r), True)
    tee = f"M10 10 L30 10 M20 10 L20 32"
    gap_line = f"M-2 {mid} L42 {mid}"
    t_hole = "" if t_c else f'<path d="{tee}" stroke="#000" stroke-width="{t}" fill="none"/>'
    mask = (f'<mask id="{cid}-m" maskUnits="userSpaceOnUse" x="-5" y="-5" width="50" height="50">'
            f'<rect x="-5" y="-5" width="50" height="50" fill="#fff"/>{t_hole}'
            f'<path d="{gap_line}" stroke="#000" stroke-width="{gap}"/></mask>')
    t_draw = f'<path d="{tee}" stroke="{t_c}" stroke-width="{t}" fill="none"/>' if t_c else ""
    back = ""
    if bg == "dark":
        back = f'<rect width="64" height="64" rx="12" fill="{DARK_BG}"/>'
    elif bg == "light":
        back = f'<rect width="64" height="64" rx="12" fill="{LIGHT_BG}" stroke="#dedee2"/>'
    return (f'{back}<defs>{mask}</defs><g transform="translate({o:.3f} {o:.3f}) scale({s:.4f})">'
            f'<g mask="url(#{cid}-m)"><path d="{top}" fill="{top_c}"/><path d="{bottom}" fill="{bottom_c}"/>'
            f'{t_draw}</g></g>')


def variant_svg(cid, key, in_circle):
    if "|" not in key:
        return wave3.variant_svg(cid, key, in_circle)
    color, bg, frac = key.split("|")
    return mark(cid, color, int(frac) / 100, bg)


def pair(code, color):
    return [(f"{code} на светлом", "на светлом", f"{color}|light|86", False),
            (f"{code} на тёмном", "на тёмном", f"{color}|dark|86", False)]


NUMBER = 4
TITLE = "Волна 4 · цвет и размер"
PROMPT = ("Форма A1 зафиксирована. Цвет — в сирени Tolk, каждый на светлом и тёмном фоне. T и щель — "
          "сквозные: сквозь них виден фон. Отметь цвет, размер в иконке и вид трея.")
CHOSEN = ["C2", "S 94 %", "T вкл", "T выкл"]

CARDS = [
    ("Цвет", [
        {"code": "C1", "name": "Светлая сирень",
         "note": "Акцент Tolk #e9a8ff целиком. Ярко на тёмном, на светлом мягче, но читается.",
         "tiles": pair("C1", "c1")},
        {"code": "C2", "name": "Два тона",
         "note": "Верхняя пластинка светлее нижней — как свет на настоящем табло. Объём без градиента.",
         "tiles": pair("C2", "c2")},
        {"code": "C3", "name": "Тёмное табло",
         "note": "Тёмные пластинки, сиреневая T — как настоящее табло ночью. Сильный характер, но на тёмной "
                 "панели задач пластинки сливаются с фоном.",
         "tiles": pair("C3", "c3")},
        {"code": "C4", "name": "Насыщенная сирень",
         "note": "Тёмный акцент светлой темы #9a3fc2. Солидно на светлом, на тёмном глуше.",
         "tiles": pair("C4", "c4")},
    ]),
    ("Размер в иконке и трей", [
        {"code": "S", "name": "Доля в иконке приложения", "radio": True,
         "note": "Сколько квадрата иконки занимает знак. 86 % — как у большинства значков Windows; 94 % — крупнее "
                 "на панели задач, но тесно у края.",
         "tiles": [("S 78 %", "78 %", "c2|none|78", False), ("S 86 %", "86 %", "c2|none|86", False, True),
                   ("S 94 %", "94 %", "c2|none|94", False)]},
        {"code": "T", "name": "Трей — во весь значок",
         "note": "В трее знак занимает все 16 px, как значки соседей; щель и T — сквозные. Выключенный Tolk — "
                 "серый.",
         "tiles": [("T вкл", "включён", "c2|none|100", False), ("T выкл", "выключен", "off|none|100", False)]},
    ]),
]

SUMMARY = ("Моё мнение: C2 — живее всех и держится на обеих темах; C1 — запасной, если два тона покажутся "
           "лишними. C3 красивый, но тонет на тёмной панели задач. Размер — 86 %.")
