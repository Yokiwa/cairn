interface Props {
  checked: boolean;
  onChange(v: boolean): void;
  label: string;
  disabled?: boolean;
}

/** An accessible on/off switch (a real checkbox underneath). */
export default function Switch({ checked, onChange, label, disabled }: Props) {
  return (
    <label className="relative inline-flex cursor-pointer items-center">
      <input
        type="checkbox"
        role="switch"
        aria-label={label}
        className="peer sr-only"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className="h-6 w-10 rounded-full bg-line-strong transition peer-checked:bg-accent peer-focus-visible:outline-2 peer-focus-visible:outline-info" />
      <span className="absolute left-[3px] h-[18px] w-[18px] rounded-full bg-fg transition peer-checked:translate-x-4" />
    </label>
  );
}
