import math

import wave1
from wave1 import glyph_svg, line, rrect, shape, sparkle


def round_poly(verts, radii, steps=12):
    pts = []
    n = len(verts)
    for i, (p, r) in enumerate(zip(verts, radii)):
        a, b = verts[i - 1], verts[(i + 1) % n]
        if not r:
            pts.append(p)
            continue
        da = math.hypot(a[0] - p[0], a[1] - p[1])
        db = math.hypot(b[0] - p[0], b[1] - p[1])
        s = (p[0] + (a[0] - p[0]) * r / da, p[1] + (a[1] - p[1]) * r / da)
        e = (p[0] + (b[0] - p[0]) * r / db, p[1] + (b[1] - p[1]) * r / db)
        for k in range(steps + 1):
            t = k / steps
            pts.append(((1 - t) ** 2 * s[0] + 2 * (1 - t) * t * p[0] + t * t * e[0],
                        (1 - t) ** 2 * s[1] + 2 * (1 - t) * t * p[1] + t * t * e[1]))
    return pts


def falling_flap():
    top = round_poly([(-3.5, 3), (43.5, 3), (40, 18.2), (0, 18.2)], [7, 7, 1, 1])
    bottom = rrect(0, 21.8, 40, 18.2, 7)
    return [shape(top), shape(bottom),
            line([(7.5, 9.6), (32.5, 9.6)], 6, "paper", cap="butt"),
            line([(20, 9.6), (20, 18.2)], 6, "paper", cap="butt"),
            line([(20, 21.8), (20, 32.5)], 6, "paper", cap="butt")]


def tall_flap():
    gap = 3.4
    return [shape(rrect(0, 0, 30, 21 - gap / 2, 6)), shape(rrect(0, 21 + gap / 2, 30, 21 - gap / 2, 6)),
            line([(6.5, 10), (23.5, 10)], 5.6, "paper", cap="butt"),
            line([(15, 10), (15, 34)], 5.6, "paper", cap="butt"),
            line([(-2, 21), (32, 21)], gap, "paper", cap="butt")]


def corner_spark():
    return [shape(rrect(0, 8, 36, 20, 10)), shape(sparkle(35, 6, 14), halo=5)]


def half_word():
    return [line(rrect(0, 0, 44, 20, 10), 3.6, closed=True),
            shape(round_poly([(22, 0), (44, 0), (44, 20), (22, 20)], [0, 10, 10, 0])),
            shape(sparkle(22, 10, 15), halo=2.6)]


VARIANTS = {
    "a1-falling": falling_flap,
    "a1-tall": tall_flap,
    "b3-corner": corner_spark,
    "b3-half": half_word,
}


def variant_svg(cid, key, in_circle):
    if key not in VARIANTS:
        return wave1.variant_svg(cid, key, in_circle)
    return glyph_svg(VARIANTS[key](), 56)


NUMBER = 2
TITLE = "Волна 2 · композиция"
PROMPT = ("Меняется только композиция — толщину и пропорции деталей доводим дальше. В карточке «было» и два "
          "«стало». Отметь, что оставить, можно несколько.")
CHOSEN = ["A1", "B3″"]

CARDS = [
    ("Табло", [
        {"code": "A1", "name": "Пластинка табло",
         "note": "A1′ — верхняя пластинка падает на зрителя (шире и короче нижней): видно движение, щелчок табло. "
                 "A1″ — высокая пластинка, как цифра на настоящем табло: стройнее, но в трее уже.",
         "tiles": [("A1", "было", "a1-flap", False), ("A1′", "падает", "a1-falling", False),
                   ("A1″", "высокая", "a1-tall", False)]},
    ]),
    ("Слово вспыхивает", [
        {"code": "B3", "name": "Плашка и искра",
         "note": "B3′ — искра вырывается из угла плашки: читается чище, но ближе к «новому уведомлению». "
                 "B3″ — слово наполовину старое (контур) и наполовину новое (заливка), искра на границе — сама подмена.",
         "tiles": [("B3", "было", "b3-pill-spark", False), ("B3′", "из угла", "b3-corner", False),
                   ("B3″", "полслова", "b3-half", False)]},
    ]),
]

SUMMARY = ("Моё мнение: A1′ — лучший из табло, движение без лишних деталей и прежний крупный силуэт. "
           "B3″ рассказывает подмену точнее всех, но контур на 16 px тонкий — если выберешь, в трее дам "
           "упрощённую версию. B3′ я бы закрыл: похоже на значок уведомления.")
