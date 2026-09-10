import { expect, test } from 'vitest';
import { buildCatalog, renderCatalog } from './generate-model-catalog.mjs';
import fs from 'node:fs';
test('generated catalog matches its pinned data sources with no duplicate files or IDs', () => {
  const models = buildCatalog();
  expect(models.length).toBeGreaterThan(350);
  expect(new Set(models.map((model) => model.id)).size).toBe(models.length);
  expect(new Set(models.map((model) => model.file_name)).size).toBe(models.length);
  expect(fs.readFileSync(new URL('../src-tauri/src/model_catalog.rs', import.meta.url), 'utf8').replaceAll('\r\n', '\n')).toBe(renderCatalog());
  for (const model of models) {
    expect(model.url).toMatch(/\/resolve\/[a-f0-9]{40}\//);
    expect(model.sha256).toMatch(/^[a-f0-9]{64}$/);
    expect(model.source_url).toMatch(/^https:\/\/huggingface.co\//);
  }
});
test('Parakeet variants, precision choices and non-English single-language models retain their capabilities', () => {
  const models = buildCatalog();
  for (const fragment of ['parakeet-tdt-0.6b-v2', 'parakeet-tdt-0.6b-v3', 'parakeet-ctc-0.6b', 'parakeet-rnnt-0.6b', 'parakeet-unified-en']) {
    const variants = models.filter((model) => model.id.includes(fragment));
    expect(variants.length).toBeGreaterThanOrEqual(6);
    expect(variants.some((model) => model.quant === 'Q4_K_M')).toBe(true);
  }
  const japanese = models.find((model) => model.id === 'moonshine-tiny-ja-q8_0');
  expect(japanese.supported_languages).toEqual(['ja']);
  expect(japanese.supports_language_detection).toBe(false);
});
