<script>
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { translate } from "./i18n.js";

  const tones = ["Casual", "Professional", "Warm", "Concise", "Assertive", "Custom"];
  let context = $state("");
  let instruction = $state("");
  let draft = $state("");
  let tone = $state("Warm");
  let customTone = $state("");
  let loading = $state(false);
  let error = $state("");
  let copied = $state(false);
  let providerNotice = $state("Provider details are loading…");
  let language = $state("en");

  const t = (key) => translate(language, key);

  const toneValue = () => tone === "Custom" ? (customTone.trim() || "friendly") : tone.toLowerCase();

  async function captureSelection() {
    error = "";
    try { context = await invoke("capture_reply_context"); }
    catch (message) { error = String(message); }
  }

  async function generate() {
    error = "";
    loading = true;
    try {
      draft = await invoke("generate_reply", { context, tone: toneValue(), instruction });
    } catch (message) { error = String(message); }
    finally { loading = false; }
  }

  async function copy() {
    error = "";
    try {
      await invoke("copy_reply", { draft });
      copied = true;
      setTimeout(() => copied = false, 1600);
    } catch (message) { error = String(message); }
  }

  async function insert() {
    error = "";
    try {
      await invoke("insert_reply", { draft });
    } catch (message) { error = `${message} Your draft is still here; use Copy instead.`; }
  }

  function describeProvider(config) {
    if (config.provider === "custom") return language === "es" ? "El extremo compatible con OpenAI recibe el contexto solo cuando generas." : "A configured OpenAI-compatible endpoint receives context only when you generate.";
    if (config.provider === "openrouter") return language === "es" ? "OpenRouter recibe el contexto solo cuando generas." : "OpenRouter receives context only when you generate.";
    return language === "es" ? "Gemini recibe el contexto solo cuando generas." : "Gemini receives context only when you generate.";
  }

  onMount(() => {
    let disposed = false;
    let unlisten = () => {};
    void (async () => {
      try {
        const config = await invoke("get_config");
        language = config.app_language === "es" ? "es" : "en";
        providerNotice = describeProvider(config);
      } catch {
        providerNotice = language === "es" ? "Tu proveedor configurado recibe el contexto solo cuando generas." : "Your configured provider receives context only when you generate.";
      }
      const dispose = await listen("flick://composer-context", (event) => {
        context = String(event.payload || "");
        draft = "";
        error = context ? "" : "No selected text was found. Add context manually.";
      });
      if (disposed) dispose();
      else unlisten = dispose;
    })();
    return () => { disposed = true; unlisten(); };
  });
</script>

<main class="composer">
  <header>
    <div><span class="eyebrow">FLICK REPLY</span><h1>{t("composer.title")}</h1></div>
    <button class="icon" aria-label={t("composer.close")} onclick={() => getCurrentWindow().hide()}>×</button>
  </header>
  <p class="privacy">Only the text below is used as context. It is never saved by Flick. {providerNotice}</p>

  <label for="context">{t("composer.context")} <button class="link" onclick={captureSelection}>{t("composer.capture")}</button></label>
  <textarea id="context" class="context" bind:value={context} placeholder={t("composer.contextPlaceholder")}></textarea>

  <span class="label">{t("composer.tone")}</span>
  <div class="tones">
    {#each tones as item}<button class:active={tone === item} onclick={() => tone = item}>{t(`tone.${item}`)}</button>{/each}
  </div>
  {#if tone === "Custom"}<input bind:value={customTone} placeholder={t("composer.customTone")} />{/if}

  <label for="intent">{t("composer.intent")}</label>
  <textarea id="intent" bind:value={instruction} placeholder={t("composer.intentPlaceholder")}></textarea>
  <button class="generate" onclick={generate} disabled={loading || !context.trim() || !instruction.trim()}>{loading ? t("composer.drafting") : draft ? t("composer.regenerate") : t("composer.generate")}</button>

  {#if error}<p class="error">{error}</p>{/if}
  {#if draft}
    <label for="draft">{t("composer.draft")}</label>
    <textarea id="draft" class="draft" bind:value={draft}></textarea>
    <div class="actions"><button class="secondary" onclick={copy}>{copied ? t("composer.copied") : t("composer.copy")}</button><button class="insert" onclick={insert}>{t("composer.insert")}</button></div>
  {/if}
</main>

<style>
  .composer { height: 100vh; overflow:auto; padding:22px; background:var(--bg-primary); color:var(--text-primary); font-family:var(--font-body); }
  header,.actions { display:flex; align-items:center; justify-content:space-between; gap:12px; } h1{font-size:1.2rem;margin:3px 0 0}.eyebrow{font-size:.68rem;letter-spacing:.12em;color:var(--accent)}
  .privacy{color:var(--text-secondary);font-size:.8rem;margin:16px 0}.icon{border:0;background:transparent;color:var(--text-muted);font-size:1.7rem;cursor:pointer}.composer label,.label{display:block;font-size:.78rem;font-weight:700;margin:14px 0 6px;color:var(--text-secondary)}
  textarea,input{font:inherit;background:var(--bg-surface);border:1px solid var(--border);border-radius:8px;color:var(--text-primary);padding:10px;width:100%;resize:vertical}.context{min-height:72px}.draft{min-height:95px}.tones{display:flex;flex-wrap:wrap;gap:6px}.tones button,.secondary,.link{background:var(--bg-elevated);border:1px solid var(--border);border-radius:999px;color:var(--text-secondary);padding:6px 10px;cursor:pointer;font-size:.78rem}.tones button.active{background:var(--accent-glow);border-color:var(--accent);color:var(--accent)}.link{float:right;padding:0;border:0;background:transparent;color:var(--accent)}
  .generate,.insert{width:100%;margin-top:14px;border:0;border-radius:8px;padding:10px;background:var(--accent);color:var(--bg-primary);font-weight:700;cursor:pointer}.generate:disabled{opacity:.45;cursor:not-allowed}.error{color:var(--error);font-size:.78rem;margin:10px 0}.actions{margin-top:8px}.actions button{margin-top:0}.secondary{flex:1}.insert{flex:2}
</style>
