// Exercise the packaged executable's real native engine, not a mocked IPC call.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawn } from 'node:child_process';
import { pipeline } from 'node:stream/promises';
import { Readable } from 'node:stream';
import { buildCatalog } from './generate-model-catalog.mjs';

const target = process.env.FLICK_BUILD_TARGET;
if (!target) throw new Error('FLICK_BUILD_TARGET is required');
const executable = path.resolve('src-tauri', 'target', target, 'release', process.platform === 'win32' ? 'flick.exe' : 'flick');
if (!fs.existsSync(executable)) throw new Error(`Executable missing: ${executable}`);
const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'flick-engine-smoke-'));
try {
  const sampleResponse = await fetch('https://raw.githubusercontent.com/ggml-org/whisper.cpp/b0a11594aec50892a02cd8d129eee2dfe93a8bb8/samples/jfk.wav', { signal: AbortSignal.timeout(30000) });
  if (!sampleResponse.ok) throw new Error(`Sample download: HTTP ${sampleResponse.status}`);
  const wav = Buffer.from(await sampleResponse.arrayBuffer());
  if (wav.toString('ascii', 0, 4) !== 'RIFF' || wav.toString('ascii', 8, 12) !== 'WAVE') throw new Error('Invalid smoke-test WAV');
  let audio;
  let format;
  for (let offset = 12; offset + 8 <= wav.length;) {
    const length = wav.readUInt32LE(offset + 4);
    const id = wav.toString('ascii', offset, offset + 4);
    if (id === 'fmt ') format = { type: wav.readUInt16LE(offset + 8), channels: wav.readUInt16LE(offset + 10), rate: wav.readUInt32LE(offset + 12), bits: wav.readUInt16LE(offset + 22) };
    if (id === 'data') audio = wav.subarray(offset + 8, offset + 8 + length);
    offset += 8 + length + (length % 2);
  }
  if (!audio || format?.type !== 1 || format.channels !== 1 || format.rate !== 16000 || format.bits !== 16) throw new Error('Expected 16kHz mono PCM16 WAV');
  const pcm = Buffer.alloc(audio.length * 2);
  for (let i = 0; i < audio.length / 2; i++) pcm.writeFloatLE(audio.readInt16LE(i * 2) / 32768, i * 4);

  const runWorker = (modelPath) => new Promise((resolve, reject) => {
    const child = spawn(executable, ['--flick-transcribe-worker', modelPath, 'en', 'false'], { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
    const output = [];
    let bytes = 0;
    let diagnostics = '';
    const timer = setTimeout(() => { child.kill(); reject(new Error('Native transcription timed out')); }, 300000);
    child.on('error', (error) => { clearTimeout(timer); reject(error); });
    child.stdout.on('data', (chunk) => { bytes += chunk.length; if (bytes > 1024 * 1024) { child.kill(); reject(new Error('Excessive worker output')); } else output.push(chunk); });
    child.stderr.on('data', (chunk) => { diagnostics = (diagnostics + chunk).slice(-6000); });
    child.stdin.on('error', () => {}); // A native crash is reported by exit below.
    child.on('close', (code) => {
      clearTimeout(timer);
      if (code !== 0) return reject(new Error(`Worker exited ${code}: ${diagnostics}`));
      const envelope = Buffer.concat(output).toString().split('\n').reverse().map((line) => { try { return JSON.parse(line); } catch { return null; } }).find((entry) => entry?.flick_worker === 1);
      if (!envelope) return reject(new Error(`Missing worker response: ${diagnostics}`));
      resolve(envelope);
    });
    child.stdin.end(pcm);
  });
  const invalid = path.join(directory, 'invalid.gguf');
  fs.writeFileSync(invalid, 'not a speech model');
  const failed = await runWorker(invalid);
  if (!failed.error) throw new Error('Invalid models must return a recoverable error');
  console.log('Invalid model produced a recoverable worker error.');
  for (const id of ['whisper-tiny-en', 'moonshine-tiny-q8', 'parakeet-tdt_ctc-110m-q8_0']) {
    const model = buildCatalog().find((entry) => entry.id === id);
    if (!model) throw new Error(`Unknown smoke model: ${id}`);
    const filename = path.join(directory, model.file_name);
    const response = await fetch(model.url, { signal: AbortSignal.timeout(240000) });
    if (!response.ok) throw new Error(`${id} download: HTTP ${response.status}`);
    await pipeline(Readable.fromWeb(response.body), fs.createWriteStream(filename));
    const hash = crypto.createHash('sha256');
    for await (const chunk of fs.createReadStream(filename)) hash.update(chunk);
    if (hash.digest('hex') !== model.sha256 || fs.statSync(filename).size !== model.size_bytes) throw new Error(`${id}: integrity check failed`);
    const result = await runWorker(filename);
    if (!/ask\W+not/i.test(result.text || '')) throw new Error(`${id}: actual transcription failed: ${result.error || result.text}`);
    console.log(`${id}: real audio transcription passed`);
  }
} finally {
  // mkdtemp owns this exact, freshly allocated directory; no user data lives here.
  fs.rmSync(directory, { recursive: true, force: true });
}
