import { defineConfig, devices } from "@playwright/test";

// Runs the UI in a plain browser, where it uses the in-memory mock backend.
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["github"], ["html", { open: "never" }]] : "list",
  use: { baseURL: "http://localhost:5173", locale: "en-US", trace: "retain-on-failure" },
  projects: [
    { name: "desktop", use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } } },
    { name: "phone", use: { ...devices["Desktop Chrome"], viewport: { width: 390, height: 844 }, isMobile: false } },
  ],
  webServer: { command: "pnpm dev", url: "http://localhost:5173", reuseExistingServer: !process.env.CI, timeout: 60_000 },
});
