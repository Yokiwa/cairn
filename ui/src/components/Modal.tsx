import { useEffect, useRef, type ReactNode } from "react";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";

interface Props {
  title: string;
  onClose(): void;
  children: ReactNode;
  width?: number;
}

export default function Modal({ title, onClose, children, width = 640 }: Props) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null;
    ref.current?.querySelector<HTMLElement>("input, button:not([data-close]), select, textarea, [tabindex]")?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      prev?.focus?.();
    };
  }, [onClose]);
  return (
    <div className="fixed inset-0 z-40 flex items-start justify-center overflow-y-auto bg-[var(--overlay)] px-4 py-12" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <motion.div
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        initial={{ opacity: 0, scale: 0.97 }}
        animate={{ opacity: 1, scale: 1 }}
        transition={{ duration: 0.2 }}
        style={{ width, maxWidth: "100%" }}
        className="flex flex-col gap-4 rounded-2xl border border-line bg-panel p-6 shadow-2xl"
      >
        <div className="flex items-center justify-between gap-3">
          <h2 className="m-0 text-xl font-bold">{title}</h2>
          <button type="button" data-close aria-label={t("common.close")} onClick={onClose} className="grid min-h-11 min-w-11 place-items-center rounded-[10px] text-muted hover:bg-panel-2 hover:text-fg">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" aria-hidden="true">
              <path d="M6 6l12 12M18 6L6 18" />
            </svg>
          </button>
        </div>
        {children}
      </motion.div>
    </div>
  );
}
