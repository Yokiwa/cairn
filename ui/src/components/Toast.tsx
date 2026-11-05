import { useEffect } from "react";
import { AnimatePresence, motion } from "motion/react";
import { useUi } from "../lib/store";

export default function Toast() {
  const toast = useUi((s) => s.toast);
  const show = useUi((s) => s.showToast);
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => show(null), 4000);
    return () => clearTimeout(t);
  }, [toast, show]);
  return (
    <div aria-live="polite" className="pointer-events-none fixed inset-x-0 top-5 z-50 flex justify-center px-4">
      <AnimatePresence>
        {toast && (
          <motion.div
            role="status"
            initial={{ opacity: 0, y: -8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -8 }}
            className="pointer-events-auto max-w-full rounded-xl border border-line-strong bg-panel px-4 py-3 font-semibold shadow-xl"
          >
            {toast}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
