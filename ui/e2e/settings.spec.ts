import { expect, test } from "@playwright/test";
import { fresh } from "./helpers";

test("language choice survives a reload", async ({ page }) => {
  await fresh(page, "/settings");
  await page.getByRole("button", { name: "中文" }).click();
  await expect(page.getByRole("heading", { name: "设置", level: 1 })).toBeVisible();
  await page.reload();
  await expect(page.getByRole("heading", { name: "设置", level: 1 })).toBeVisible();
  await page.getByRole("button", { name: "English" }).click();
  await expect(page.getByRole("heading", { name: "Settings", level: 1 })).toBeVisible();
});

test("light theme applies to the page", async ({ page }) => {
  await fresh(page, "/settings");
  await page.getByRole("button", { name: "Light" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
});
