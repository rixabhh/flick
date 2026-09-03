<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let entries = $state([]);
  let error = $state("");
  let query = $state("");
  const date = (seconds) => new Date(seconds * 1000).toLocaleString();
  async function load() { try { entries = await invoke("get_history", { limit: 100, query }); } catch (message) { error = String(message); } }
  async function saved(entry) { try { await invoke("set_history_saved", { id: entry.id, saved: !entry.saved }); await load(); } catch (message) { error = String(message); } }
  async function copy(entry) { try { await invoke("copy_history_entry", { id: entry.id }); } catch (message) { error = String(message); } }
  async function clear() { if (!confirm("Delete all local Flick history? This cannot be undone.")) return; try { await invoke("clear_history"); await load(); } catch (message) { error = String(message); } }
  onMount(load);
</script>

<section class="history"><div class="heading"><div><h2>Local history</h2><p>Only enabled local activity is shown here.</p></div><button onclick={clear} disabled={!entries.length}>Clear</button></div><input class="search" type="search" bind:value={query} oninput={load} placeholder="Search local history" aria-label="Search local history" />{#if error}<p class="error">{error}</p>{/if}{#if !entries.length}<p class="empty">{query ? "No matching activity." : "No saved activity yet."}</p>{/if}{#each entries as entry}<article><div><span>{entry.kind} · {date(entry.created_at)}</span><p>{entry.text}</p></div><div class="actions"><button onclick={() => copy(entry)}>Copy</button><button class:saved={entry.saved} onclick={() => saved(entry)}>{entry.saved ? "Saved" : "Save"}</button></div></article>{/each}</section>

<style>
  .history{display:flex;flex-direction:column;gap:10px}.heading{display:flex;justify-content:space-between;align-items:start}.heading h2{font-size:.98rem}.heading p,article span,.empty{color:var(--text-secondary);font-size:.8rem;margin:4px 0}.heading button,article button{background:var(--bg-elevated);border:1px solid var(--border);border-radius:7px;color:var(--text-primary);padding:6px 10px;cursor:pointer}.heading button:disabled{opacity:.4}.search{width:100%;background:var(--bg-primary);border:1px solid var(--border);border-radius:7px;padding:8px;color:var(--text-primary)}article{display:flex;justify-content:space-between;gap:10px;padding:12px;border:1px solid var(--border);border-radius:8px}article p{margin:5px 0 0;white-space:pre-wrap}.actions{display:flex;gap:6px;align-items:start}.saved{color:var(--success)!important}.error{color:var(--error);font-size:.8rem}
</style>
