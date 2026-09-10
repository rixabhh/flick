import { readFileSync, appendFileSync } from "node:fs";
import { execFileSync } from "node:child_process";

const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));
export function validateRelease({ tag = "", createDraft = false, packageVersion, tauriVersion, cargoVersion, lockVersion, minimumMacOS }) {
  if (![tauriVersion, cargoVersion, lockVersion].every((version) => version === packageVersion)) throw new Error("Release versions disagree between package.json, package-lock.json, Cargo.toml and tauri.conf.json.");
  if (minimumMacOS !== "11.0") throw new Error("macOS packaging must use the same 11.0 deployment floor as native verification.");
  if (createDraft && !tag) throw new Error("Creating a release draft requires an existing version tag; main is not a release tag.");
  if (tag && (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag) || tag.slice(1) !== packageVersion)) throw new Error("Release tag must be v<version> and match the packaged version exactly.");
  return { tag, createDraft };
}

if (process.argv.includes("--check")) {
  const config = readJson("src-tauri/tauri.conf.json");
  const cargo = readFileSync("src-tauri/Cargo.toml", "utf8");
  const tag = process.env.RELEASE_TAG || "";
  const createDraft = process.env.CREATE_RELEASE === "true";
  validateRelease({ tag, createDraft, packageVersion: readJson("package.json").version, tauriVersion: config.version,
    cargoVersion: cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1], lockVersion: readJson("package-lock.json").version,
    minimumMacOS: config.bundle.macOS?.minimumSystemVersion });
  const git = (...args) => execFileSync("git", args, { encoding: "utf8" }).trim();
  const sha = git("rev-parse", "HEAD");
  if (tag && git("rev-parse", `refs/tags/${tag}^{commit}`) !== sha) throw new Error("The selected tag does not point to the checked-out source.");
  if (createDraft) git("merge-base", "--is-ancestor", sha, "origin/main");
  if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `sha=${sha}\ntag=${tag}\ncreate_draft=${createDraft}\n`);
  console.log(`Validated Flick ${config.version} at ${sha}; ${createDraft ? "draft release" : "build-only, no release publication"}.`);
}
