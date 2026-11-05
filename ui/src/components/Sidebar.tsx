import { NavLink } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getBackend } from "../lib/backend";
import Logo from "./Logo";

const icons = {
  home: <path d="M3 11l9-7 9 7M5 10v10h14V10" />,
  instances: (
    <>
      <rect x="3" y="3" width="7" height="7" rx="1.5" />
      <rect x="14" y="3" width="7" height="7" rx="1.5" />
      <rect x="3" y="14" width="7" height="7" rx="1.5" />
      <rect x="14" y="14" width="7" height="7" rx="1.5" />
    </>
  ),
  settings: (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 2v3M12 19v3M4.2 4.2l2.1 2.1M17.7 17.7l2.1 2.1M2 12h3M19 12h3M4.2 19.8l2.1-2.1M17.7 6.3l2.1-2.1" />
    </>
  ),
};

function Item({ to, label, icon, end }: { to: string; label: string; icon: keyof typeof icons; end?: boolean }) {
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) =>
        `flex min-h-11 items-center gap-3 rounded-[10px] px-3.5 text-sm font-medium no-underline transition ${isActive ? "bg-panel-2 text-accent-strong" : "text-muted hover:bg-panel-2 hover:text-fg"}`
      }
    >
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        {icons[icon]}
      </svg>
      {label}
    </NavLink>
  );
}

export default function Sidebar() {
  const { t } = useTranslation();
  const accounts = useQuery({ queryKey: ["accounts"], queryFn: () => getBackend().accountsList() });
  const active = accounts.data?.find((a) => a.active);
  return (
    <nav aria-label={t("nav.main")} className="flex flex-[1_1_220px] flex-col gap-1 border-b border-line bg-sidebar p-3.5 sm:max-w-[240px] sm:border-r sm:border-b-0">
      <div className="flex items-center gap-2.5 px-2.5 pt-1 pb-4">
        <Logo />
        <span className="text-base font-bold tracking-wide">Cairn</span>
      </div>
      <div className="flex flex-wrap gap-1 sm:flex-col">
        <Item to="/" end label={t("nav.home")} icon="home" />
        <Item to="/instances" label={t("nav.instances")} icon="instances" />
        <Item to="/settings" label={t("nav.settings")} icon="settings" />
      </div>
      <div className="hidden grow sm:block" />
      {active && (
        <div className="hidden items-center gap-2.5 rounded-xl bg-panel p-2.5 sm:flex">
          <div aria-hidden="true" className="grid h-9 w-9 place-items-center rounded-lg bg-accent font-bold text-accent-fg">
            {active.username[0]?.toUpperCase()}
          </div>
          <div className="min-w-0">
            <div className="truncate text-[13px] font-semibold">{active.username}</div>
            <div className="text-xs text-muted">{t(active.kind === "microsoft" ? "settings.microsoft" : "settings.offline")}</div>
          </div>
        </div>
      )}
    </nav>
  );
}
