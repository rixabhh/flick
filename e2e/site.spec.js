import { expect, test } from "@playwright/test";

test("product page reveals its full story on scroll", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/docs/index.html");
  for (const selector of [".hero-copy", ".principles", ".story-intro", ".dictation-copy", ".privacy-title", ".closing"]) {
    const section = page.locator(selector);
    await section.scrollIntoViewIfNeeded();
    await expect(section).toHaveCSS("opacity", "1");
  }
  expect(errors).toEqual([]);
});

test("product page works on a narrow screen without JavaScript", async ({ browser }) => {
  const context = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 390, height: 844 } });
  const page = await context.newPage();
  await page.goto("http://127.0.0.1:1420/docs/index.html");
  await expect(page.locator(".hero-copy")).toHaveCSS("opacity", "1");
  await expect(page.locator(".closing")).toHaveCSS("opacity", "1");
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await context.close();
});

test("product page respects the reduced-motion preference", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/docs/index.html");
  await expect(page.locator(".closing")).toHaveCSS("opacity", "1");
  await expect(page.locator("html")).not.toHaveClass(/motion-ready/);
});
