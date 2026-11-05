import { useTranslation } from "react-i18next";
import type { AppError } from "../bindings/AppError";
import { errorText } from "../lib/i18n";

export default function ErrorMessage({ error }: { error: AppError }) {
  const { t } = useTranslation();
  return (
    <div role="alert" className="rounded-[10px] border border-line-strong bg-panel-2 px-4 py-3 text-sm">
      <div className="font-semibold text-danger">{errorText(error, t)}</div>
      {error.detail && (
        <details className="mt-1 text-muted">
          <summary className="cursor-pointer text-xs">{t("common.showDetails")}</summary>
          <pre className="mt-1 whitespace-pre-wrap break-all font-mono text-xs">{error.detail}</pre>
        </details>
      )}
    </div>
  );
}
