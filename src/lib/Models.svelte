<script>
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  let models = $state([]);
  let downloading = $state("");
  let verifying = $state("");
  let progress = $state({});
  let error = $state("");
  let discoveryError = $state("");
  let watcherError = $state("");
  let loading = $state(true);
  let removing = $state("");
  let cancelling = $state("");
  let refreshRequest = 0;
  let downloadRevision = 0;
  let disposed = false;
  const busy = () => loading || Boolean(verifying) || Boolean(removing);

  async function refresh() {
    const request = ++refreshRequest;
    const observedDownload = downloadRevision;
    loading = true;
    try {
      const [availableModels, activeDownload] = await Promise.all([
        invoke("list_local_models"),
        // Download recovery is helpful, but it must never make the model
        // library itself unavailable if the process is still registering its
        // downloader state during app startup.
        invoke("active_local_model_download").then((id) => ({ id })).catch(() => null),
      ]);
      if (disposed || request !== refreshRequest) return;
      models = availableModels;
      discoveryError = "";
      // A download event is newer than the snapshot requested above. Never
      // resurrect a finished transfer or hide one that started during I/O.
      if (observedDownload === downloadRevision && activeDownload) downloading = activeDownload.id || "";
    }
    catch (message) { if (!disposed && request === refreshRequest) discoveryError = `Couldn't load your local models. Nothing was changed. ${String(message)}`; }
    finally { if (!disposed && request === refreshRequest) loading = false; }
  }
  async function download(id) {
    if (busy()) return;
    if (downloading) {
      error = "A model download is already in progress. Wait for it to finish or cancel it before starting another.";
      return;
    }
    const revision = ++downloadRevision;
    downloading = id; error = "";
    delete progress[id];
    try { await invoke("download_local_model", { id }); }
    catch (message) {
      if (disposed || revision !== downloadRevision) return;
      downloading = "";
      error = `Download didn't start. Your existing model is safe. ${String(message)}`;
    }
  }
  async function remove(id) {
    if (busy() || downloading) return;
    error = "";
    removing = id;
    try { await invoke("delete_local_model", { id }); await refresh(); }
    catch (message) { if (!disposed) error = `Couldn't remove that model. ${String(message)}`; }
    finally { removing = ""; }
  }
  async function select(id) {
    if (busy() || downloading) return;
    error = "";
    verifying = id;
    try { await invoke("set_active_local_model", { id }); await refresh(); }
    catch (message) { if (!disposed) error = `Verification didn't finish, so Flick won't use this model. ${String(message)}`; }
    finally { if (verifying === id) verifying = ""; }
  }
  async function cancel(id) {
    if (cancelling || downloading !== id) return;
    cancelling = id;
    error = "";
    try { await invoke("cancel_local_model_download", { id }); }
    catch (message) { if (!disposed) { error = String(message); cancelling = ""; } }
  }
  const size = (bytes) => `${(bytes / 1024 / 1024).toFixed(0)} MB`;
  const languageBadge = (model) => {
    if (!model.supported_languages?.length) return model.id.startsWith("custom:") ? "Language capability checked at load" : "Multilingual";
    return model.supported_languages.length === 1 ? `${model.supported_languages[0].toUpperCase()} only` : `${model.supported_languages.length} languages`;
  };
  const installedSize = () => models
    .filter((model) => model.installed)
    .reduce((total, model) => total + (model.size_bytes || 0), 0);
  const downloadPercent = (model) => {
    const transfer = progress[model.id];
    if (!transfer?.total) return null;
    return Math.min(100, Math.round((transfer.received / transfer.total) * 100));
  };
  const readiness = (model) => {
    if (verifying === model.id) return { label: "Verifying integrity…", tone: "verifying" };
    if (downloading === model.id) {
      const percent = downloadPercent(model);
      return { label: percent === null ? "Preparing download…" : `Downloading ${percent}%`, tone: "downloading" };
    }
    if (model.active) return { label: "In use · ready", tone: "ready" };
    if (model.installed) return { label: "Ready on this computer", tone: "ready" };
    if (model.available_locally) return { label: "On disk · needs verification", tone: "attention" };
    return { label: "Not installed", tone: "muted" };
  };
  onMount(() => {
    let unlisten;
    void (async () => {
      try {
        const dispose = await listen("flick://model-download", ({ payload }) => {
          if (!payload?.id) return;
          downloadRevision += 1;
          if (payload.state === "complete") {
            if (downloading === payload.id) downloading = "";
            if (cancelling === payload.id) cancelling = "";
            delete progress[payload.id];
            void refresh();
            return;
          }
          if (payload.state === "failed") {
            if (downloading === payload.id) downloading = "";
            if (cancelling === payload.id) cancelling = "";
            delete progress[payload.id];
            error = `Download didn't finish. Your existing model is safe. ${payload.message || "Please try again."}`;
            return;
          }
          downloading = payload.id;
          if (payload.state === "started") delete progress[payload.id];
          if (payload.received !== undefined) progress[payload.id] = payload;
        });
        if (disposed) dispose();
        else unlisten = dispose;
      } catch (message) {
        watcherError = `Couldn't watch model downloads. ${String(message)}`;
      } finally {
        if (!disposed) void refresh();
      }
    })();
    return () => { disposed = true; unlisten?.(); };
  });
