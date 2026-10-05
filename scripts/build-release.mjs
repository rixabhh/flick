import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";

const target = process.env.FLICK_BUILD_TARGET;
if (!/^(x86_64|aarch64)-(apple-darwin|pc-windows-msvc|unknown-linux-gnu)$/.test(target || "")) throw new Error("Missing supported build target");
const env = { ...process.env };
const isMacosRelease = target.endsWith("apple-darwin") && process.env.REQUIRE_MACOS_SIGNING === "true";
if (isMacosRelease) {
  const required = ["APPLE_CERTIFICATE", "APPLE_CERTIFICATE_PASSWORD", "APPLE_SIGNING_IDENTITY", "APPLE_ID", "APPLE_PASSWORD", "APPLE_TEAM_ID"];
  const missing = required.filter((key) => !env[key]?.trim());
  if (missing.length) {
    throw new Error(`Refusing to create a distributable macOS installer without signing and notarization credentials: ${missing.join(", ")}`);
  }
}
// Missing optional secrets must be absent, not empty strings: the bundler
// treats the presence of a signing identity as an instruction to sign.
for (const key of Object.keys(env)) {
  if (key.startsWith("APPLE_") && (!env[key]?.trim() || !target.endsWith("apple-darwin"))) delete env[key];
}
const args = ["node_modules/@tauri-apps/cli/tauri.js", "build", "--ci", "--target", target, "--", "--locked"];
const build = () => execFileSync(process.execPath, args, { env, stdio: "inherit" });

try {
  build();
} catch (error) {
  // GitHub's hosted macOS runners occasionally fail inside hdiutil after the
  // signed .app is already complete. Retry that narrow packaging condition
  // once; compilation/configuration errors still fail immediately.
  const appBundle = join("src-tauri", "target", target, "release", "bundle", "macos", "Flick.app");
  if (!target.endsWith("apple-darwin") || !existsSync(appBundle)) throw error;
  console.warn("macOS app bundle is complete but installer packaging failed; retrying once after the disk image service settles.");
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 4_000);
  build();
}
