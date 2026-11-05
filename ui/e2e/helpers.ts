import type { Page } from "@playwright/test";

/** Start from a clean mock (settings in localStorage are cleared too). */
export async function fresh(page: Page, path = "/") {
  await page.goto(path);
  await page.evaluate(() => (window as unknown as { __cairnMock: { reset(): void } }).__cairnMock.reset());
  await page.reload();
}
