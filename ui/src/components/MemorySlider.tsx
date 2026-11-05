import { useId } from "react";
import { useTranslation } from "react-i18next";

interface Props {
  label: string;
  value: number;
  onChange(mb: number): void;
  totalMb: number;
  recommendedMb: number;
}

/** Memory in 512 MB steps, from 1 GB up to half the computer's memory. */
export default function MemorySlider({ label, value, onChange, totalMb, recommendedMb }: Props) {
  const { t } = useTranslation();
  const id = useId();
  const max = Math.max(2048, Math.floor(totalMb / 2 / 512) * 512);
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={id} className="font-semibold">
        {label}: {value} MB
      </label>
      <input id={id} type="range" min={1024} max={max} step={512} value={Math.min(value, max)} onChange={(e) => onChange(Number(e.target.value))} className="w-full accent-[var(--color-accent)]" />
      <div className="text-xs text-muted">{t("detail.memoryHint", { mb: recommendedMb })}</div>
    </div>
  );
}
