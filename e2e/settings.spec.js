import { expect, test } from "@playwright/test";

test("command center exposes first-run setup and all primary sections", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Your writing workspace" })).toBeVisible();
  await expect(page.getByText("First-time setup")).toBeVisible();

  for (const section of ["Home", "Write", "Commands", "Dictate", "Models", "History", "Privacy", "Advanced"]) {
    await expect(page.getByRole("tab", { name: section, exact: true })).toBeVisible();
  }
});

test("primary settings sections work with keyboard navigation", async ({ page }) => {
  await page.goto("/");

  const models = page.getByRole("tab", { name: "Models", exact: true });
  await models.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { name: "Speech models" })).toBeVisible();

  const privacy = page.getByRole("tab", { name: "Privacy", exact: true });
  await privacy.focus();
  await page.keyboard.press("Space");
  await expect(page.getByRole("heading", { name: "Privacy", exact: true })).toBeVisible();

  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("tab", { name: "Advanced", exact: true })).toBeFocused();
  await expect(page.getByRole("tab", { name: "Advanced", exact: true })).toHaveAttribute("aria-selected", "true");
});

test("dictation settings exposes a stable floating-pill placement choice", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("tab", { name: "Dictate", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Dictation", exact: true })).toBeVisible();

  const placement = page.getByLabel("Floating pill position");
  await expect(placement).toHaveValue("bottom-center");
  await placement.selectOption("bottom-right");
  await expect(placement).toHaveValue("bottom-right");
  await expect(page.getByText("Used for both dictation and transformation status.")).toBeVisible();
});
