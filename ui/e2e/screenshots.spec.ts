import { expect, test } from "@playwright/test";
import { fresh } from "./helpers";

// Run with SCREENSHOTS=1 to refresh docs/screenshots (used by the README).
test.skip(!process.env.SCREENSHOTS, "set SCREENSHOTS=1 to capture");
test.use({ deviceScaleFactor: 2 });

for (const theme of ["dark", "light"] as const) {
  test(`capture (${theme})`, async ({ page }, info) => {
    const shot = (name: string) => page.screenshot({ path: `../docs/screenshots/${name}-${theme}-${info.project.name}.png` });
    await fresh(page, "/settings");
    await page.getByRole("button", { name: theme === "dark" ? "Dark" : "Light" }).click();
    await page.getByLabel("Player name").fill("PlayerOne");
    await page.getByRole("button", { name: "Add offline account" }).click();
    await page.goto("/");
    await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
    await shot("home");
    await page.goto("/instances");
    await page.getByRole("button", { name: "New instance" }).click();
    const dialog = page.getByRole("dialog");
    await dialog.getByRole("button", { name: "1.21.1", exact: true }).click();
    await dialog.getByRole("button", { name: "Next" }).click();
    await dialog.getByRole("button", { name: /^Fabric/ }).click();
    await expect(dialog.getByText(/0\.20\.1/)).toBeVisible();
    await expect(dialog).toHaveCSS("opacity", "1");
    await shot("wizard");
    await page.goto("/instances/survival-1-20-1");
    await page.getByRole("tab", { name: "Mods" }).click();
    await expect(page.getByRole("switch").first()).toBeVisible();
    await shot("detail");
    await page.goto("/settings");
    await shot("settings");
  });
}
