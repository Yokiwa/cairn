import type { ButtonHTMLAttributes } from "react";

type Variant = "primary" | "default" | "ghost" | "danger";

const styles: Record<Variant, string> = {
  primary: "bg-accent border-accent text-accent-fg hover:brightness-110",
  default: "bg-panel-2 border-line-strong text-fg hover:bg-line",
  ghost: "bg-transparent border-transparent text-muted hover:bg-panel-2 hover:text-fg",
  danger: "bg-transparent border-line-strong text-danger hover:bg-panel-2",
};

export default function Button({ variant = "default", className = "", type = "button", ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant }) {
  return (
    <button
      type={type}
      className={`inline-flex min-h-11 items-center justify-center gap-2 rounded-[10px] border px-4 text-sm font-semibold transition active:scale-[.98] disabled:cursor-not-allowed disabled:opacity-50 ${styles[variant]} ${className}`}
      {...rest}
    />
  );
}
