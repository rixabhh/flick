<script>
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, tick } from "svelte";
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
  let copying = $state(false);
  let copied = $state(false);
  let error = $state("");
  let draftSignature = $state("");
  let providerNotice = $state("");
  let language = $state("en");
  let contextInput = $state();
  let intentInput = $state();
  let contextExpanded = $state(true);
  let generateShortcut = $state("⌘ ↵");
  let session = 0;
  let copiedTimer;

  const t = (key) => translate(language, key);
  const toneValue = () => tone === "Custom" ? (customTone.trim() || "friendly") : tone.toLowerCase();
  const requestSignature = () => `${context}\u0000${toneValue()}\u0000${instruction}`;
  const draftMatchesRequest = () => Boolean(draft) && draftSignature === requestSignature();
  const contextPreview = () => context.replace(/\s+/g, " ").trim().slice(0, 88);

  function resetSession() {
    session += 1;
    loading = capturing = inserting = copying = copied = false;
    clearTimeout(copiedTimer);
  }

  function beginSession(payload) {
    resetSession();
    context = typeof payload === "string" ? payload : String(payload?.context || "");
    instruction = "";
    draft = "";
    draftSignature = "";
    contextExpanded = !context;
    error = typeof payload === "object" && payload?.error
      ? String(payload.error)
      : context ? "" : t("composer.noSelection");
    void tick().then(() => context ? intentInput?.focus() : contextInput?.focus());
  }

  async function captureSelection() {
    if (capturing || loading || inserting) return;
    const currentSession = session;
    error = "";
    capturing = true;
    try {
      const selection = await invoke("capture_reply_context");
      if (currentSession !== session) return;
      context = selection;
      contextExpanded = !selection;
      draft = "";
      draftSignature = "";
      void tick().then(() => selection ? intentInput?.focus() : contextInput?.focus());
    } catch (message) {
      if (currentSession === session) error = String(message);
    } finally {
      if (currentSession === session) capturing = false;
    }
  }

  async function generate() {
    if (loading || capturing || inserting || !context.trim() || !instruction.trim()) return;
    const currentSession = session;
    const signature = requestSignature();
    error = "";
    copied = false;
    loading = true;
    try {
      const result = await invoke("generate_reply", { context, tone: toneValue(), instruction });
      if (currentSession !== session) return;
      draft = result;
      draftSignature = signature;
    } catch (message) {
      if (currentSession === session) error = String(message);
    } finally {
      if (currentSession === session) loading = false;
    }
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
    } catch (message) {
      if (currentSession === session) error = String(message);
    } finally {
      if (currentSession === session) copying = false;
    }
  }

  async function insert() {
    if (inserting || capturing || loading || copying) return;
    if (!draftMatchesRequest()) {
      error = t("composer.staleError");
      return;
    }
    const currentSession = session;
    error = "";
    inserting = true;
    try {
      await invoke("insert_reply", { draft });
    } catch (message) {
      if (currentSession === session) error = `${message} ${t("composer.insertRecovery")}`;
    } finally {
      if (currentSession === session) inserting = false;
    }
  }

  async function close() {
    try {
      await getCurrentWindow().hide();
      resetSession();
    } catch (message) {
      error = String(message);
    }
  }

  function handleKeydown(event) {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault();
      void generate();
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
    if (!/mac/i.test(navigator.platform || navigator.userAgent)) generateShortcut = "Ctrl ↵";
    void listen("flick://composer-context", (event) => beginSession(event.payload))
      .then((dispose) => { if (disposed) dispose(); else unlisten = dispose; })
      .catch((message) => { if (!disposed) error = String(message); });
    void invoke("get_config")
      .then((config) => {
        language = config.app_language === "es" ? "es" : "en";
        providerNotice = describeProvider(config);
      })
      .catch(() => providerNotice = t("composer.providerFallback"));
    return () => { disposed = true; resetSession(); unlisten(); clearTimeout(focusTimer); };
  });
</script>

