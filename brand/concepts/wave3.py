import wave2
from wave1 import glyph_svg, line, rrect, shape, sparkle
from wave2 import round_poly


def flap(w=40, h=40, r=7, gap=3.6, t=6, bar=0.5, stem=0.8):
    mid = h / 2
    cx = w / 2
    ty = h * 0.25
    half = w * bar / 2
    return [shape(rrect(0, 0, w, mid - gap / 2, r)), shape(rrect(0, mid + gap / 2, w, mid - gap / 2, r)),
            line([(cx - half, ty), (cx + half, ty)], t, "paper", cap="butt"),
            line([(cx, ty), (cx, h * stem)], t, "paper", cap="butt"),
            line([(-2, mid), (w + 2, mid)], gap, "paper", cap="butt")]


def half_word(w=44, h=20, o=3.6, s=15, halo=2.6, split=0.5):
    x = w * split
    return [line(rrect(0, 0, w, h, h / 2), o, closed=True),
            shape(round_poly([(x, 0), (w, 0), (w, h), (x, h)], [0, h / 2, h / 2, 0])),
            shape(sparkle(x, h / 2, s), halo=halo)]


VARIANTS = {
    "a1-light": lambda: flap(t=4.8, gap=3),
    "a1-bold": lambda: flap(r=9, gap=4.4, t=7.6, bar=0.56),
    "a1-wide": lambda: flap(w=44, h=38, r=8, gap=4, t=6.6, bar=0.6),
    "b3-bold": lambda: half_word(o=5, s=15, halo=3),
    "b3-big": lambda: half_word(w=40, s=19, halo=3, split=0.46),
    "b3-tall": lambda: half_word(w=40, h=24, o=4.6, s=17, halo=3),
}


def variant_svg(cid, key, in_circle):
    if key not in VARIANTS:
        return wave2.variant_svg(cid, key, in_circle)
    return glyph_svg(VARIANTS[key](), 56)


NUMBER = 3
TITLE = "Волна 3 · толщина и пропорции"
PROMPT = ("Композиции зафиксированы, меняются толщины и пропорции. Главный судья — ряд 16 px: в трее знак "
          "должен быть крупным и не слипаться. Отметь по одному в каждой карточке или оба фаворита.")
CHOSEN = ["A1"]

CARDS = [
    ("Табло A1", [
        {"code": "A1", "name": "Пластинка с T",
         "note": "a — тонкая T и узкая щель: изящнее, но на 16 px щель пропадает. b — жирная T, широкая щель, "
                 "круглее углы: самый крепкий на 16 px. c — пластинка шире и ниже, T шире: спокойнее, ближе к табло.",
         "tiles": [("A1", "было", "a1-flap", False), ("A1a", "тоньше", "a1-light", False),
                   ("A1b", "жирнее", "a1-bold", False), ("A1c", "шире", "a1-wide", False)]},
    ]),
    ("Полслова B3″", [
        {"code": "B3″", "name": "Контур, заливка, искра",
         "note": "a — контур толще: старое слово не теряется на 16 px. b — искра крупнее, выходит за плашку — "
                 "акцент на вспышке. c — плашка выше и короче: компактнее, лучше вписывается в квадрат иконки.",
         "tiles": [("B3″", "было", "b3-half", False), ("B3″a", "контур толще", "b3-bold", False),
                   ("B3″b", "искра крупнее", "b3-big", False), ("B3″c", "выше", "b3-tall", False)]},
    ]),
]

SUMMARY = ("Моё мнение: A1b — для трея лучший, держится на 16 px как значки соседей. Из полуслова — B3″c: "
           "квадратнее, контур не тает. Следующая волна — цвет (сирень Tolk), подложка и светлая/тёмная тема "
           "для того, что выберешь.")
