import { describe, expect, it } from "vitest";
import { supportedLanguages, translate } from "./i18n.js";

describe("localization", () => {
  it("translates the command-center surface into Spanish", () => {
    expect(translate("es", "tab.dictate")).toBe("Dictado");
    expect(translate("es", "home.title")).toBe("Tu espacio de escritura");
    expect(translate("es", "composer.stale")).toBe("Regenera antes de insertar");
    expect(translate("es", "composer.privacy")).toBe("Solo el texto de abajo se usa como contexto. Flick nunca lo guarda.");
    expect(translate("es", "dictation.transcribingCloud")).toBe("Transcribiendo con tu proveedor");
    expect(translate("es", "transform.copied")).toBe("Copiado al portapapeles");
  });

  it("falls back safely to English for missing languages or keys", () => {
    expect(translate("unknown", "tab.home")).toBe("Home");
    expect(translate("en", "missing.key")).toBe("missing.key");
    expect(supportedLanguages.map((language) => language.id)).toEqual(["en", "es"]);
  });
});
