import { act, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "../App";
import { getBackend } from "../lib/backend";
import { mockFinishLogin, resetMock } from "../lib/mockBackend";
import { renderApp } from "../test-utils";

test("language switch applies immediately and persists", async () => {
  renderApp(<App />, { route: "/settings" });
  await userEvent.click(await screen.findByRole("button", { name: "中文" }));
  expect(await screen.findByRole("heading", { name: "设置" })).toBeInTheDocument();
  expect((await getBackend().settingsGet()).language).toBe("zh-CN");
  await userEvent.click(screen.getByRole("button", { name: "English" }));
  expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
  expect((await getBackend().settingsGet()).language).toBe("en");
});

test("theme setting sets data-theme", async () => {
  renderApp(<App />, { route: "/settings" });
  await userEvent.click(await screen.findByRole("button", { name: "Light" }));
  await vi.waitFor(() => expect(document.documentElement.dataset.theme).toBe("light"));
  await userEvent.click(screen.getByRole("button", { name: "Dark" }));
  await vi.waitFor(() => expect(document.documentElement.dataset.theme).toBe("dark"));
  await userEvent.click(screen.getByRole("button", { name: "Sky" }));
  await vi.waitFor(() => expect(document.documentElement.style.getPropertyValue("--accent")).toBe("#7DD3FC"));
});

test("offline account name validation message", async () => {
  renderApp(<App />, { route: "/settings" });
  await userEvent.type(await screen.findByLabelText("Player name"), "a b");
  await userEvent.click(screen.getByRole("button", { name: "Add offline account" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Player names are 3–16 characters");
  await userEvent.clear(screen.getByLabelText("Player name"));
  await userEvent.type(screen.getByLabelText("Player name"), "Tester");
  await userEvent.click(screen.getByRole("button", { name: "Add offline account" }));
  expect(await within(screen.getByRole("region", { name: "Accounts" })).findByText("Tester")).toBeInTheDocument();
});

test("device login dialog shows code and closes on done", async () => {
  resetMock({ msaConfigured: true });
  renderApp(<App />, { route: "/settings" });
  await userEvent.click(await screen.findByRole("button", { name: "Add Microsoft account" }));
  const dialog = await screen.findByRole("dialog", { name: "Sign in with Microsoft" });
  expect(within(dialog).getByText("ABCD-EFGH")).toBeInTheDocument();
  expect(within(dialog).getByText("https://www.microsoft.com/link")).toBeInTheDocument();
  act(() => mockFinishLogin("Steve_MS"));
  await vi.waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  expect(await within(screen.getByRole("region", { name: "Accounts" })).findByText("Steve_MS")).toBeInTheDocument();
});

test("microsoft sign-in not configured explains itself", async () => {
  renderApp(<App />, { route: "/settings" });
  await userEvent.click(await screen.findByRole("button", { name: "Add Microsoft account" }));
  expect(await screen.findByText(/isn't set up in this build yet/)).toBeInTheDocument();
});

test("accounts can be switched and removed", async () => {
  resetMock({
    accounts: [
      { id: "a", username: "Alpha", kind: "offline", active: true },
      { id: "b", username: "Beta", kind: "offline", active: false },
    ],
  });
  renderApp(<App />, { route: "/settings" });
  await userEvent.click(await screen.findByRole("button", { name: "Use Beta" }));
  await vi.waitFor(async () => expect((await getBackend().accountsList()).find((a) => a.active)?.username).toBe("Beta"));
  await userEvent.click(screen.getByRole("button", { name: "Remove Alpha" }));
  await vi.waitFor(() => expect(within(screen.getByRole("region", { name: "Accounts" })).queryByText("Alpha")).not.toBeInTheDocument());
});
