import { act, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "../App";
import { mockCrash, resetMock } from "../lib/mockBackend";
import { renderApp } from "../test-utils";

const offline = { id: "00000000-0000-3000-8000-000000000001", username: "Tester", kind: "offline" as const, active: true };

test("home shows the last played instance", async () => {
  renderApp(<App />);
  expect(await screen.findByRole("heading", { name: "Survival 1.20.1" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Play" })).toBeEnabled();
});

test("home empty state", async () => {
  resetMock({ empty: true });
  renderApp(<App />);
  expect(await screen.findByText("No instances yet")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "New instance" })).toBeInTheDocument();
});

test("launch without account prompts to add one", async () => {
  renderApp(<App />);
  await userEvent.click(await screen.findByRole("button", { name: "Play" }));
  const dialog = await screen.findByRole("dialog", { name: "Add an account first" });
  expect(dialog).toBeInTheDocument();
  await userEvent.click(screen.getByRole("link", { name: "Go to accounts" }));
  expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
});

test("crash opens log panel with tail", async () => {
  resetMock({ accounts: [offline] });
  renderApp(<App />);
  await userEvent.click(await screen.findByRole("button", { name: "Play" }));
  expect(await screen.findByRole("button", { name: "Game running" })).toBeDisabled();
  act(() => mockCrash("survival-1-20-1"));
  const panel = await screen.findByRole("dialog", { name: "The game closed unexpectedly" });
  expect(panel).toHaveTextContent("java.lang.IllegalStateException: sample crash");
  expect(screen.getByRole("button", { name: "Open log folder" })).toBeInTheDocument();
  expect(await screen.findByRole("button", { name: "Play" })).toBeEnabled();
});
