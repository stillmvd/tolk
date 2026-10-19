import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { checkNow, installNow, useShip } from "@stillmvd/tauri-ship";
import { Icon, IconName, Mark } from "./Icon";

type Config = {
  enabled: boolean;
  hotkey: string;
  area_hotkey: string;
  window_hotkey: string | null;
  mode: "online" | "offline";
  excluded: string[];
  autostart: boolean;
  history: boolean;
  repeat_ms: number;
};
type Model = { ready: boolean; busy: boolean; done: number; total: number };
type HkKey = "hotkey" | "area_hotkey" | "window_hotkey";
type Section = "keys" | "mode" | "ex" | "hist" | "gen";
type Entry = { t: number; from: string; to: string; src: string; dst: string };

const SECTIONS: [Section, string, IconName][] = [
  ["keys", "Клавиши", "kbd"],
  ["mode", "Перевод", "globe"],
  ["ex", "Исключения", "ban"],
  ["hist", "История", "clock"],
  ["gen", "Общие", "gear"],
];
const HOTKEYS: [HkKey, string][] = [
  ["hotkey", "Перевести слово или выделение"],
  ["area_hotkey", "Перевести область экрана"],
  ["window_hotkey", "Открыть переводчик"],
];
const GHBDTN = ["Pause", "Shift+Pause"];
const MODIFIERS = ["ControlLeft", "ControlRight", "AltLeft", "AltRight", "ShiftLeft", "ShiftRight", "MetaLeft", "MetaRight"];
const SPECIAL: Record<string, string> = { Pause: "Pause", Insert: "Insert", Space: "Space", Backquote: "`" };

const mb = (bytes: number) => Math.round(bytes / 1_048_576);

function keyName(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1\d|2[0-4])$/.test(code)) return code;
  return SPECIAL[code] ?? null;
}

function when(ms: number) {
  const d = new Date(ms);
  const today = d.toDateString() === new Date().toDateString();
  return today ? d.toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" }) : d.toLocaleDateString("ru-RU", { day: "2-digit", month: "2-digit" });
}

function ago(ms: number) {
  const m = Math.round((Date.now() - ms) / 60000);
  return m < 1 ? "только что" : m < 60 ? `${m} мин назад` : `в ${new Date(ms).toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" })}`;
}

function History({ on }: { on: boolean }) {
  const [items, setItems] = useState<Entry[]>([]);
  const [q, setQ] = useState("");
  const [copied, setCopied] = useState<number | null>(null);
  const load = () => invoke<Entry[]>("history_list").then(setItems);

  useEffect(() => {
    load();
    const off = getCurrentWindow().onFocusChanged(({ payload }) => payload && load());
    return () => {
      off.then((f) => f());
    };
  }, [on]);

  const needle = q.trim().toLowerCase();
  const shown = needle ? items.filter((e) => `${e.src}\n${e.dst}`.toLowerCase().includes(needle)) : items;
  const copy = async (e: Entry) => {
    await invoke("copy_text", { text: e.dst });
    setCopied(e.t);
  };
  const clear = async () => {
    await invoke("history_clear");
    load();
  };

  if (!items.length) return <p className="tk-empty">{on ? "Здесь появятся твои переводы" : "История выключена"}</p>;
  return (
    <>
      <div className="tk-hbar">
        <label className="t-field tk-hsearch">
          <Icon name="search" />
          <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Найти в истории" spellCheck={false} />
        </label>
        <button type="button" className="t-btn s32" onClick={clear}>
          Очистить
        </button>
      </div>
      <div className="tk-hl">
        {shown.map((e) => (
          <button type="button" className="tk-hr" key={e.t} onClick={() => copy(e)} title="Скопировать перевод">
            <span className="src">{e.src}</span>
            <span className="dst">{e.dst}</span>
            <span className="at">{copied === e.t ? "Скопировано" : when(e.t)}</span>
          </button>
        ))}
        {!shown.length && <p className="tk-empty">Ничего не нашлось</p>}
      </div>
    </>
  );
}

function Updates() {
  const { status } = useShip();
  const [checking, setChecking] = useState(false);
  const phase = status?.phase ?? "idle";
  const busy = checking || phase === "checking" || phase === "downloading" || phase === "installing";
  const sub = status?.error
    ? status.error
    : phase === "ready"
      ? `Готова версия ${status?.available?.version ?? ""}`
      : phase === "downloading"
        ? "Скачиваю обновление"
        : busy
          ? "Проверяю"
          : status?.lastCheck
            ? `Последняя версия · проверено ${ago(status.lastCheck)}`
            : "";
  const check = async () => {
    setChecking(true);
    try {
      await checkNow();
    } finally {
      setChecking(false);
    }
  };
  return (
    <div className="t-setrow tk-btnrow">
      <div className="tx">
        <b>Обновления{status?.current ? ` · версия ${status.current}` : ""}</b>
        {sub && <span>{sub}</span>}
      </div>
      {phase === "ready" ? (
        <button type="button" className="t-btn s32 p" onClick={() => installNow()}>
          Перезапустить
        </button>
      ) : (
        <button type="button" className="t-btn s32" disabled={busy} onClick={check}>
          Проверить
        </button>
      )}
    </div>
  );
}

