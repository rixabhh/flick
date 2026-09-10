import { execFileSync } from "node:child_process";

const target = process.env.FLICK_BUILD_TARGET;
if (!/^(x86_64|aarch64)-(apple-darwin|pc-windows-msvc|unknown-linux-gnu)$/.test(target || "")) throw new Error("Missing supported build target");
const env = { ...process.env };
// Missing optional secrets must be absent, not empty strings: the bundler
// treats the presence of a signing identity as an instruction to sign.
for (const key of Object.keys(env)) {
  if (key.startsWith("APPLE_") && (!env[key]?.trim() || !target.endsWith("apple-darwin"))) delete env[key];
}
execFileSync(process.execPath, ["node_modules/@tauri-apps/cli/tauri.js", "build", "--ci", "--target", target, "--", "--locked"], { env, stdio: "inherit" });
