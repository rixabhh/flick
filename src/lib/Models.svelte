<script>
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  let models = $state([]);
  let downloading = $state("");
  let progress = $state({});
  let error = $state("");
  let loading = $state(true);

  async function refresh() {
    loading = true;
    try { models = await invoke("list_local_models"); }
    catch (message) { error = `Couldn't load your local models. Nothing was changed. ${String(message)}`; }
    finally { loading = false; }
  }
  async function download(id) {
    downloading = id; error = "";
    try { await invoke("download_local_model", { id }); }
    catch (message) { downloading = ""; error = `Download didn't start. Your existing model is safe. ${String(message)}`; }
  }
  async function remove(id) {
    error = "";
    try { await invoke("delete_local_model", { id }); await refresh(); }
    catch (message) { error = `Couldn't remove that model. ${String(message)}`; }
  }
  async function select(id) {
    error = "";
    try { await invoke("set_active_local_model", { id }); await refresh(); }
    catch (message) { error = `Verification didn't finish, so Flick won't use this model. ${String(message)}`; }
  }
  async function cancel(id) {
    try { await invoke("cancel_local_model_download", { id }); }
    catch (message) { error = String(message); }
  }
  const size = (bytes) => `${(bytes / 1024 / 1024).toFixed(0)} MB`;
  const installedSize = () => models
    .filter((model) => model.installed)
    .reduce((total, model) => total + (model.size_bytes || 0), 0);
  onMount(() => {
    refresh();
    let unlisten;
    listen("flick://model-download", ({ payload }) => {
      if (!payload?.id) return;
      if (payload.state === "complete") {
        downloading = "";
        delete progress[payload.id];
        void refresh();
        return;
      }
      if (payload.state === "failed") {
        downloading = "";
        error = `Download didn't finish. Your existing model is safe. ${payload.message || "Please try again."}`;
        return;
      }
      if (payload.received !== undefined) progress[payload.id] = payload;
    }).then((dispose) => unlisten = dispose);
    return () => unlisten?.();
  });
</script>

<section class="models">
  <div class="heading"><div><h2>Speech models</h2><p>Pick one model to dictate without an internet connection. Flick verifies every download before it can be used. {models.some((model) => model.installed) ? `${size(installedSize())} ready on this computer.` : "Start with Tiny English for the fastest setup."}</p></div><button onclick={refresh} disabled={loading}>{loading ? "Checking…" : "Refresh"}</button></div>
  {#if error}<p class="error">{error}</p>{/if}
  {#if loading}<div class="loading" aria-live="polite">Checking what is already on this computer…</div>{/if}
  {#each models as model}
    <article>
      <div><strong>{model.name}</strong><p>{model.description}</p><span>{model.language} · {size(model.size_bytes)}</span></div>
      {#if model.installed}
        {#if model.active}<span class="active">In use</span>{:else}<div class="actions"><button onclick={() => select(model.id)}>Use</button><button class="remove" onclick={() => remove(model.id)}>Remove</button></div>{/if}
      {:else if model.available_locally}
        <div class="actions"><button onclick={() => select(model.id)}>Verify & use</button><button class="remove" onclick={() => remove(model.id)}>Remove</button></div>
      {:else}
        {#if downloading === model.id}<button class="remove" onclick={() => cancel(model.id)}>Cancel</button>{:else}<button class="download" onclick={() => download(model.id)}>Download</button>{/if}
      {/if}
      {#if progress[model.id] && downloading === model.id}<progress value={progress[model.id].received} max={progress[model.id].total}></progress>{/if}
    </article>
  {/each}
</section>

<style>
  .models{display:flex;flex-direction:column;gap:12px}.heading{display:flex;justify-content:space-between;gap:14px;align-items:start}.heading h2{font-size:.98rem}.heading p,article p,article span{color:var(--text-secondary);font-size:.8rem;line-height:1.45}.heading button,article button{border:1px solid var(--border);background:var(--bg-elevated);color:var(--text-primary);border-radius:7px;padding:7px 10px;cursor:pointer}.heading button:disabled{opacity:.65;cursor:wait}.download{background:#f2f4f8!important;color:#101113!important;border-color:#f2f4f8!important}.remove{color:var(--error)!important}.active{color:var(--accent)!important;font-weight:600}.actions{display:flex;gap:6px}.loading{padding:11px 12px;border:1px solid var(--border);border-radius:8px;color:var(--text-secondary);font-size:.82rem}article{display:grid;grid-template-columns:1fr auto;gap:10px;align-items:center;border:1px solid var(--border);padding:14px;border-radius:8px}article p{margin:5px 0}.error{color:var(--error);font-size:.8rem}progress{grid-column:1 / -1;width:100%;accent-color:var(--accent)}
</style>
