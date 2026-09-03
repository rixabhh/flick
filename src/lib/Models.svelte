<script>
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  let models = $state([]);
  let downloading = $state("");
  let progress = $state({});
  let error = $state("");

  async function refresh() {
    try { models = await invoke("list_local_models"); }
    catch (message) { error = String(message); }
  }
  async function download(id) {
    downloading = id; error = "";
    try { await invoke("download_local_model", { id }); await refresh(); }
    catch (message) { error = String(message); }
    finally { downloading = ""; }
  }
  async function remove(id) {
    error = "";
    try { await invoke("delete_local_model", { id }); await refresh(); }
    catch (message) { error = String(message); }
  }
  async function select(id) {
    error = "";
    try { await invoke("set_active_local_model", { id }); await refresh(); }
    catch (message) { error = String(message); }
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
    listen("flick://model-download", ({ payload }) => progress[payload.id] = payload)
      .then((dispose) => unlisten = dispose);
    return () => unlisten?.();
  });
</script>

<section class="models">
  <div class="heading"><div><h2>Local speech models</h2><p>Models run on your computer after a verified one-time download. {models.some((model) => model.installed) ? `${size(installedSize())} installed locally.` : "No local model installed yet."}</p></div><button onclick={refresh}>Refresh</button></div>
  {#if error}<p class="error">{error}</p>{/if}
  {#each models as model}
    <article>
      <div><strong>{model.name}</strong><p>{model.description}</p><span>{model.language} · {size(model.size_bytes)}</span></div>
      {#if model.installed}
        {#if model.active}<span class="active">In use</span>{:else}<div class="actions"><button onclick={() => select(model.id)}>Use</button><button class="remove" onclick={() => remove(model.id)}>Remove</button></div>{/if}
      {:else}
        {#if downloading === model.id}<button class="remove" onclick={() => cancel(model.id)}>Cancel</button>{:else}<button class="download" onclick={() => download(model.id)}>Download</button>{/if}
      {/if}
      {#if progress[model.id] && downloading === model.id}<progress value={progress[model.id].received} max={progress[model.id].total}></progress>{/if}
    </article>
  {/each}
</section>

<style>
  .models{display:flex;flex-direction:column;gap:12px}.heading{display:flex;justify-content:space-between;gap:14px;align-items:start}.heading h2{font-size:.98rem}.heading p,article p,article span{color:var(--text-secondary);font-size:.8rem;line-height:1.45}.heading button,article button{border:1px solid var(--border);background:var(--bg-elevated);color:var(--text-primary);border-radius:7px;padding:7px 10px;cursor:pointer}.download{background:#f2f4f8!important;color:#101113!important;border-color:#f2f4f8!important}.remove{color:var(--error)!important}.active{color:var(--accent)!important;font-weight:600}.actions{display:flex;gap:6px}article{display:grid;grid-template-columns:1fr auto;gap:10px;align-items:center;border:1px solid var(--border);padding:14px;border-radius:8px}article p{margin:5px 0}.error{color:var(--error);font-size:.8rem}progress{grid-column:1 / -1;width:100%;accent-color:var(--accent)}
</style>