export function Settings() {
  const win = getCurrentWindow();
  const [cfg, setCfg] = useState<Config | null>(null);
  const [section, setSection] = useState<Section>("keys");
  const [rec, setRec] = useState<HkKey | null>(null);
  const [msg, setMsg] = useState<{ key: string; text: string; err: boolean } | null>(null);

  const [model, setModel] = useState<Model | null>(null);

  useEffect(() => {
    invoke<Config>("get_config").then(setCfg);
    invoke<Model>("model_status").then(setModel);
    const install = () =>
      invoke<boolean>("take_install").then((go) => {
        if (go) installNow();
      });
    install();
    const off = [
      listen("tolk:install", install),
      listen<Config>("config:changed", (e) => setCfg(e.payload)),
      listen<Model>("model:progress", (e) => setModel(e.payload)),
      listen<string | null>("model:done", (e) => e.payload && setMsg({ key: "model", text: e.payload, err: true })),
    ];
    return () => {
      off.forEach((p) => p.then((f) => f()));
    };
  }, []);

  const removeModel = async () => {
    try {
      await invoke("delete_model");
    } catch (e) {
      setMsg({ key: "model", text: String(e), err: true });
    }
  };

  const latest = useRef<Config | null>(null);
  latest.current = cfg;

  const save = async (patch: Partial<Config>, key = "") => {
    if (!latest.current) return false;
    const next = { ...latest.current, ...patch };
    try {
      await invoke("set_config", { cfg: next });
      latest.current = next;
      setCfg(next);
      return true;
    } catch (e) {
      setMsg({ key, text: String(e), err: true });
      return false;
    }
  };

  const stopRec = () => {
    setRec(null);
    invoke("set_recording", { on: false });
  };

  useEffect(() => {
    if (!rec || !cfg) return;
    const onKey = async (e: KeyboardEvent) => {
      e.preventDefault();
      if (e.repeat || MODIFIERS.includes(e.code)) return;
      if (e.code === "Escape") return stopRec();
      if (e.code === "Backspace" && rec === "window_hotkey") {
        stopRec();
        setMsg(null);
        return save({ window_hotkey: null }, rec);
      }
      const key = keyName(e.code);
      if (!key) return setMsg({ key: rec, text: "Эту клавишу нельзя назначить", err: true });
      const combo = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Win", key].filter(Boolean).join("+");
      const bare = !e.ctrlKey && !e.altKey && !e.metaKey && !/^(F\d+|Pause|Insert)$/.test(key);
      if (bare) return setMsg({ key: rec, text: "Добавь Ctrl, Alt или Win", err: true });
      if (GHBDTN.includes(combo)) return setMsg({ key: rec, text: "Это сочетание у Ghbdtn", err: true });
      const twin = HOTKEYS.find(([k]) => k !== rec && cfg[k] === combo);
      if (twin) return setMsg({ key: rec, text: `Уже занято: ${twin[1].toLowerCase()}`, err: true });
      const target = rec;
      stopRec();
      const free = await invoke<boolean>("hotkey_free", { combo });
      if ((await save({ [target]: combo }, target)) && !free) {
        setMsg({ key: target, text: "Это сочетание занято другой программой — она перестанет его получать", err: false });
      } else if (free) setMsg(null);
    };
    addEventListener("keydown", onKey);
    const offBlur = win.onFocusChanged(({ payload }) => !payload && stopRec());
    return () => {
      removeEventListener("keydown", onKey);
      offBlur.then((f) => f());
    };
  }, [rec, cfg]);

  const startRec = (key: HkKey) => {
    setMsg(null);
    setRec(key);
    invoke("set_recording", { on: true });
  };

  const addProgram = async () => {
    if (!cfg) return;
    const name = await invoke<string | null>("pick_program");
    const excluded = latest.current?.excluded ?? [];
    if (name && !excluded.includes(name)) save({ excluded: [...excluded, name] }, "ex");
  };

  if (!cfg) return null;

  const note = (key: string) => msg?.key === key && <p className={`t-field-msg${msg.err ? " err" : ""}`}>{msg.text}</p>;

  return (
    <div className="tk-set">
      <div className="t-titlebar" data-tauri-drag-region>
        <Mark />
        <span className="name" data-tauri-drag-region>
          Настройки
        </span>
        <span className="tk-sp" data-tauri-drag-region />
        <div className="t-win">
          <button type="button" onClick={() => win.minimize()} aria-label="Свернуть">
            <Icon name="min" />
          </button>
          <button type="button" className="close" onClick={() => win.close()} aria-label="Закрыть">
            <Icon name="x" />
          </button>
        </div>
      </div>
      <div className="tk-split">
        <nav className="t-list tk-nav">
          {SECTIONS.map(([key, name, icon]) => (
            <button type="button" key={key} className={`t-row${section === key ? " on" : ""}`} aria-current={section === key ? "page" : undefined} onClick={() => setSection(key)}>
              <Icon name={icon} />
              {name}
            </button>
          ))}
        </nav>
        <main className="tk-pg">
          {section === "keys" &&
            HOTKEYS.map(([key, name]) => (
              <div className="tk-item" key={key}>
                <div className="t-setrow tk-btnrow">
                  <div className="tx">
                    <b>{name}</b>
                  </div>
                  <span className={`tk-hk${rec !== key && !cfg[key] ? " no" : ""}`}>{rec === key ? "Нажми сочетание…" : cfg[key] || "Не задано"}</span>
                  <button type="button" className="t-btn s32" onClick={() => (rec === key ? stopRec() : startRec(key))}>
                    {rec === key ? "Отменить" : "Изменить"}
                  </button>
                </div>
                {note(key)}
              </div>
            ))}
          {section === "mode" && (
            <>
              <div className="t-setrow tk-segrow">
                <div className="tx">
                  <b>Переводить</b>
                </div>
                <div className="t-seg" role="group" aria-label="Режим">
                  <button type="button" aria-pressed={cfg.mode === "online"} onClick={() => save({ mode: "online" }, "mode")}>
                    Онлайн
                  </button>
                  <button type="button" aria-pressed={cfg.mode === "offline"} onClick={() => save({ mode: "offline" }, "mode")}>
                    Только офлайн
                  </button>
                </div>
              </div>
              <div className="t-setrow tk-btnrow">
                <div className="tx">
                  <b>Офлайн-модель</b>
                  {model?.busy ? (
                    <>
                      <div className="t-progress tk-prog">
                        <i style={{ width: `${(model.done / model.total) * 100}%` }} />
                      </div>
                      <span>
                        {mb(model.done)} из {mb(model.total)} МБ
                      </span>
                    </>
                  ) : (
                    <span>{model?.ready ? `Скачана · ${mb(model.total)} МБ — переводит без сети` : `≈ ${mb(model?.total ?? 0)} МБ, русский и английский`}</span>
                  )}
                </div>
                {model?.busy ? (
                  <button type="button" className="t-btn s32" onClick={() => invoke("cancel_download")}>
                    Отменить
                  </button>
                ) : model?.ready ? (
                  <button type="button" className="t-btn s32" onClick={removeModel}>
                    Удалить
                  </button>
                ) : (
                  <button type="button" className="t-btn s32" disabled={!model} onClick={() => (setMsg(null), invoke("download_model"))}>
                    Скачать
                  </button>
                )}
              </div>
              {note("model")}
              {note("mode")}
            </>
          )}
          {section === "ex" && (
            <>
              <div className="tk-exl">
                {cfg.excluded.map((name) => (
                  <div className="tk-exr" key={name}>
                    <i className="tk-app">{name[0]?.toUpperCase()}</i>
                    <span className="nm">{name}</span>
                    <button type="button" className="t-btn g ib s32" aria-label={`Убрать ${name}`} onClick={() => save({ excluded: cfg.excluded.filter((e) => e !== name) }, "ex")}>
                      <Icon name="x" />
                    </button>
                  </div>
                ))}
              </div>
              <div className="tk-add">
                <button type="button" className="t-btn s32" onClick={addProgram}>
                  <Icon name="plus" />
                  Добавить программу
                </button>
              </div>
              {note("ex")}
            </>
          )}
          {section === "hist" && (
            <>
              <div className="t-setrow">
                <div className="tx">
                  <b>Хранить историю переводов</b>
                  <span>Только на этом компьютере, до 500 записей. Выключение стирает историю</span>
                </div>
                <button type="button" className="t-tg" role="switch" aria-checked={cfg.history} aria-label="Хранить историю переводов" onClick={() => save({ history: !cfg.history }, "hist")} />
              </div>
              {note("hist")}
              <History on={cfg.history} />
            </>
          )}
          {section === "gen" && (
            <>
              <div className="t-setrow">
                <div className="tx">
                  <b>Запускать вместе с Windows</b>
                </div>
                <button type="button" className="t-tg" role="switch" aria-checked={cfg.autostart} aria-label="Запускать вместе с Windows" onClick={() => save({ autostart: !cfg.autostart }, "gen")} />
              </div>
              <Updates />
              {note("gen")}
            </>
          )}
        </main>
      </div>
    </div>
  );
}
