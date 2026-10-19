const PATHS: Record<string, string> = {
  copy: "M11.5 8.5h5a3 3 0 0 1 3 3v5a3 3 0 0 1-3 3h-5a3 3 0 0 1-3-3v-5a3 3 0 0 1 3-3ZM15.5 8.5V7a3 3 0 0 0-3-3H7a3 3 0 0 0-3 3v5.5a3 3 0 0 0 3 3h1.5",
  check: "M5 12.5 10 17.5 19 7",
  win: "M7 5h10a3.5 3.5 0 0 1 3.5 3.5v7A3.5 3.5 0 0 1 17 19H7a3.5 3.5 0 0 1-3.5-3.5v-7A3.5 3.5 0 0 1 7 5ZM3.5 9.5h17",
};

export function Icon({ name }: { name: keyof typeof PATHS }) {
  return (
    <svg className="tk-ic" viewBox="0 0 24 24" aria-hidden="true">
      <path d={PATHS[name]} fill="none" stroke="currentColor" strokeWidth={name === "check" ? 2.2 : 1.6} strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
