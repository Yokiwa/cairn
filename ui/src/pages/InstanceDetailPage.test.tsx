import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "../App";
import { getBackend } from "../lib/backend";
import { renderApp } from "../test-utils";

test("toggling a mod calls backend and updates row", async () => {
  renderApp(<App />, { route: "/instances/survival-1-20-1" });
  await userEvent.click(await screen.findByRole("tab", { name: "Mods" }));
  const sw = await screen.findByRole("switch", { name: "sodium-0.5.11.jar" });
  expect(sw).toBeChecked();
  await userEvent.click(sw);
  expect(await screen.findByRole("switch", { name: "sodium-0.5.11.jar.disabled" })).not.toBeChecked();
  const mods = await getBackend().instanceMods("survival-1-20-1");
  expect(mods.find((m) => m.file_name.startsWith("sodium"))!.enabled).toBe(false);
});

test("mod search filters the list", async () => {
  renderApp(<App />, { route: "/instances/survival-1-20-1" });
  await userEvent.click(await screen.findByRole("tab", { name: "Mods" }));
  await screen.findByRole("switch", { name: "lithium-0.11.2.jar" });
  await userEvent.type(screen.getByRole("searchbox", { name: "Search installed mods" }), "sod");
  expect(screen.queryByRole("switch", { name: "lithium-0.11.2.jar" })).not.toBeInTheDocument();
  expect(screen.getByRole("switch", { name: "sodium-0.5.11.jar" })).toBeInTheDocument();
});

test("vanilla instance shows no-mods message", async () => {
  renderApp(<App />, { route: "/instances/vanilla-1-21-4" });
  await userEvent.click(await screen.findByRole("tab", { name: "Mods" }));
  expect(await screen.findByText(/Vanilla instances have no mods/)).toBeInTheDocument();
});

test("instance settings save memory and jvm args", async () => {
  renderApp(<App />, { route: "/instances/tech-valley" });
  await userEvent.click(await screen.findByRole("tab", { name: "Settings" }));
  const args = await screen.findByLabelText("JVM arguments (advanced)");
  await userEvent.clear(args);
  await userEvent.type(args, "-XX:+UseG1GC  -Dfoo=bar");
  await userEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(await screen.findByRole("status")).toHaveTextContent("Saved");
  expect((await getBackend().instanceGet("tech-valley")).jvm_args).toEqual(["-XX:+UseG1GC", "-Dfoo=bar"]);
});

test("delete instance asks for confirmation", async () => {
  renderApp(<App />, { route: "/instances/skyblock" });
  await userEvent.click(await screen.findByRole("button", { name: "Delete" }));
  const dialog = await screen.findByRole("dialog", { name: 'Delete "Skyblock"?' });
  expect(dialog).toHaveTextContent(/trash/);
  await userEvent.click(within(dialog).getByRole("button", { name: "Delete" }));
  expect(await screen.findByRole("heading", { name: "Instances" })).toBeInTheDocument();
  expect((await getBackend().instancesList()).instances.some((i) => i.id === "skyblock")).toBe(false);
});

test("unknown instance shows a message", async () => {
  renderApp(<App />, { route: "/instances/nope" });
  expect(await screen.findByText("This instance no longer exists.")).toBeInTheDocument();
});

test("loader version can be changed in instance settings", async () => {
  renderApp(<App />, { route: "/instances/tech-valley" });
  await userEvent.click(await screen.findByRole("tab", { name: "Settings" }));
  const select = await screen.findByRole("combobox", { name: "Loader version" });
  expect(select).toHaveValue("21.1.77");
  await screen.findByRole("option", { name: /21\.1\.265/ });
  await userEvent.selectOptions(select, "21.1.265");
  await userEvent.click(screen.getByRole("button", { name: "Save" }));
  await vi.waitFor(async () => expect((await getBackend().instanceGet("tech-valley")).loader).toEqual({ kind: "neoforge", version: "21.1.265" }));
});

test("vanilla instances have no loader version setting", async () => {
  renderApp(<App />, { route: "/instances/vanilla-1-21-4" });
  await userEvent.click(await screen.findByRole("tab", { name: "Settings" }));
  await screen.findByLabelText("JVM arguments (advanced)");
  expect(screen.queryByRole("combobox", { name: "Loader version" })).not.toBeInTheDocument();
});
