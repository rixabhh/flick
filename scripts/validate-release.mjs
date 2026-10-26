import { readFileSync, appendFileSync } from "node:fs";
import { execFileSync } from "node:child_process";

const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));
const RELEASE_TAG_PATTERN = /^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;

// A restored historical tag (e.g. re-creating the original Flick 2.0 release
// after the tag was lost) is older than the packaged version. Such runs stay
// build-only: producing a draft from an older tag would ship artifacts whose
// embedded version disagrees with the release they land on.
const isOlderVersion = (tag, packageVersion) => {
  const asVersion = (value) => value.split(".").map(Number);
  const [tagMajor, tagMinor, tagPatch] = asVersion(tag.slice(1));
  const [packMajor, packMinor, packPatch] = asVersion(packageVersion);
  return tagMajor < packMajor
    || (tagMajor === packMajor && tagMinor < packMinor)
    || (tagMajor === packMajor && tagMinor === packMinor && tagPatch < packPatch);
};

export function validateRelease({ tag = "", createDraft = false, packageVersion, tauriVersion, cargoVersion, lockVersion, minimumMacOS }) {
  if (![tauriVersion, cargoVersion, lockVersion].every((version) => version === packageVersion)) throw new Error("Release versions disagree between package.json, package-lock.json, Cargo.toml and tauri.conf.json.");
  if (minimumMacOS !== "11.0") throw new Error("macOS packaging must use the same 11.0 deployment floor as native verification.");
  if (tag && !RELEASE_TAG_PATTERN.test(tag)) throw new Error("Release tag must be v<version> and match the packaged version exactly.");
  if (tag && tag.slice(1) !== packageVersion) {
    if (!isOlderVersion(tag, packageVersion)) throw new Error("Release tag must be v<version> and match the packaged version exactly.");
    createDraft = false;
  }
  if (createDraft && !tag) throw new Error("Creating a release draft requires an existing version tag; main is not a release tag.");
  return { tag, createDraft };
}

if (process.argv.includes("--check")) {
  const config = readJson("src-tauri/tauri.conf.json");
  const cargo = readFileSync("src-tauri/Cargo.toml", "utf8");
  const tag = process.env.RELEASE_TAG || "";
  const validated = validateRelease({ tag, createDraft: process.env.CREATE_RELEASE === "true", packageVersion: readJson("package.json").version, tauriVersion: config.version,
    cargoVersion: cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1], lockVersion: readJson("package-lock.json").version,
    minimumMacOS: config.bundle.macOS?.minimumSystemVersion });
  const git = (...args) => execFileSync("git", args, { encoding: "utf8" }).trim();
  const sha = git("rev-parse", "HEAD");
  if (tag && git("rev-parse", `refs/tags/${tag}^{commit}`) !== sha) throw new Error("The selected tag does not point to the checked-out source.");
  if (validated.createDraft) git("merge-base", "--is-ancestor", sha, "origin/main");
  if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `sha=${sha}\ntag=${tag}\ncreate_draft=${validated.createDraft}\n`);
  const mode = validated.createDraft ? "draft release"
    : tag ? "historical tag — build-only, no release publication"
    : "build-only, no release publication";
  console.log(`Validated Flick ${config.version} at ${sha}; ${mode}.`);
}
