import math

INK = "#17171a"
PAPER = "#ffffff"


def arc(cx, cy, r, a0, a1, step=2):
    n = max(2, int(abs(a1 - a0) / step) + 1)
    return [(cx + r * math.cos(math.radians(a0 + (a1 - a0) * i / (n - 1))),
             cy + r * math.sin(math.radians(a0 + (a1 - a0) * i / (n - 1)))) for i in range(n)]


def line(pts, w, color="ink", cap="round", join="round", closed=False):
    return {"pts": pts, "w": w, "cap": cap, "join": join, "closed": closed, "fill": None, "color": color, "halo": 0}


def shape(pts, color="ink", halo=0):
    return {"pts": pts, "w": 0, "cap": "round", "join": "round", "closed": True, "fill": True, "color": color,
            "halo": halo}


def rrect(x, y, w, h, r):
    r = min(r, w / 2, h / 2)
    return (arc(x + w - r, y + r, r, -90, 0) + arc(x + w - r, y + h - r, r, 0, 90)
            + arc(x + r, y + h - r, r, 90, 180) + arc(x + r, y + r, r, 180, 270))


def sparkle(cx, cy, r, k=3.2, rot=0):
    pts = []
    for i in range(240):
        t = 2 * math.pi * i / 240
        c, s = math.cos(t), math.sin(t)
        x = r * math.copysign(abs(c) ** k, c)
        y = r * math.copysign(abs(s) ** k, s)
        a = math.radians(rot)
        pts.append((cx + x * math.cos(a) - y * math.sin(a), cy + x * math.sin(a) + y * math.cos(a)))
    return pts


def arrow_head(tip, frm, size):
    dx, dy = tip[0] - frm[0], tip[1] - frm[1]
    d = math.hypot(dx, dy)
    ux, uy = dx / d, dy / d
    bx, by = tip[0] - ux * size, tip[1] - uy * size
    return [tip, (bx - uy * size * 0.62, by + ux * size * 0.62), (bx + uy * size * 0.62, by - ux * size * 0.62)]


def bbox(items):
    xs, ys = [], []
    for it in items:
        h = max(it["w"], it["halo"]) / 2
        for x, y in it["pts"]:
            xs += [x - h, x + h]
            ys += [y - h, y + h]
    return min(xs), min(ys), max(xs), max(ys)


def path_d(pts, closed):
    d = "M" + " L".join(f"{x:.2f} {y:.2f}" for x, y in pts)
    return d + (" Z" if closed else "")


def item_svg(it, ink, paper):
    color = ink if it["color"] == "ink" else paper
    d = path_d(it["pts"], it["closed"])
    if it["fill"]:
        halo = (f' stroke="{paper}" stroke-width="{it["halo"]}" stroke-linejoin="round" paint-order="stroke"'
                if it["halo"] else "")
        return f'<path d="{d}" fill="{color}"{halo}/>'
    return (f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{it["w"]}" stroke-linecap="{it["cap"]}" '
            f'stroke-linejoin="{it["join"]}" stroke-miterlimit="12"/>')


def glyph_svg(items, size, ink=INK, paper=PAPER):
    x0, y0, x1, y1 = bbox([it for it in items if it["color"] == "ink" or it["fill"] is None])
    s = size / max(x1 - x0, y1 - y0)
    tx = 32 - s * (x0 + x1) / 2
    ty = 32 - s * (y0 + y1) / 2
    body = "".join(item_svg(it, ink, paper) for it in items)
    return f'<g transform="translate({tx:.3f} {ty:.3f}) scale({s:.4f})">{body}</g>'


def split_flap():
    gap = 3.6
    top = rrect(0, 0, 40, 20 - gap / 2, 7)
    bottom = rrect(0, 20 + gap / 2, 40, 20 - gap / 2, 7)
    return [shape(top), shape(bottom),
            line([(10, 10), (30, 10)], 6, "paper", cap="butt"),
            line([(20, 10), (20, 32)], 6, "paper", cap="butt"),
            line([(-2, 20), (42, 20)], gap, "paper", cap="butt")]


