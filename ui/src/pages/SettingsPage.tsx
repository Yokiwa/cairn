import { useTranslation } from "react-i18next";

export default function SettingsPage() {
  const { t } = useTranslation();
  return <h1 className="m-0 text-[26px]">{t("settings.title")}</h1>;
}
