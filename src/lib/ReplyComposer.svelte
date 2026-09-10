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
  let capturing = $state(false);
  let inserting = $state(false);
  let error = $state("");
  let copied = $state(false);
  let copying = $state(false);
  let draftSignature = $state("");
  let providerNotice = $state("");
  let language = $state("en");
  let contextInput = $state();
  let generateShortcut = $state("⌘ ↵");
  let session = 0;
  let copiedTimer;

  const t = (key) => translate(language, key);
  const toneValue = () => tone === "Custom" ? (customTone.trim() || "friendly") : tone.toLowerCase();
  const requestSignature = () => `${context}\u0000${toneValue()}\u0000${instruction}`;
  const draftMatchesRequest = () => Boolean(draft) && draftSignature === requestSignature();

  async function captureSelection() {
    if (capturing || loading || inserting) return;
    const currentSession = session;
    const previousContext = context;
    error = "";
    capturing = true;
    try {
      const selection = await invoke("capture_reply_context");
      if (currentSession === session && context === previousContext) context = selection;
    }
    catch (message) { if (currentSession === session) error = String(message); }
    finally { if (currentSession === session) capturing = false; }
  }

  async function generate() {
    if (loading || capturing || inserting || !context.trim() || !instruction.trim()) return;
    const currentSession = session;
    error = "";
    copied = false;
    loading = true;
    const signature = requestSignature();
    try {
      const result = await invoke("generate_reply", { context, tone: toneValue(), instruction });
      if (currentSession !== session) return;
      draft = result;
      // A user can keep typing while the provider is responding. Associate
      // the result with the exact request that left Flick, not whatever is in
      // the fields when the promise later resolves.
      draftSignature = signature;
    }
    catch (message) { if (currentSession === session) error = String(message); }
    finally { if (currentSession === session) loading = false; }
  }

  async function copy() {
    if (copying || loading || inserting || !draft) return;
    const currentSession = session;
    const text = draft;
    error = "";
    copying = true;
    try {
      await invoke("copy_reply", { draft: text });
      if (currentSession !== session || text !== draft) return;
      copied = true;
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => copied = false, 1600);
    } catch (message) { if (currentSession === session) error = String(message); }
    finally { if (currentSession === session) copying = false; }
  }

  async function insert() {
    if (inserting || capturing || loading || copying) return;
    const currentSession = session;
    error = "";
    if (!draftMatchesRequest()) {
      error = t("composer.staleError");
      return;
    }
    inserting = true;
    try { await invoke("insert_reply", { draft }); }
    catch (message) { if (currentSession === session) error = `${message} ${t("composer.insertRecovery")}`; }
    finally { if (currentSession === session) inserting = false; }
  }

  function invalidateSession() {
    session += 1;
    loading = capturing = inserting = copying = copied = false;
    clearTimeout(copiedTimer);
  }

  async function close() {
    try {
      await getCurrentWindow().hide();
      invalidateSession();
    } catch (message) { error = String(message); }
  }

  function handleKeydown(event) {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault();
      generate();
    }
    if (event.key === "Escape" && !inserting) void close();
  }

  function describeProvider(config) {
    if (config.provider === "custom") return t("composer.providerCustom");
    if (config.provider === "openrouter") return t("composer.providerOpenRouter");
    return t("composer.providerGemini");
  }

  onMount(() => {
    let disposed = false;
    let unlisten = () => {};
    const focusTimer = setTimeout(() => contextInput?.focus(), 0);
    providerNotice = t("composer.providerLoading");
    // The handler supports either modifier. Reflect the user's platform in
    // the compact hint rather than teaching Windows/Linux users a macOS-only
    // shortcut.
    if (!/mac/i.test(navigator.platform || navigator.userAgent)) generateShortcut = "Ctrl ↵";
    // Register the selection channel before the best-effort settings lookup.
    // `open_from_shortcut` captures text before showing this webview, so a
    // cold-start composer must be ready to receive it without waiting on disk
    // I/O for the privacy copy.
    void (async () => {
      const dispose = await listen("flick://composer-context", (event) => {
        invalidateSession();
        const payload = event.payload;
        // Support the previous string payload while the backend and renderer
        // update together. The object form preserves a meaningful recovery
        // reason when selection capture could not safely complete.
        context = typeof payload === "string" ? payload : String(payload?.context || "");
        draft = "";
        draftSignature = "";
        error = typeof payload === "object" && payload?.error
          ? String(payload.error)
          : context ? "" : t("composer.noSelection");
      });
      if (disposed) dispose();
      else unlisten = dispose;
    })().catch((message) => { if (!disposed) error = String(message); });
    void (async () => {
      try {
        const config = await invoke("get_config");
        language = config.app_language === "es" ? "es" : "en";
        providerNotice = describeProvider(config);
      } catch {
        providerNotice = t("composer.providerFallback");
      }
    })();
    return () => { disposed = true; invalidateSession(); unlisten(); clearTimeout(focusTimer); };
  });
