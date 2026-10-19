import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { Icon } from "./Icon";

type Out = { text: string; variants: string[]; from: string; to: string };
type View = { kind: "idle" } | { kind: "done"; out: Out } | { kind: "err"; msg: string };

type Spark = { dx: number; dy: number; s: number; d: number; c: string };
type Burst = { id: number; x: number; y: number; parts: Spark[] };

const TINTS = ["#fff", "var(--tk)", "#fff", "color-mix(in srgb, var(--tk) 45%, white)"];
const sparks = (): Spark[] =>
  Array.from({ length: 18 }, (_, i) => {
    const a = (i / 18) * Math.PI * 2 + (Math.random() - 0.5) * 0.5;
    const r = 34 + Math.random() * 46;
    return { dx: Math.cos(a) * r, dy: Math.sin(a) * r, s: 6 + Math.random() * 8, d: Math.random() * 70, c: TINTS[i % 4] };
  });

const guess =(t: string) => (/[а-яё]/i.test(t) || !/[a-z]/i.test(t) ? "ru" : "en");
const other = (l: string) => (l === "ru" ? "en" : "ru");

export function Translator() {
  const win = getCurrentWindow();
  const [text, setText] = useState("");
  const [flip, setFlip] = useState(false);
  const [view, setView] = useState<View>({ kind: "idle" });
  const [copied, setCopied] = useState(false);
  const [stale, setStale] = useState(false);
  const [busy, setBusy] = useState(false);
  const [burst, setBurst] = useState<Burst | null>(null);
  const input = useRef<HTMLTextAreaElement>(null);
  const wrap = useRef<HTMLDivElement>(null);
  const btn = useRef<HTMLButtonElement>(null);
  const seq = useRef(0);
  const shown = useRef(false);
  const leaving = useRef(0);

  const playOpen = () => {
    const el = wrap.current;
    if (!el) return;
    el.classList.remove("opening");
    void el.offsetWidth;
    el.classList.add("live", "opening");
  };

  const close = (restore: boolean) => {
    clearTimeout(leaving.current);
    leaving.current = 0;
    wrap.current?.classList.remove("live", "opening");
    requestAnimationFrame(() =>
      requestAnimationFrame(() =>
        invoke("hide_translator", { restore }).then(() => {
          setText("");
          setFlip(false);
          setBurst(null);
        }),
      ),
    );
  };

  useEffect(() => {
    invoke<string | null>("translator_text").then((t) => t && setText(t));
    const off = [
      listen<string>("translator:fill", (e) => {
        setText(e.payload);
        setFlip(false);
      }),
      listen("translator:show", () => {
        playOpen();
        input.current?.focus();
        input.current?.select();
      }),
      win.onFocusChanged(({ payload }) => {
        if (!payload) close(false);
      }),
    ];
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") close(true);
    };
    addEventListener("keydown", key);
    return () => {
      removeEventListener("keydown", key);
      off.forEach((p) => p.then((f) => f()));
    };
  }, []);

  useEffect(() => {
    const id = ++seq.current;
    setCopied(false);
    if (!text.trim()) {
      setView({ kind: "idle" });
      setFlip(false);
      setStale(false);
      setBusy(false);
      return;
    }
    setStale(true);
    let slow = 0;
    const start = window.setTimeout(() => {
      slow = window.setTimeout(() => id === seq.current && setBusy(true), 150);
      invoke<Out | null>("translate_text", { text, flip })
        .then((out) => id === seq.current && setView(out ? { kind: "done", out } : { kind: "idle" }))
        .catch((msg) => id === seq.current && setView({ kind: "err", msg: String(msg) }))
        .finally(() => {
          clearTimeout(slow);
          if (id === seq.current) {
            setStale(false);
            setBusy(false);
          }
        });
    }, 400);
    return () => {
      clearTimeout(start);
      clearTimeout(slow);
    };
  }, [text, flip]);

  useLayoutEffect(() => {
    const el = wrap.current;
    if (!el) return;
    win.setSize(new LogicalSize(el.offsetWidth, el.offsetHeight)).then(async () => {
      if (shown.current) return;
      shown.current = true;
      playOpen();
      await win.show();
      await win.setFocus();
      input.current?.focus();
    });
  }, [view, copied, text]);

  const from = view.kind === "done" ? view.out.from : flip ? other(guess(text)) : guess(text);
  const result = view.kind === "done" ? view.out.text : "";
  const word = view.kind === "done" && view.out.variants.length > 0;
  const variants = view.kind === "done" ? view.out.variants.filter((v) => v !== result) : [];

  const copy = async (value: string, el: HTMLElement | null) => {
    if (!value || stale || leaving.current) return;
    leaving.current = window.setTimeout(() => close(true), 650);
    await invoke("copy_text", { text: value });
    if (view.kind === "done") invoke("remember", { src: text, dst: value, from: view.out.from, to: view.out.to });
    setCopied(true);
    const box = wrap.current?.getBoundingClientRect();
    const at = el?.getBoundingClientRect();
    if (box && at) setBurst({ id: Date.now(), x: at.left + at.width / 2 - box.left, y: at.top + at.height / 2 - box.top, parts: sparks() });
  };

  const words = result.split(/(\s+)/).map((w, i) =>
    i % 2 ? (
      w
    ) : (
      <span className="w" key={i} style={{ animationDelay: `${Math.floor(i / 2) * 60}ms` }}>
        {w}
      </span>
    ),
  );

  return (
    <div className="tk-tr-wrap" ref={wrap} onMouseDown={(e) => e.target === e.currentTarget && close(true)}>
      <div className={`tk-tw${busy ? " waiting" : ""}`}>
        <div className="tk-halo" aria-hidden="true">
          <i />
          <i />
        </div>
        <div className="tk-tc">
        <label className="row in">
          <span className="code">{from.toUpperCase()}</span>
          <textarea
            ref={input}
            rows={1}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key !== "Enter" || e.shiftKey || e.nativeEvent.isComposing) return;
              e.preventDefault();
              copy(result, btn.current);
            }}
            placeholder="Введи или вставь текст"
            spellCheck={false}
            autoFocus
          />
          <button type="button" className="t-btn g ib s32" onClick={() => setFlip(!flip)} disabled={!text.trim()} aria-label="Поменять направление">
            <Icon name="swap" />
          </button>
        </label>
        {(view.kind === "done" || view.kind === "err") && (
          <div className={`row out${stale && view.kind === "done" ? " stale" : ""}`}>
            <span className="code">{other(from).toUpperCase()}</span>
            <div className="bd">
              {view.kind === "err" && (
                <span className="tk-err">
                  <i className="ico">
                    <Icon name="alert" />
                  </i>
                  {view.msg}
                </span>
              )}
              {view.kind === "done" && (
                <>
                  <p className={word ? "tk-main" : "tk-phr"} key={`m-${result}`} onClick={(e) => copy(result, e.currentTarget)}>
                    {words}
                  </p>
                  {variants.length > 0 && (
                    <div className="tk-vs" key={`v-${result}`}>
                      {variants.map((v, i) => (
                        <button type="button" key={v} className="t-chip" style={{ animationDelay: `${180 + i * 50}ms` }} onClick={(e) => copy(v, e.currentTarget)}>
                          {v}
                        </button>
                      ))}
                    </div>
                  )}
                </>
              )}
            </div>
            {view.kind === "done" && (
              <div className="ft">
                <button type="button" className="t-btn s32 p busy-able" data-state={copied ? "done" : "idle"} ref={btn} onClick={(e) => copy(result, e.currentTarget)}>
                  <span className="s-idle">
                    <Icon name="copy" />
                    Скопировать
                  </span>
                  <span className="s-done">
                    <Icon name="check" />
                    Скопировано
                  </span>
                </button>
              </div>
            )}
          </div>
        )}
        </div>
      </div>
      {burst && (
        <div className="tk-sparks" key={burst.id} style={{ left: burst.x, top: burst.y }} aria-hidden="true">
          {burst.parts.map((p, i) => (
            <i key={i} style={{ "--dx": `${p.dx}px`, "--dy": `${p.dy}px`, "--s": `${p.s}px`, "--c": p.c, animationDelay: `${p.d}ms` } as CSSProperties} />
          ))}
        </div>
      )}
    </div>
  );
}
