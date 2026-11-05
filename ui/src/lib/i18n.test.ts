import en from "../locales/en.json";
import zh from "../locales/zh-CN.json";
import i18n, { detectLanguage, errorText } from "./i18n";

test("detectLanguage", () => {
  expect(detectLanguage("zh-CN")).toBe("zh-CN");
  expect(detectLanguage("zh-TW")).toBe("zh-CN");
  expect(detectLanguage("en-US")).toBe("en");
  expect(detectLanguage("de-DE")).toBe("en");
  expect(detectLanguage("en-US", "zh-CN")).toBe("zh-CN");
  expect(detectLanguage("zh-CN", "en")).toBe("en");
});

function keys(o: unknown, prefix = ""): string[] {
  if (typeof o !== "object" || o === null) return [prefix];
  return Object.entries(o).flatMap(([k, v]) => keys(v, prefix ? `${prefix}.${k}` : k));
}

test("en and zh-CN have identical key sets", () => {
  expect(keys(zh).sort()).toEqual(keys(en).sort());
});

test("errorText interpolates params", async () => {
  await i18n.changeLanguage("en");
  const text = errorText({ key: "error.download.checksum", params: { file: "a.jar" }, detail: "" }, i18n.t);
  expect(text).toContain("a.jar");
  await i18n.changeLanguage("zh-CN");
  expect(errorText({ key: "error.download.checksum", params: { file: "a.jar" }, detail: "" }, i18n.t)).toContain("损坏");
});

test("errorText falls back for unknown keys", () => {
  expect(errorText({ key: "error.nope", params: {}, detail: "boom" }, i18n.t)).toMatch(/boom|went wrong/);
});