</script>

<div class="composer" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="composer-title" aria-busy={loading || capturing || inserting} onkeydown={handleKeydown}>
  <section class="window-surface">
    <header data-tauri-drag-region>
      <div class="title-group" data-tauri-drag-region>
        <span class="composer-mark" aria-hidden="true">↗</span>
        <div data-tauri-drag-region><span class="eyebrow">FLICK REPLY</span><h1 id="composer-title">{t("composer.title")}</h1></div>
      </div>
      <button class="icon" aria-label={t("composer.close")} onclick={close} disabled={inserting}>×</button>
    </header>

    <p class="privacy"><span aria-hidden="true">⌁</span><span>{t("composer.privacy")} {providerNotice}</span></p>

    <section class="field-group">
      <div class="field-label"><label for="context">{t("composer.context")}</label><button class="capture" onclick={captureSelection} disabled={capturing || loading || inserting}>{capturing ? t("composer.capturing") : t("composer.capture")}</button></div>
      <textarea id="context" class="context" bind:this={contextInput} bind:value={context} placeholder={t("composer.contextPlaceholder")}></textarea>
    </section>

    <section class="field-group">
      <span class="label">{t("composer.tone")}</span>
      <div class="tones" role="group" aria-label={t("composer.tone")}>
        {#each tones as item}<button class:active={tone === item} aria-pressed={tone === item} onclick={() => tone = item}>{t(`tone.${item}`)}</button>{/each}
      </div>
      {#if tone === "Custom"}<input bind:value={customTone} placeholder={t("composer.customTone")} />{/if}
    </section>

    <section class="field-group">
      <div class="field-label"><label for="intent">{t("composer.intent")}</label><span class="shortcut">{generateShortcut}</span></div>
      <textarea id="intent" class="intent" bind:value={instruction} placeholder={t("composer.intentPlaceholder")}></textarea>
    </section>

    <button class="generate" onclick={generate} disabled={loading || capturing || inserting || !context.trim() || !instruction.trim()}>
      {#if loading}<span class="mini-spinner" aria-hidden="true"></span>{/if}
      <span>{loading ? t("composer.drafting") : draft ? t("composer.regenerate") : t("composer.generate")}</span>
    </button>

    {#if error}<p class="error" role="alert"><span aria-hidden="true">!</span>{error}</p>{/if}
    {#if draft}
      <section class="draft-card">
        <div class="field-label"><label for="draft">{t("composer.draft")}</label>{#if draftMatchesRequest()}<span class="ready"><i></i> {t("composer.ready")}</span>{:else}<span class="stale" role="status">{t("composer.stale")}</span>{/if}</div>
        <textarea id="draft" class="draft" bind:value={draft} readonly={loading || inserting} oninput={() => copied = false}></textarea>
        <div class="actions"><button class="secondary" onclick={copy} disabled={copying || loading || inserting}>{copied ? t("composer.copied") : t("composer.copy")}</button><button class="insert" onclick={insert} disabled={inserting || copying || capturing || loading || !draftMatchesRequest()} title={draftMatchesRequest() ? undefined : t("composer.staleTooltip")}>{inserting ? t("composer.inserting") : t("composer.insert")}</button></div>
      </section>
    {/if}
  </section>
</div>

<style>
  .composer { height:100vh; overflow:auto; padding:14px; color:#f5f7fb; background:radial-gradient(circle at 20% -10%,rgba(122,158,255,.16),transparent 40%),linear-gradient(145deg,#171a22,#0e1015); font-family:-apple-system,BlinkMacSystemFont,"SF Pro Display","Helvetica Neue",system-ui,sans-serif; }
  .window-surface { min-height:100%; padding:18px; border:1px solid rgba(255,255,255,.12); border-radius:20px; background:linear-gradient(145deg,rgba(42,47,59,.72),rgba(21,23,30,.8)); box-shadow:inset 0 1px rgba(255,255,255,.1),0 20px 50px rgba(0,0,0,.25); backdrop-filter:blur(28px) saturate(1.25); -webkit-backdrop-filter:blur(28px) saturate(1.25); }
  header,.title-group,.field-label,.actions { display:flex; align-items:center; justify-content:space-between; gap:10px; }
  header { padding:1px 0 14px; }
  .title-group { justify-content:flex-start; }
  .composer-mark { display:grid; place-items:center; width:30px; height:30px; border-radius:10px; color:#c9dcff; background:linear-gradient(145deg,rgba(132,170,255,.3),rgba(76,105,186,.17)); border:1px solid rgba(176,204,255,.28); font-size:17px; box-shadow:inset 0 1px rgba(255,255,255,.2); }
  h1 { margin:1px 0 0; font-size:17px; line-height:1.1; letter-spacing:-.025em; }
  .eyebrow { color:#9fc1ff; font-size:9px; font-weight:750; letter-spacing:.14em; }
  .icon { display:grid; place-items:center; width:28px; height:28px; border:1px solid transparent; border-radius:50%; background:rgba(255,255,255,.07); color:rgba(255,255,255,.68); cursor:pointer; font-size:20px; line-height:1; transition:background 150ms ease,transform 150ms ease,color 150ms ease; }
  .icon:hover { color:white; background:rgba(255,107,107,.26); transform:scale(1.05); }
  .privacy { display:flex; gap:8px; margin:0 0 16px; padding:9px 10px; border:1px solid rgba(163,195,255,.13); border-radius:11px; color:rgba(227,234,248,.67); background:rgba(111,149,229,.08); font-size:10px; line-height:1.42; }
  .privacy > :first-child { color:#b8d0ff; font-size:14px; line-height:1; }
  .field-group { margin-top:13px; }
  .field-label { margin-bottom:6px; }
  label,.label { display:block; color:rgba(243,246,253,.76); font-size:11px; font-weight:650; letter-spacing:.01em; }
  textarea,input { width:100%; border:1px solid rgba(255,255,255,.11); border-radius:11px; color:#f5f7fb; background:rgba(8,10,15,.4); box-shadow:inset 0 1px 2px rgba(0,0,0,.15); font:inherit; font-size:12px; line-height:1.45; padding:10px; resize:vertical; transition:border-color 150ms ease,box-shadow 150ms ease,background 150ms ease; }
  textarea:focus,input:focus { border-color:rgba(151,188,255,.76); background:rgba(8,10,15,.58); box-shadow:0 0 0 3px rgba(124,164,255,.16),inset 0 1px 2px rgba(0,0,0,.2); }
  textarea::placeholder,input::placeholder { color:rgba(224,231,244,.32); }
  .context { min-height:70px; }.intent { min-height:52px; }.draft { min-height:90px; }
  .capture { border:0; border-radius:7px; padding:3px 6px; color:#aac9ff; background:transparent; cursor:pointer; font-size:10px; font-weight:650; transition:background 150ms ease,color 150ms ease; }.capture:hover:not(:disabled){background:rgba(142,181,255,.13);color:#d7e5ff}.capture:disabled{opacity:.55;cursor:wait}
  .shortcut { padding:2px 5px; border:1px solid rgba(255,255,255,.11); border-radius:5px; color:rgba(235,240,249,.47); background:rgba(255,255,255,.05); font-size:9px; }
  .tones { display:flex; flex-wrap:wrap; gap:5px; }.tones button { border:1px solid rgba(255,255,255,.1); border-radius:999px; padding:5px 8px; color:rgba(240,244,252,.66); background:rgba(255,255,255,.045); cursor:pointer; font-size:10px; font-weight:600; transition:all 160ms ease; }.tones button:hover{border-color:rgba(175,202,255,.35);color:#f7f9ff}.tones button.active{border-color:rgba(153,190,255,.55);color:#dbe8ff;background:linear-gradient(145deg,rgba(107,151,239,.31),rgba(93,127,195,.2));box-shadow:inset 0 1px rgba(255,255,255,.13)}
  .tones + input { margin-top:7px; }
  .generate,.insert,.secondary { display:inline-flex; align-items:center; justify-content:center; min-height:36px; border-radius:10px; cursor:pointer; font-size:11px; font-weight:700; transition:transform 150ms ease,filter 150ms ease,border-color 150ms ease,background 150ms ease; }
  .generate { width:100%; gap:7px; margin-top:17px; border:1px solid rgba(204,223,255,.5); color:#10213d; background:linear-gradient(180deg,#d7e6ff,#9fc0f8); box-shadow:inset 0 1px rgba(255,255,255,.72),0 6px 16px rgba(86,139,232,.2); }.generate:hover:not(:disabled),.insert:hover:not(:disabled){filter:brightness(1.06);transform:translateY(-1px)}.generate:active:not(:disabled),.insert:active:not(:disabled),.secondary:active{transform:translateY(1px) scale(.985)}.generate:disabled,.insert:disabled{opacity:.48;cursor:not-allowed}
  .mini-spinner { width:12px; height:12px; border:1.5px solid rgba(16,33,61,.26); border-top-color:#10213d; border-radius:50%; animation:spin .8s linear infinite; }
  .error { display:flex; gap:6px; margin:10px 0 0; padding:8px 9px; border:1px solid rgba(255,130,130,.2); border-radius:9px; color:#ffd1d1; background:rgba(210,63,63,.12); font-size:10px; line-height:1.35; }.error > :first-child{display:grid;place-items:center;flex:0 0 14px;width:14px;height:14px;border-radius:50%;color:#ffbcbc;background:rgba(255,132,132,.2);font-weight:800}
  .draft-card { margin-top:14px; padding:11px; border:1px solid rgba(153,193,255,.16); border-radius:14px; background:rgba(122,159,236,.075); animation:reveal 220ms cubic-bezier(.2,.8,.2,1) both; }
  .ready { display:flex; align-items:center; gap:4px; color:rgba(190,240,207,.84); font-size:9px; }.ready i{width:5px;height:5px;border-radius:50%;background:#88e5a7;box-shadow:0 0 7px rgba(136,229,167,.7)}
  .stale { color:#ffd69a; font-size:9px; }
  .actions { margin-top:8px; }.secondary,.insert { flex:1; border:1px solid rgba(255,255,255,.13); }.secondary { color:rgba(247,249,255,.85); background:rgba(255,255,255,.07); }.secondary:hover{border-color:rgba(193,213,255,.35);background:rgba(255,255,255,.11)}.insert { color:#10213d; background:linear-gradient(180deg,#d7e6ff,#9fc0f8); border-color:rgba(204,223,255,.5); }
  @keyframes spin { to { transform:rotate(360deg); } }@keyframes reveal { from{opacity:0;transform:translateY(6px) scale(.985)}to{opacity:1;transform:none} }
  @media (prefers-reduced-motion: reduce) { *,*::before,*::after { animation-duration:.01ms!important; transition-duration:.01ms!important; } }
</style>
