import { useEffect } from "react";
import { Route, Routes } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import Sidebar from "./components/Sidebar";
import DownloadCapsule from "./components/DownloadCapsule";
import Toast from "./components/Toast";
import GameLogPanel from "./components/GameLogPanel";
import HomePage from "./pages/HomePage";
import InstancesPage from "./pages/InstancesPage";
import InstanceDetailPage from "./pages/InstanceDetailPage";
import SettingsPage from "./pages/SettingsPage";
import { getBackend } from "./lib/backend";
import { detectLanguage } from "./lib/i18n";
import { useApplyTheme } from "./lib/theme";

export default function App() {
  const { i18n } = useTranslation();
  const settings = useQuery({ queryKey: ["settings"], queryFn: () => getBackend().settingsGet() });
  useApplyTheme(settings.data);
  useEffect(() => {
    if (!settings.data) return;
    const lang = detectLanguage(navigator.language, settings.data.language);
    if (i18n.language !== lang) void i18n.changeLanguage(lang);
    document.documentElement.lang = lang;
  }, [settings.data, i18n]);

  return (
    <div className="flex min-h-full flex-col sm:flex-row">
      <Sidebar />
      <main className="min-w-0 flex-[999_1_560px] px-4 pt-6 pb-28 sm:px-7">
        <Routes>
          <Route path="/" element={<HomePage />} />
          <Route path="/instances" element={<InstancesPage />} />
          <Route path="/instances/:id" element={<InstanceDetailPage />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Routes>
      </main>
      <DownloadCapsule />
      <GameLogPanel />
      <Toast />
    </div>
  );
}
