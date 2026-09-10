import { expect, test } from "@playwright/test";
import { mockDesktop, emit, calls } from "./tauri-fixture.js";

test("composer prevents repeated keyboard requests and discards a previous session's reply", async ({ page }) => {
  await mockDesktop(page, "composer");
  await page.goto("/");
  await expect(page.locator("#context")).toBeVisible();
  await page.evaluate(() => window.__flickTest.held.push("generate_reply"));
  await page.locator("#context").fill("Can we meet on Monday?");
  await page.locator("#intent").fill("Say yes");
  await page.keyboard.press("Control+Enter");
  await page.keyboard.press("Control+Enter");
  await expect.poll(async () => (await calls(page, "generate_reply")).length).toBe(1);
  await emit(page, "flick://composer-context", { context: "A different conversation" });
  await page.evaluate(() => window.__flickTest.resolve("generate_reply", "Old private reply"));
  await expect(page.locator("#context")).toHaveValue("A different conversation");
  await expect(page.getByText("Old private reply")).toHaveCount(0);
  await expect(page.locator(".generate")).toBeEnabled();
  expect((await calls(page, "update_config_fields")).length).toBe(0);
});

test("editing reply inputs during generation makes insertion unavailable", async ({ page }) => {
  await mockDesktop(page, "composer");
  await page.goto("/");
  await page.locator("#context").fill("Meeting tomorrow?");
  await page.locator("#intent").fill("Accept");
  await page.evaluate(() => window.__flickTest.held.push("generate_reply"));
  await page.locator(".generate").click();
  await page.locator("#intent").fill("Decline");
  await page.evaluate(() => window.__flickTest.resolve("generate_reply", "Yes, see you tomorrow."));
  await expect(page.getByRole("button", { name: "Insert into app" })).toBeDisabled();
  expect((await calls(page, "insert_reply")).length).toBe(0);
});

test("preflight errors actually show a previously hidden status pill", async ({ page }) => {
  await mockDesktop(page, "toast");
  await page.goto("/");
  await expect.poll(async () => (await calls(page, "plugin:event|listen")).length).toBe(5);
  await emit(page, "flick://error", { message: "Microphone permission denied" });
  await expect(page.getByRole("alert")).toContainText("Microphone permission denied");
  await expect.poll(async () => (await calls(page, "plugin:window|show")).length).toBe(1);
});

test("settings sends field patches and preserves native model metadata", async ({ page }) => {
  await mockDesktop(page);
  await page.goto("/");
  await page.getByRole("tab", { name: "Dictate", exact: true }).click();
  await page.evaluate(() => {
    window.__flickTest.config.local_models = [{ id: "new-download" }];
    window.__flickTest.config.dictation_model_id = "parakeet";
  });
  await page.getByLabel("Floating pill position").selectOption("bottom-right");
  await expect.poll(async () => (await calls(page, "update_config_fields")).length).toBe(1);
  expect((await calls(page, "update_config_fields"))[0].args.patch).toEqual({ floating_pill_position: "bottom-right" });
  expect(await page.evaluate(() => window.__flickTest.config.dictation_model_id)).toBe("parakeet");
  expect((await calls(page, "save_config")).length).toBe(0);
});

test("a delayed key load cannot leak into the next provider", async ({ page }) => {
  await mockDesktop(page);
  await page.goto("/");
  await page.evaluate(() => window.__flickTest.held.push("load_api_key"));
  await page.getByRole("tab", { name: "Write", exact: true }).click();
  await expect.poll(async () => (await calls(page, "load_api_key")).length).toBe(1);
  await page.locator(".provider-controls select").first().selectOption("openrouter");
  await expect.poll(async () => (await calls(page, "load_api_key")).length).toBe(2);
  await page.evaluate(() => window.__flickTest.resolve("load_api_key", "router-secret", 1));
  await expect(page.locator("#api-key-input")).toHaveValue("router-secret");
  await page.evaluate(() => window.__flickTest.resolve("load_api_key", "old-gemini-secret"));
  await expect(page.locator("#api-key-input")).toHaveValue("router-secret");
  await page.getByRole("button", { name: "Save Key", exact: true }).click();
  await expect.poll(async () => (await calls(page, "save_api_key")).length).toBe(1);
  expect((await calls(page, "save_api_key"))[0].args).toEqual({ key: "router-secret", provider: "openrouter" });
});

test("model download failures are recoverable and retry clears the error", async ({ page }) => {
  await mockDesktop(page);
  await page.goto("/");
  await page.evaluate(() => window.__flickTest.models = [{ id: "tiny", name: "Tiny test model", size_bytes: 1000, installed: false, supported_languages: ["en"] }]);
  await page.getByRole("tab", { name: "Models", exact: true }).click();
  await page.getByRole("button", { name: "Download", exact: true }).click();
  await emit(page, "flick://model-download", { id: "tiny", state: "failed", message: "Connection timed out" });
  await expect(page.getByRole("alert")).toContainText("Connection timed out");
  await page.getByRole("button", { name: "Download", exact: true }).click();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await emit(page, "flick://model-download", { id: "tiny", state: "progress", received: 400, total: 1000 });
  await expect(page.getByRole("progressbar")).toHaveAttribute("value", "400");
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect.poll(async () => (await calls(page, "cancel_local_model_download")).length).toBe(1);
});
