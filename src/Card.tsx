import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { currentMonitor, getCurrentWindow, LogicalSize, PhysicalPosition } from "@tauri-apps/api/window";
import { Icon } from "./Icon";

type CardData = { original: string; translation: string; above: number };

export function Card() {
  const [data, setData] = useState<CardData | null>(null);
  const [done, setDone] = useState(false);
  const wrap = useRef<HTMLDivElement>(null);
  const win = getCurrentWindow();

  useEffect(() => {
    invoke<CardData>("card_data").then(setData);
    const close = () => win.close();
    const key = (e: KeyboardEvent) => e.key === "Escape" && close();
    addEventListener("keydown", key);
    const off = win.onFocusChanged(({ payload }) => !payload && close());
    return () => {
      removeEventListener("keydown", key);
      off.then((f) => f());
    };
  }, []);

  useLayoutEffect(() => {
    const el = wrap.current;
    if (!data || !el) return;
    (async () => {
      await win.setSize(new LogicalSize(el.offsetWidth, el.offsetHeight));
      const [pos, size, mon] = await Promise.all([win.outerPosition(), win.outerSize(), currentMonitor()]);
      if (mon) {
        const { position: wa, size: ws } = mon.workArea;
        let { x, y } = pos;
        if (y + size.height > wa.y + ws.height) y = data.above - size.height + Math.round(40 * mon.scaleFactor);
        x = Math.max(wa.x, Math.min(x, wa.x + ws.width - size.width));
        y = Math.max(wa.y, Math.min(y, wa.y + ws.height - size.height));
        if (x !== pos.x || y !== pos.y) await win.setPosition(new PhysicalPosition(x, y));
      }
      await win.show();
      await win.setFocus();
    })();
  }, [data]);

  const copy = async () => {
    if (!data) return;
    await invoke("copy_text", { text: data.translation });
    setDone(true);
  };

  if (!data) return null;
  return (
    <div className="tk-card-wrap" ref={wrap}>
      <div className="tk-card">
        <p className="tx" onClick={copy}>{data.translation}</p>
        <div className="col">
          <button className="t-btn busy-able s32 p" data-state={done ? "done" : "idle"} onClick={copy}>
            <span className="s-idle"><Icon name="copy" />Скопировать</span>
            <span className="s-done"><Icon name="check" />Скопировано</span>
          </button>
          <button className="t-btn s32" onClick={() => invoke("open_translator_with", { text: data.original }).then(() => win.close())}>
            <Icon name="win" />В окно
          </button>
        </div>
      </div>
    </div>
  );
}
