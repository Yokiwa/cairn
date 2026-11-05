import { expect, test } from "@playwright/test";
import { fresh } from "./helpers";

test("create a Fabric 1.20.1 instance and watch it install", async ({ page }) => {
  await fresh(page, "/instances");
  await page.getByRole("button", { name: "New instance" }).click();
  const dialog = page.getByRole("dialog", { name: "New instance" });
  await dialog.getByRole("button", { name: "1.20.1", exact: true }).click();
  await dialog.getByRole("button", { name: "Next" }).click();
  await dialog.getByRole("button", { name: /^Fabric/ }).click();
  await expect(dialog.getByText(/0\.20\.1/)).toBeVisible();
  await dialog.getByRole("button", { name: "Next" }).click();
  await expect(dialog.getByLabel("Instance name")).toHaveValue("Fabric 1.20.1");
  await dialog.getByRole("button", { name: "Create" }).click();
  await expect(dialog).toBeHidden();
  const capsule = page.getByRole("button", { name: "Downloads" });
  await expect(capsule).toBeVisible();
  await expect(capsule).toContainText("All done");
  await expect(page.getByRole("link", { name: /Fabric 1\.20\.1/ })).toBeVisible();
});

test("no horizontal scrolling", async ({ page }) => {
  for (const path of ["/", "/instances", "/instances/survival-1-20-1", "/settings"]) {
    await fresh(page, path);
    await page.waitForLoadState("networkidle");
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    expect(overflow, path).toBeLessThanOrEqual(0);
  }
});
