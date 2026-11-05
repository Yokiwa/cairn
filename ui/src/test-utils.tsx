import type { ReactElement } from "react";
import { render } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router";
import Providers from "./Providers";
import { useUi } from "./lib/store";

/** Render inside every provider the app uses, at `route`. */
export function renderApp(ui: ReactElement, { route = "/", path }: { route?: string; path?: string } = {}) {
  useUi.getState().reset();
  return render(
    <Providers>
      <MemoryRouter initialEntries={[route]}>
        {path ? (
          <Routes>
            <Route path={path} element={ui} />
          </Routes>
        ) : (
          ui
        )}
      </MemoryRouter>
    </Providers>,
  );
}
