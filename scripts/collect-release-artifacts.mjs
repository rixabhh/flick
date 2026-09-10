import { createReadStream } from "node:fs";
import { readdir, mkdir, copyFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";

const target = process.env.FLICK_BUILD_TARGET;
const targets = ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc", "x86_64-apple-darwin", "aarch64-apple-darwin"];
if (!targets.includes(target)) throw new Error("Unknown release target");
const root = path.resolve("src-tauri", "target", target, "release", "bundle");
const destination = path.resolve("release-artifacts", target);
await mkdir(destination, { recursive: true });
const checksums = [];
for (const directory of await readdir(root, { withFileTypes: true })) {
  if (!directory.isDirectory()) continue;
  for (const file of await readdir(path.join(root, directory.name), { withFileTypes: true })) {
    if (!file.isFile() || !/\.(msi|exe|dmg|deb|rpm|AppImage)$/.test(file.name)) continue;
    const source = path.join(root, directory.name, file.name);
    const hash = createHash("sha256");
    let bytes = 0;
    for await (const chunk of createReadStream(source)) { hash.update(chunk); bytes += chunk.length; }
    if (!bytes) throw new Error(`Empty installer: ${file.name}`);
    await copyFile(source, path.join(destination, file.name));
    checksums.push(`${hash.digest("hex")}  ${file.name}`);
  }
}
if (!checksums.length) throw new Error(`No installers were produced for ${target}`);
await writeFile(path.join(destination, `SHA256SUMS-${target}.txt`), checksums.sort().join("\n") + "\n");
console.log(`Collected ${checksums.length} installers with SHA-256 checksums for ${target}.`);