</script>

<section class="models">
  <div class="heading"><div><h2>Speech models</h2><p>Pick one model to dictate without an internet connection. Flick verifies every download before it can be used. {models.some((model) => model.installed) ? `${size(installedSize())} ready on this computer.` : "Start with Tiny English for the fastest setup."}</p></div><button onclick={refresh} disabled={loading}>{loading ? "Checking…" : "Refresh"}</button></div>
  {#if discoveryError || error || watcherError}<p class="error" role="alert">{discoveryError || error || watcherError}</p>{/if}
  {#if loading}<div class="loading" aria-live="polite">Checking what is already on this computer…</div>{/if}
  {#each models as model}
    {@const status = readiness(model)}
    <article>
      <div><strong>{model.name}</strong><p>{model.description}</p><span>{model.engine} · {model.language} · {size(model.size_bytes)}</span><div class="capabilities"><span class="capability local">On device</span><span class="capability">{languageBadge(model)}</span>{#if model.supports_language_detection}<span class="capability detection">Auto-detect</span>{/if}{#if model.supports_translation}<span class="capability translation">English translation</span>{:else}<span class="capability muted">Transcription only</span>{/if}</div><span class:ready={status.tone === "ready"} class:attention={status.tone === "attention"} class:downloading={status.tone === "downloading" || status.tone === "verifying"} class="model-status" role="status">{status.label}</span></div>
      {#if model.installed}
        {#if model.active}<span class="active">In use</span>{:else}<div class="actions"><button onclick={() => select(model.id)} disabled={busy() || Boolean(downloading)}>{verifying === model.id ? "Verifying…" : "Use"}</button><button class="remove" onclick={() => remove(model.id)} disabled={busy() || Boolean(downloading)}>{removing === model.id ? "Removing…" : "Remove"}</button></div>{/if}
      {:else if model.available_locally}
        <div class="actions"><button onclick={() => select(model.id)} disabled={busy() || Boolean(downloading)}>{verifying === model.id ? "Verifying…" : "Verify & use"}</button><button class="remove" onclick={() => remove(model.id)} disabled={busy() || Boolean(downloading)}>{removing === model.id ? "Removing…" : "Remove"}</button></div>
      {:else}
        {#if downloading === model.id}<button class="remove" onclick={() => cancel(model.id)} disabled={Boolean(cancelling)}>{cancelling === model.id ? "Cancelling…" : "Cancel"}</button>{:else}<button class="download" onclick={() => download(model.id)} disabled={Boolean(downloading) || busy()}>Download</button>{/if}
      {/if}
      {#if progress[model.id] && downloading === model.id}<progress value={progress[model.id].received} max={progress[model.id].total} aria-label={`Downloading ${model.name}`} aria-valuetext={`${downloadPercent(model) ?? 0}% downloaded`}></progress>{/if}
    </article>
  {/each}
</section>

<style>
  .models{display:flex;flex-direction:column;gap:12px}.heading{display:flex;justify-content:space-between;gap:14px;align-items:start}.heading h2{font-size:.98rem}.heading p,article p,article span{color:var(--text-secondary);font-size:.8rem;line-height:1.45}.heading button,article button{border:1px solid var(--border);background:var(--bg-elevated);color:var(--text-primary);border-radius:7px;padding:7px 10px;cursor:pointer}.heading button:disabled,article button:disabled{opacity:.58;cursor:not-allowed}.download{background:#f2f4f8!important;color:#101113!important;border-color:#f2f4f8!important}.remove{color:var(--error)!important}.active{color:var(--accent)!important;font-weight:600}.actions{display:flex;gap:6px}.loading{padding:11px 12px;border:1px solid var(--border);border-radius:8px;color:var(--text-secondary);font-size:.82rem}article{display:grid;grid-template-columns:1fr auto;gap:10px;align-items:center;border:1px solid var(--border);padding:14px;border-radius:8px}article p{margin:5px 0}.error{color:var(--error);font-size:.8rem}progress{grid-column:1 / -1;width:100%;accent-color:var(--accent)}.capabilities{display:flex;flex-wrap:wrap;gap:5px;margin-top:9px}.capability{display:inline-flex;padding:2px 6px;border:1px solid var(--border);border-radius:999px;color:var(--text-muted);background:var(--bg-primary);font-size:.68rem;line-height:1.25}.capability.local{color:var(--success);border-color:rgba(121,217,159,.24);background:var(--success-dim)}.capability.detection{color:#dcb572;border-color:rgba(220,181,114,.28);background:rgba(220,181,114,.1)}.capability.translation{color:var(--accent);border-color:rgba(143,183,255,.26);background:var(--accent-glow)}.capability.muted{color:var(--text-secondary)}.model-status{display:inline-flex;margin-top:10px;font-size:.74rem;color:var(--text-muted)}.model-status.ready{color:var(--success)}.model-status.attention{color:#dcb572}.model-status.downloading{color:var(--accent)}
</style>
