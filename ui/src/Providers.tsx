import { useState, type ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { I18nextProvider } from "react-i18next";
import { MotionConfig } from "motion/react";
import i18n from "./lib/i18n";
import { EventsBridge } from "./lib/events";

export function makeQueryClient() {
  return new QueryClient({ defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false, staleTime: 2000 } } });
}

export default function Providers({ children, client }: { children: ReactNode; client?: QueryClient }) {
  const [qc] = useState(() => client ?? makeQueryClient());
  return (
    <QueryClientProvider client={qc}>
      <I18nextProvider i18n={i18n}>
        <MotionConfig reducedMotion="user">
          <EventsBridge />
          {children}
        </MotionConfig>
      </I18nextProvider>
    </QueryClientProvider>
  );
}
