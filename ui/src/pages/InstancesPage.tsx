import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import Button from "../components/Button";
import ErrorMessage from "../components/ErrorMessage";
import InstanceCard from "../components/InstanceCard";
import NewInstanceWizard from "../components/NewInstanceWizard";
import { getBackend, toAppError } from "../lib/backend";
import { errorText } from "../lib/i18n";

export default function InstancesPage() {
  const { t } = useTranslation();
  const [wizard, setWizard] = useState(false);
  const q = useQuery({ queryKey: ["instances"], queryFn: () => getBackend().instancesList() });
  return (
    <div className="flex flex-col gap-4.5">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="m-0 text-[26px]">{t("instances.title")}</h1>
          <div className="text-muted">{t("instances.subtitle")}</div>
        </div>
        <Button variant="primary" onClick={() => setWizard(true)}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" aria-hidden="true">
            <path d="M12 5v14M5 12h14" />
          </svg>
          {t("instances.new")}
        </Button>
      </div>
      {q.error && <ErrorMessage error={toAppError(q.error)} />}
      {q.data?.errors.map((e, i) => (
        <div key={i} className="text-sm text-warn">
          {t("instances.skipped", { message: errorText(e, t) })}
        </div>
      ))}
      <div className="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-4">
        {q.data?.instances.map((i) => <InstanceCard key={i.id} instance={i} />)}
      </div>
      {wizard && <NewInstanceWizard onClose={() => setWizard(false)} />}
    </div>
  );
}
