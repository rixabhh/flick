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
