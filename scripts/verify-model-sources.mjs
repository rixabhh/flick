// Read-only audit of pinned upstream file metadata, without downloading weights.
import { buildCatalog } from './generate-model-catalog.mjs';
const groups = new Map();
for (const model of buildCatalog()) {
  const [, repo, revision, file] = new URL(model.url).pathname.match(/^\/(.+)\/resolve\/([^/]+)\/(.+)$/);
  const key = `${repo}/revision/${revision}`;
  if (!groups.has(key)) groups.set(key, []);
  groups.get(key).push({ ...model, remoteFile: file });
}
let checked = 0;
const queue = [...groups.entries()];
await Promise.all(Array.from({ length: 4 }, async () => {
  while (queue.length) {
    const [repository, files] = queue.shift();
    const response = await fetch(`https://huggingface.co/api/models/${repository}?blobs=true`, { signal: AbortSignal.timeout(30000) });
    if (!response.ok) throw new Error(`${repository}: HTTP ${response.status}`);
    const metadata = await response.json();
    for (const file of files) {
      const remote = metadata.siblings.find((entry) => entry.rfilename === file.remoteFile);
      if (remote?.lfs?.sha256 !== file.sha256 || remote?.size !== file.size_bytes) throw new Error(`${file.id}: upstream hash/size mismatch`);
      checked += 1;
    }
    console.log(`Verified ${checked} artifacts`);
  }
}));
console.log(`All ${checked} model hashes, sizes and pinned revisions match Hugging Face.`);
