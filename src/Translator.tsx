import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { Icon } from "./Icon";

type Out = { text: string; variants: string[]; from: string; to: string };
type View = { kind: "idle" } | { kind: "done"; out: Out } | { kind: "err"; msg: string };

const guess = (t: string) => (/[а-яё]/i.test(t) || !/[a-z]/i.test(t) ? "ru" : "en");
const other = (l: string) => (l === "ru" ? "en" : "ru");

export function Translator() {
  const win = getCurrentWindow();
  const [text, setText] = useState("");
  const [flip, setFlip] = useState(false);
  const [view, setView] = useState<View>({ kind: "idle" });
  const [pick, setPick] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [stale, setStale] = useState(false);
  const [busy, setBusy] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const wrap = useRef<HTMLDivElement>(null);
  const seq = useRef(0);
  const shown = useRef(false);

  const playOpen = () => {
    const el = wrap.current;
    if (!el) return;
    el.classList.remove("opening");
    void el.offsetWidth;
    el.classList.add("opening");
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
        if (!payload) invoke("hide_translator", { restore: false });
      }),
    ];
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") invoke("hide_translator", { restore: true });
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
    setPick(null);
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
  }, [view, pick, copied]);

  const from = view.kind === "done" ? view.out.from : flip ? other(guess(text)) : guess(text);
  const result = view.kind === "done" ? pick ?? view.out.text : "";
  const word = view.kind === "done" && view.out.variants.length > 0;
  const variants = view.kind === "done" ? view.out.variants.filter((v) => v !== result) : [];

  const copy = async () => {
    if (!result || stale) return;
    await invoke("copy_text", { text: result });
    setCopied(true);
  };

  const words = result.split(/(\s+)/).map((w, i) => (
    <span className="w" key={i} style={{ animationDelay: `${Math.floor(i / 2) * 60}ms` }}>
      {w}
    </span>
  ));

  return (
    <div className="tk-tr-wrap" ref={wrap} onMouseDown={(e) => e.target === e.currentTarget && invoke("hide_translator", { restore: true })}>
      <div className={`tk-tw${busy ? " waiting" : ""}`}>
        <div className="tk-halo" aria-hidden="true">
          <i />
          <i />
        </div>
        <div className="tk-tc">
        <label className="row in">
          <span className="code">{from.toUpperCase()}</span>
          <input
            ref={input}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && copy()}
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
                  <p className={word ? "tk-main" : "tk-phr"} key={`m-${result}`} onClick={copy}>
                    {words}
                  </p>
                  {variants.length > 0 && (
                    <div className="tk-vs" key={`v-${result}`}>
                      {variants.map((v, i) => (
                        <button type="button" key={v} className="t-chip" style={{ animationDelay: `${180 + i * 50}ms` }} onClick={() => (setPick(v), setCopied(false))}>
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
                <button type="button" className="t-btn s32 p busy-able" data-state={copied ? "done" : "idle"} onClick={copy}>
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
    </div>
  );
}
