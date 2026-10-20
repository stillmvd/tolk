import { MARK_BOTTOM, MARK_TOP } from "./mark";

const PATHS = {
  copy: "M11.5 8.5h5a3 3 0 0 1 3 3v5a3 3 0 0 1-3 3h-5a3 3 0 0 1-3-3v-5a3 3 0 0 1 3-3ZM15.5 8.5V7a3 3 0 0 0-3-3H7a3 3 0 0 0-3 3v5.5a3 3 0 0 0 3 3h1.5",
  check: "M5 12.5 10 17.5 19 7",
  win: "M7 5h10a3.5 3.5 0 0 1 3.5 3.5v7A3.5 3.5 0 0 1 17 19H7a3.5 3.5 0 0 1-3.5-3.5v-7A3.5 3.5 0 0 1 7 5ZM3.5 9.5h17",
  swap: "M4 8h15M15.5 4.5 19 8l-3.5 3.5M20 16H5M8.5 12.5 5 16l3.5 3.5",
  alert: "M10.3 4.6 3.6 16.5A2 2 0 0 0 5.3 19.5h13.4a2 2 0 0 0 1.7-3L13.7 4.6a2 2 0 0 0-3.4 0ZM12 9.5v4M12 16.6v.01",
  x: "M18 6 6 18M6 6l12 12",
  plus: "M12 5v14M5 12h14",
  min: "M5 12h14",
  kbd: "M4 5h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2ZM6 9h.01M10 9h.01M14 9h.01M18 9h.01M8 13h.01M12 13h.01M16 13h.01M7 16h10",
  globe: "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0ZM3 12h18M12 3c2.5 2.7 3.6 5.7 3.6 9s-1.1 6.3-3.6 9c-2.5-2.7-3.6-5.7-3.6-9S9.5 5.7 12 3Z",
  ban: "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0ZM5.6 5.6l12.8 12.8",
  gear: "M20 7h-9M14 17H5M14 17a3 3 0 1 0 6 0a3 3 0 1 0-6 0ZM4 7a3 3 0 1 0 6 0a3 3 0 1 0-6 0Z",
  clock: "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0ZM12 7.5V12l3 2",
  search: "M4 11a7 7 0 1 0 14 0a7 7 0 1 0-14 0ZM20 20l-3.6-3.6",
};

export type IconName = keyof typeof PATHS;

export function Icon({ name }: { name: IconName }) {
  return (
    <svg className="tk-ic" viewBox="0 0 24 24" aria-hidden="true">
      <path d={PATHS[name]} fill="none" stroke="currentColor" strokeWidth={name === "check" ? 2.2 : 1.6} strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function Mark() {
  return (
    <svg className="tk-mark" viewBox="0 0 16 16" aria-hidden="true" data-tauri-drag-region>
      <path className="a" d={MARK_TOP} />
      <path className="b" d={MARK_BOTTOM} />
    </svg>
  );
}