def pill_caret():
    return [shape(rrect(0, 7, 28, 18, 5)),
            line([(36, 2), (36, 30)], 4),
            line([(32, 2), (40, 2)], 4),
            line([(32, 30), (40, 30)], 4)]


def word_turn():
    r = 18
    top = arc(0, 0, r, 205, 322)
    bottom = arc(0, 0, r, 25, 142)
    return [line([(-9, 0), (9, 0)], 6),
            line(top[:-3], 4.2), shape(arrow_head(top[-1], top[-6], 7.5)),
            line(bottom[:-3], 4.2), shape(arrow_head(bottom[-1], bottom[-6], 7.5))]


def twin_sparkle():
    return [shape(sparkle(0, 0, 20)), shape(sparkle(19, -15, 7.5))]


def bolt_caret():
    w = 4.6
    return [line([(-7, 0), (7, 0)], w),
            line([(-7, 40), (7, 40)], w),
            line([(2.5, 0), (-4, 21), (4, 19), (-2.5, 40)], w, join="miter")]


def pill_spark():
    return [shape(rrect(0, 0, 40, 20, 10)),
            shape(sparkle(34, 10, 15), halo=6)]


VARIANTS = {
    "a1-flap": split_flap,
    "a2-caret": pill_caret,
    "a3-turn": word_turn,
    "b1-sparkle": twin_sparkle,
    "b2-bolt": bolt_caret,
    "b3-pill-spark": pill_spark,
}


def variant_svg(cid, key, in_circle):
    items = VARIANTS[key]()
    if in_circle:
        return f'<circle cx="32" cy="32" r="32" fill="{INK}"/>' + glyph_svg(items, 40, PAPER, INK)
    return glyph_svg(items, 56)


NUMBER = 1
TITLE = "Волна 1 · направления"
PROMPT = ("Две метафоры по три варианта, монохром — цвет и подложка позже. Под каждым ряд 48 / 32 / 24 / 16: "
          "16 — размер трея. Отметь, что оставить, можно несколько.")
CHOSEN = ["A1", "B3"]

CARDS = [
    ("Подмена на месте", [
        {"code": "A1", "name": "Табло",
         "note": "Пластинка вокзального табло с буквой T: слово перещёлкивается на месте, как на табло. Своё, не похоже "
                 "на переводчики; на 16 px остаётся квадрат с прорезью — крупный и узнаваемый в трее.",
         "tiles": [("A1", "", "a1-flap", False)]},
        {"code": "A2", "name": "Слово и каретка",
         "note": "Выделенное слово и текстовый курсор: прямо про «заменить набранное». Честно, но рискует читаться "
                 "как обычное поле ввода.",
         "tiles": [("A2", "", "a2-caret", False)]},
        {"code": "A3", "name": "Оборот слова",
         "note": "Слово-черта и две дуги вокруг: слово переворачивается на месте. Дуги разомкнуты, чтобы не было "
                 "«обновить», но на 16 px стрелки слипаются.",
         "tiles": [("A3", "", "a3-turn", False)]},
    ]),
    ("Вспышка", [
        {"code": "B1", "name": "Искры",
         "note": "Большая и малая искра — те же, что разлетаются при копировании. Красиво, но это общий значок «ИИ», "
                 "своего мало.",
         "tiles": [("B1", "", "b1-sparkle", False)]},
        {"code": "B2", "name": "Каретка-молния",
         "note": "Текстовый курсор, у которого ножка — молния: «ввод» и «мгновенно» в одной фигуре. Узкий — в трее "
                 "будет тонким столбиком.",
         "tiles": [("B2", "", "b2-bolt", False)]},
        {"code": "B3", "name": "Слово вспыхивает",
         "note": "Плашка слова и искра, прорывающая её край: момент подмены. Соединяет обе метафоры, на 16 px "
                 "читается как пилюля со звездой.",
         "tiles": [("B3", "", "b3-pill-spark", False)]},
    ]),
]

SUMMARY = ("Моё мнение: сильнейшие — A1 (своя метафора, крупный силуэт для трея) и B3 (подмена + вспышка в одной "
           "фигуре). B2 характерный, но узкий. B1 банален, A3 мелко рассыпается — их бы закрыл.")
