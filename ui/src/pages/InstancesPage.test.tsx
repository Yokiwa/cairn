import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "../App";
import { renderApp } from "../test-utils";

async function openWizard() {
  renderApp(<App />, { route: "/instances" });
  await userEvent.click(await screen.findByRole("button", { name: "New instance" }));
  return screen.getByRole("dialog", { name: "New instance" });
}

test("wizard creates instance with defaults", async () => {
  const dialog = await openWizard();
  await userEvent.click(await within(dialog).findByRole("button", { name: "1.21.1" }));
  await userEvent.click(within(dialog).getByRole("button", { name: "Next" }));
  await userEvent.click(await within(dialog).findByRole("button", { name: /^Fabric/ }));
  expect(await within(dialog).findByText(/0\.20\.1/)).toBeInTheDocument();
  await userEvent.click(within(dialog).getByRole("button", { name: "Next" }));
  expect(within(dialog).getByLabelText("Instance name")).toHaveValue("Fabric 1.21.1");
  expect(within(dialog).getByText("Minecraft 1.21.1 · Fabric · Java 21")).toBeInTheDocument();
  await userEvent.click(within(dialog).getByRole("button", { name: "Create" }));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(await screen.findByRole("link", { name: /Fabric 1\.21\.1/ })).toBeInTheDocument();
});

test("wizard shows Java 17 for 1.20.1 and 8 for 1.12.2", async () => {
  const dialog = await openWizard();
  for (const [mc, java] of [["1.20.1", "17"], ["1.12.2", "8"]]) {
    await userEvent.click(await within(dialog).findByRole("button", { name: mc }));
    await userEvent.click(within(dialog).getByRole("button", { name: "Next" }));
    await userEvent.click(within(dialog).getByRole("button", { name: /None \(vanilla\)/ }));
    await userEvent.click(within(dialog).getByRole("button", { name: "Next" }));
    expect(within(dialog).getByText(`Minecraft ${mc} · Vanilla · Java ${java}`)).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: "Back" }));
    await userEvent.click(within(dialog).getByRole("button", { name: "Back" }));
  }
});

test("loader card disabled when no versions for mc", async () => {
  const dialog = await openWizard();
  await userEvent.click(await within(dialog).findByRole("button", { name: "1.12.2" }));
  await userEvent.click(within(dialog).getByRole("button", { name: "Next" }));
  const fabric = await within(dialog).findByRole("button", { name: /^Fabric/ });
  await vi.waitFor(() => expect(fabric).toBeDisabled());
  expect(fabric).toHaveTextContent("Not available for 1.12.2");
  expect(within(dialog).getByRole("button", { name: /Forge.*Classic/ })).toBeEnabled();
});

test("snapshots appear only when switched on", async () => {
  const dialog = await openWizard();
  await within(dialog).findByRole("button", { name: "1.21.1" });
  expect(within(dialog).queryByRole("button", { name: "26.4-snapshot-7" })).not.toBeInTheDocument();
  await userEvent.click(within(dialog).getByRole("switch", { name: "Show snapshots" }));
  expect(await within(dialog).findByRole("button", { name: "26.4-snapshot-7" })).toBeInTheDocument();
});
