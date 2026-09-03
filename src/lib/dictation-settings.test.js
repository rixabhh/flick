import { describe, expect, it } from "vitest";
import { formatCorrections, parseCorrections } from "./dictation-settings.js";

describe("dictation correction settings", () => {
  it("parses valid correction lines and ignores incomplete entries", () => {
    expect(parseCorrections("Acme => ACME\nmissing separator\n  Jon => John  \n=> ignored"))
      .toEqual([{ find: "Acme", replace: "ACME" }, { find: "Jon", replace: "John" }]);
  });

  it("formats corrections for round-tripping through the settings textarea", () => {
    const corrections = [{ find: "Flick", replace: "Flick 2" }];
    expect(parseCorrections(formatCorrections(corrections))).toEqual(corrections);
  });
});
