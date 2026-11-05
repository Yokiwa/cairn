import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";
import { resetMock } from "./lib/mockBackend";
import i18n from "./lib/i18n";

afterEach(() => {
  cleanup();
  resetMock();
  void i18n.changeLanguage("en");
});