<div class="composer" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="composer-title" aria-busy={loading || capturing || inserting} onkeydown={handleKeydown}>
  <section class="quick-reply">
    <header data-tauri-drag-region>
      <div class="title" data-tauri-drag-region>
        <span class="mark" aria-hidden="true">✦</span>
        <span id="composer-title">Reply</span>
      </div>
      <button class="close" aria-label={t("composer.close")} onclick={close} disabled={inserting}>×</button>
    </header>

    {#if context && !contextExpanded}
      <button class="selection-chip" onclick={() => contextExpanded = true} title="Edit selected context">
        <span class="selection-dot" aria-hidden="true"></span>
        <span>{contextPreview()}</span>
        <span class="edit-context" aria-hidden="true">Edit</span>
      </button>
    {:else}
      <section class="context-panel">
        <div class="context-label"><span>{t("composer.context")}</span><button class="text-button" onclick={captureSelection} disabled={capturing || loading || inserting}>{capturing ? t("composer.capturing") : t("composer.capture")}</button></div>
        <textarea id="context" bind:this={contextInput} bind:value={context} placeholder={t("composer.contextPlaceholder")}></textarea>
        {#if context}<button class="collapse-context" onclick={() => contextExpanded = false}>Use selected context</button>{/if}
      </section>
    {/if}

    <div class="prompt-shell" class:has-draft={Boolean(draft)}>
      <textarea id="intent" bind:this={intentInput} bind:value={instruction} placeholder={t("composer.intentPlaceholder")} aria-label={t("composer.intent")}></textarea>
      <div class="prompt-toolbar">
        <label class="tone-picker"><span class="sr-only">{t("composer.tone")}</span><select bind:value={tone} aria-label={t("composer.tone")}>{#each tones as item}<option value={item}>{t(`tone.${item}`)}</option>{/each}</select></label>
        {#if tone === "Custom"}<input class="custom-tone" aria-label={t("composer.customTone")} bind:value={customTone} placeholder={t("composer.customTone")} />{/if}
        <span class="shortcut">{generateShortcut}</span>
        <button class="generate" onclick={generate} disabled={loading || capturing || inserting || !context.trim() || !instruction.trim()}>
          {#if loading}<span class="spinner" aria-hidden="true"></span>{/if}<span>{loading ? t("composer.drafting") : draft ? t("composer.regenerate") : t("composer.generate")}</span>
        </button>
      </div>
    </div>

    {#if error}<p class="error" role="alert"><span aria-hidden="true">!</span>{error}</p>{/if}

    {#if draft}
      <section class="draft-card">
        <div class="draft-label"><span>{t("composer.draft")}</span>{#if draftMatchesRequest()}<span class="ready"><i></i>{t("composer.ready")}</span>{:else}<span class="stale">{t("composer.stale")}</span>{/if}</div>
        <textarea id="draft" bind:value={draft} readonly={loading || inserting} oninput={() => copied = false}></textarea>
        <div class="draft-actions"><button class="secondary" onclick={copy} disabled={copying || loading || inserting}>{copied ? t("composer.copied") : t("composer.copy")}</button><button class="insert" onclick={insert} disabled={inserting || copying || capturing || loading || !draftMatchesRequest()}>{inserting ? t("composer.inserting") : t("composer.insert")}</button></div>
      </section>
    {/if}

    <p class="provider-note">{providerNotice}</p>
  </section>
</div>

<style>
  :global(html), :global(body) { margin:0; background:transparent; overflow:hidden; }
  .composer { height:100vh; overflow:auto; padding:6px; box-sizing:border-box; color:rgba(250,251,255,.96); background:transparent; font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text","Helvetica Neue",system-ui,sans-serif; scrollbar-width:none; }
  .composer::-webkit-scrollbar { display:none; }
  .quick-reply { min-height:calc(100% - 12px); padding:12px; box-sizing:border-box; border:1px solid rgba(255,255,255,.18); border-radius:18px; background:linear-gradient(145deg,rgba(45,47,54,.88),rgba(21,22,27,.92)); box-shadow:0 16px 38px rgba(0,0,0,.32),inset 0 1px rgba(255,255,255,.16); backdrop-filter:blur(32px) saturate(1.5); -webkit-backdrop-filter:blur(32px) saturate(1.5); }
  header,.title,.prompt-toolbar,.draft-label,.draft-actions,.context-label { display:flex; align-items:center; }
  header { justify-content:space-between; margin-bottom:10px; }.title { gap:7px; color:rgba(255,255,255,.94); font-size:12px; font-weight:720; letter-spacing:-.015em; }.mark { display:grid; place-items:center; width:21px; height:21px; border:1px solid rgba(192,215,255,.28); border-radius:7px; color:#dbe7ff; background:linear-gradient(145deg,rgba(149,183,255,.34),rgba(99,119,181,.18)); font-size:10px; box-shadow:inset 0 1px rgba(255,255,255,.2); }.close { display:grid; place-items:center; width:22px; height:22px; padding:0; border:0; border-radius:7px; color:rgba(244,247,255,.55); background:transparent; cursor:pointer; font-size:18px; line-height:1; transition:background 140ms ease,color 140ms ease; }.close:hover { color:#fff; background:rgba(255,255,255,.1); }
  .selection-chip { display:flex; align-items:center; width:100%; gap:7px; padding:8px 9px; overflow:hidden; border:1px solid rgba(163,195,255,.18); border-radius:10px; color:rgba(235,242,255,.77); background:rgba(115,148,215,.09); cursor:pointer; font:inherit; font-size:10.5px; text-align:left; transition:border-color 140ms ease,background 140ms ease; }.selection-chip:hover { border-color:rgba(177,206,255,.36); background:rgba(115,148,215,.14); }.selection-chip > :nth-child(2) { min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }.selection-dot { width:6px; height:6px; flex:0 0 auto; border-radius:50%; background:#9bc3ff; box-shadow:0 0 0 3px rgba(155,195,255,.13); }.edit-context { margin-left:auto; color:rgba(194,216,255,.68); font-size:9px; font-weight:700; }
  .context-panel { padding:9px; border:1px solid rgba(255,255,255,.1); border-radius:11px; background:rgba(0,0,0,.13); }.context-label,.draft-label { justify-content:space-between; color:rgba(232,238,251,.7); font-size:9.5px; font-weight:700; letter-spacing:.04em; text-transform:uppercase; }.text-button,.collapse-context { padding:0; border:0; color:#b7d2ff; background:transparent; cursor:pointer; font:inherit; font-size:9.5px; font-weight:700; }.collapse-context { display:block; margin:7px 0 0 auto; }
  textarea,input,select,button { font:inherit; }.context-panel textarea { width:100%; min-height:56px; margin-top:7px; resize:vertical; } textarea,.custom-tone { box-sizing:border-box; border:1px solid rgba(255,255,255,.11); border-radius:9px; color:rgba(250,252,255,.94); background:rgba(5,7,11,.29); outline:none; transition:border-color 140ms ease,box-shadow 140ms ease,background 140ms ease; } textarea:focus,.custom-tone:focus { border-color:rgba(163,199,255,.76); background:rgba(5,7,11,.47); box-shadow:0 0 0 3px rgba(126,168,255,.15); } textarea::placeholder,.custom-tone::placeholder { color:rgba(227,233,246,.35); }
  .prompt-shell { margin-top:8px; overflow:hidden; border:1px solid rgba(255,255,255,.14); border-radius:12px; background:rgba(5,7,11,.27); box-shadow:inset 0 1px rgba(255,255,255,.035); }.prompt-shell > textarea { display:block; width:100%; min-height:51px; max-height:92px; padding:10px 11px 4px; border:0; border-radius:0; background:transparent; resize:vertical; font-size:12px; line-height:1.4; box-shadow:none; }.prompt-shell.has-draft > textarea { min-height:38px; max-height:42px; padding-top:7px; resize:none; }.prompt-toolbar { min-height:35px; gap:7px; padding:5px 6px 6px 8px; border-top:1px solid rgba(255,255,255,.07); }.tone-picker select { max-width:116px; padding:3px 22px 3px 6px; border:0; border-radius:6px; color:rgba(231,238,250,.68); background:transparent; cursor:pointer; font-size:10px; outline:none; }.tone-picker select:hover { color:#fff; background:rgba(255,255,255,.06); }.tone-picker option { color:#1d2028; }.custom-tone { width:88px; padding:4px 6px; font-size:9px; }.shortcut { margin-left:auto; color:rgba(226,232,246,.35); font-size:9px; white-space:nowrap; }
  .generate,.insert,.secondary { display:inline-flex; align-items:center; justify-content:center; min-height:26px; padding:0 9px; border-radius:7px; cursor:pointer; font-size:10px; font-weight:720; transition:filter 140ms ease,transform 140ms ease,background 140ms ease; }.generate,.insert { border:1px solid rgba(211,227,255,.54); color:#10203a; background:linear-gradient(180deg,#e0ecff,#abc9fb); box-shadow:inset 0 1px rgba(255,255,255,.76); }.generate:hover:not(:disabled),.insert:hover:not(:disabled) { filter:brightness(1.06); transform:translateY(-1px); }.generate:disabled,.insert:disabled,.secondary:disabled,.close:disabled { opacity:.45; cursor:not-allowed; }.spinner { width:10px; height:10px; margin-right:5px; border:1.5px solid rgba(16,32,58,.25); border-top-color:#10203a; border-radius:50%; animation:spin .72s linear infinite; }
  .error { display:flex; align-items:flex-start; gap:6px; margin:8px 0 0; padding:7px 8px; border:1px solid rgba(255,139,145,.22); border-radius:9px; color:#ffd9dc; background:rgba(194,61,72,.13); font-size:9.5px; line-height:1.35; }.error span { display:grid; place-items:center; width:13px; height:13px; flex:0 0 auto; border-radius:50%; color:#ffdde0; background:rgba(255,139,145,.2); font-weight:800; }.draft-card { margin-top:8px; padding:9px; border:1px solid rgba(158,194,255,.17); border-radius:11px; background:rgba(105,144,223,.08); }.ready,.stale { display:flex; align-items:center; gap:4px; font-size:8.5px; letter-spacing:0; text-transform:none; }.ready { color:#aeecc3; }.ready i { width:5px; height:5px; border-radius:50%; background:#83dca0; box-shadow:0 0 6px rgba(131,220,160,.62); }.stale { color:#ffd493; }.draft-card textarea { display:block; width:100%; min-height:65px; max-height:125px; margin-top:7px; padding:8px; resize:vertical; font-size:11px; line-height:1.43; }.draft-actions { gap:6px; margin-top:7px; }.draft-actions button { flex:1; }.secondary { border:1px solid rgba(255,255,255,.13); color:rgba(247,249,255,.86); background:rgba(255,255,255,.075); }.secondary:hover:not(:disabled) { background:rgba(255,255,255,.12); }.provider-note { margin:8px 1px 0; color:rgba(227,233,247,.4); font-size:9px; line-height:1.25; }.sr-only { position:absolute; width:1px; height:1px; padding:0; margin:-1px; overflow:hidden; clip:rect(0,0,0,0); white-space:nowrap; border:0; }
  @keyframes spin { to { transform:rotate(1turn); } }
  @media (prefers-color-scheme:light) { .quick-reply { color:#1b202b; background:linear-gradient(145deg,rgba(255,255,255,.94),rgba(239,243,251,.91)); border-color:rgba(39,50,73,.14); box-shadow:0 16px 36px rgba(28,40,64,.18),inset 0 1px rgba(255,255,255,.9); }.title { color:#202633; }.close { color:rgba(27,34,46,.48); }.selection-chip { color:rgba(30,46,75,.72); background:rgba(111,151,222,.1); border-color:rgba(76,111,169,.16); }.context-panel,.prompt-shell { background:rgba(246,249,255,.56); border-color:rgba(35,48,74,.13); }.context-label,.draft-label,.provider-note,.shortcut { color:rgba(36,48,69,.52); } textarea,.custom-tone { color:#222936; background:rgba(255,255,255,.64); border-color:rgba(35,48,74,.16); }.tone-picker select { color:rgba(35,47,67,.72); }.secondary { color:#293244; background:rgba(45,61,91,.07); border-color:rgba(35,48,74,.14); }.draft-card { background:rgba(103,143,218,.08); border-color:rgba(76,111,169,.16); } }
  @media (prefers-reduced-motion:reduce) { *,*::before,*::after { animation-duration:.01ms!important; transition-duration:.01ms!important; } }
</style>
