import { describe, expect, it } from "vitest";
import { validateRelease } from "./validate-release.mjs";

const baseline = { packageVersion: "2.0.0", tauriVersion: "2.0.0", cargoVersion: "2.0.0", lockVersion: "2.0.0", minimumMacOS: "11.0" };
describe("release safety gates", () => {
  it("allows build-only without creating a tag", () => expect(validateRelease(baseline).createDraft).toBe(false));
  it("allows a matching immutable release tag", () => expect(validateRelease({ ...baseline, tag: "v2.0.0", createDraft: true }).tag).toBe("v2.0.0"));
  it("allows older tags only as build-only runs", () => {
    const result = validateRelease({ ...baseline, packageVersion: "2.0.1", tauriVersion: "2.0.1", cargoVersion: "2.0.1", lockVersion: "2.0.1", tag: "v2.0.0", createDraft: true });
    expect(result.tag).toBe("v2.0.0");
    expect(result.createDraft).toBe(false);
  });
  it.each(["main", "v9.0.0", "", "v2.0.0\nunsafe"])("rejects invalid release tag %s", (tag) => expect(() => validateRelease({ ...baseline, tag, createDraft: true })).toThrow());
  it("rejects mismatched manifest versions", () => expect(() => validateRelease({ ...baseline, cargoVersion: "1.0.0" })).toThrow());
  it("catches the packaging-only deployment regression", () => expect(() => validateRelease({ ...baseline, minimumMacOS: "10.13" })).toThrow());
});
