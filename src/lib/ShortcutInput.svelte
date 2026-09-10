<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onDestroy } from "svelte";
  import { normalizeShortcut, shortcutFromEvent } from "./shortcuts.js";
  let { label, value, onChange, conflicts = [] } = $props();
  let recording = $state(false);
  let saving = $state(false);
  let error = $state("");
  let timer;
  async function stop() {
    recording = false;
    clearTimeout(timer);
    await invoke("set_shortcut_capture", { active: false }).catch(() => {});
  }
  async function begin() {
    error = "";
    try {
      await invoke("set_shortcut_capture", { active: true });
      recording = true;
      clearTimeout(timer);
      timer = setTimeout(stop, 25000);
    } catch (message) { error = `Could not record a shortcut. ${message}`; }
  }
  async function capture(event) {
    if (!recording) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") { await stop(); return; }
    try {
      const next = shortcutFromEvent(event);
      if (!next) return;
      if (conflicts.some((other) => { try { return normalizeShortcut(other) === next; } catch { return false; } })) throw new Error("That shortcut is already used by another Flick action.");
      saving = true;
      await onChange(next);
      // Keep the lease until key-up so the captured chord cannot immediately
      // trigger its newly assigned action through the native hook.
      error = "";
      clearTimeout(timer);
      timer = setTimeout(stop, 250);
    } catch (message) { error = String(message.message || message); }
    finally { saving = false; }
  }
  onDestroy(() => { void stop(); });
</script>

<div class="shortcut-field">
  <span>{label}</span>
  <button type="button" class:recording disabled={saving} aria-label={`${label}: ${value}. Click to change`} onclick={begin} onkeydown={capture} onblur={stop}>
    {saving ? "Saving…" : recording ? "Press your shortcut…" : value}
  </button>
  <small>{recording ? "Escape to cancel. Include Ctrl, Alt/Option, or Command." : "Click to record your own shortcut. Changes apply immediately."}</small>
  {#if error}<small class="error" role="alert">{error}</small>{/if}
</div>

<style>
  .shortcut-field { display:grid; gap:7px; margin:16px 0; font-size:12px; }
  button { justify-self:start; min-width:150px; padding:9px 14px; border:1px solid var(--border); border-radius:9px; background:var(--bg-elevated); color:var(--text-primary); font:600 12px ui-monospace,monospace; cursor:pointer; }
  button.recording { border-color:var(--accent); box-shadow:0 0 0 3px var(--accent-glow); }
  small { color:var(--text-secondary); font-size:11px; }.error { color:var(--error); }
</style>
