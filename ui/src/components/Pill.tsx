import type { ButtonHTMLAttributes } from "react";

export default function Pill({ on, className = "", ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { on: boolean }) {
  return (
    <button
      type="button"
      aria-pressed={on}
      className={`min-h-10 rounded-full border px-4 text-[13px] font-semibold transition ${on ? "border-fg bg-fg text-bg" : "border-line-strong text-fg hover:border-muted"} ${className}`}
      {...rest}
    />
  );
}
