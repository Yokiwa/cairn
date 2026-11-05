import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";
import { fresh } from "./helpers";

async function noSeriousViolations(page: Page, label: string) {
  const results = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa"]).analyze();
  const bad = results.violations.filter((v) => v.impact === "serious" || v.impact === "critical");
  expect(bad.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`), label).toEqual([]);
}

for (const theme of ["dark", "light"] as const) {
  test(`no serious accessibility problems (${theme})`, async ({ page }) => {
    await fresh(page, "/settings");
    await page.getByRole("button", { name: theme === "dark" ? "Dark" : "Light" }).click();
    for (const path of ["/", "/instances", "/instances/survival-1-20-1", "/settings"]) {
      await page.goto(path);
      await page.waitForLoadState("networkidle");
      await noSeriousViolations(page, `${theme} ${path}`);
    }
    await page.goto("/instances");
    await page.getByRole("button", { name: "New instance" }).click();
    await page.getByRole("dialog").getByRole("button", { name: "1.21.1", exact: true }).waitFor();
    // Measure after the open animation, not while the dialog is still fading in.
    await expect(page.getByRole("dialog")).toHaveCSS("opacity", "1");
    await noSeriousViolations(page, `${theme} wizard`);
  });
}

test("create an instance with the keyboard only", async ({ page }) => {
  await fresh(page, "/instances");
  const newButton = page.getByRole("button", { name: "New instance" });
  await newButton.focus();
  await page.keyboard.press("Enter");
  const dialog = page.getByRole("dialog", { name: "New instance" });
  await expect(dialog).toBeVisible();
  // Tab until the 1.21.1 version button has focus, then pick it.
  const target = dialog.getByRole("button", { name: "1.21.1", exact: true });
  await target.waitFor();
  for (let i = 0; i < 40 && !(await target.evaluate((el) => el === document.activeElement)); i++) await page.keyboard.press("Tab");
  await page.keyboard.press("Enter");
  const next = dialog.getByRole("button", { name: "Next" });
  for (let i = 0; i < 40 && !(await next.evaluate((el) => el === document.activeElement)); i++) await page.keyboard.press("Tab");
  await page.keyboard.press("Enter"); // → loader step (vanilla is preselected)
  for (let i = 0; i < 40 && !(await next.evaluate((el) => el === document.activeElement)); i++) await page.keyboard.press("Tab");
  await page.keyboard.press("Enter"); // → name step
  const create = dialog.getByRole("button", { name: "Create" });
  for (let i = 0; i < 40 && !(await create.evaluate((el) => el === document.activeElement)); i++) await page.keyboard.press("Tab");
  await page.keyboard.press("Space");
  await expect(dialog).toBeHidden();
  await expect(page.getByRole("link", { name: /Vanilla 1\.21\.1/ })).toBeVisible();
});
