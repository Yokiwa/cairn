import { expect, test } from "@playwright/test";
import { fresh } from "./helpers";

test("add an offline account, play, then see the crash panel", async ({ page }) => {
  await fresh(page, "/settings");
  await page.getByLabel("Player name").fill("Tester");
  await page.getByRole("button", { name: "Add offline account" }).click();
  await expect(page.getByRole("region", { name: "Accounts" }).getByText("Tester")).toBeVisible();

  await page.getByRole("link", { name: "Home" }).click();
  await page.getByRole("button", { name: "Play" }).click();
  await expect(page.getByRole("button", { name: "Game running" })).toBeDisabled();

  await page.evaluate(() => (window as unknown as { __cairnMock: { crash(id: string): void } }).__cairnMock.crash("survival-1-20-1"));
  const panel = page.getByRole("dialog", { name: "The game closed unexpectedly" });
  await expect(panel).toBeVisible();
  await expect(panel).toContainText("IllegalStateException");
  await panel.getByRole("button", { name: "Close" }).last().click();
  await expect(page.getByRole("button", { name: "Play" })).toBeEnabled();
});

test("playing without an account leads to the accounts section", async ({ page }) => {
  await fresh(page);
  await page.getByRole("button", { name: "Play" }).click();
  await page.getByRole("link", { name: "Go to accounts" }).click();
  await expect(page.getByRole("region", { name: "Accounts" })).toBeVisible();
});
