<script>
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { translate } from "./i18n.js";

  let state = $state("idle"); // idle | transforming | done | error | notice
  let errorMessage = $state("");
  let noticeMessage = $state("");
  let language = $state("en");
  let fadeTimer = $state(null);
  const t = (key) => translate(language, key);

  onMount(() => {
    void invoke("get_config")
      .then((config) => language = config.app_language === "es" ? "es" : "en")
      .catch(() => {});
    const unlisten1 = listen("flick://transforming", () => {
      clearTimers();
      state = "transforming";
      showWindow();
    });

    const unlisten2 = listen("flick://done", () => {
      clearTimers();
      state = "done";
      showWindow();
      fadeTimer = setTimeout(() => {
        state = "idle";
        hideWindow();
      }, 1100);
    });

    const unlisten3 = listen("flick://error", (event) => {
      clearTimers();
      state = "error";
      errorMessage = event.payload?.message || "Something went wrong";
      showWindow();
      fadeTimer = setTimeout(() => {
        state = "idle";
        hideWindow();
      }, 2800);
    });

    const unlisten4 = listen("flick://transform-finished", () => {
      clearTimers();
      state = "idle";
      hideWindow();
    });

    const unlisten5 = listen("flick://toast", (event) => {
      clearTimers();
      noticeMessage = String(event.payload || t("transform.copied"));
      state = "notice";
      showWindow();
      fadeTimer = setTimeout(() => {
        state = "idle";
        hideWindow();
      }, 1800);
    });

    return () => {
      unlisten1.then((dispose) => dispose());
      unlisten2.then((dispose) => dispose());
      unlisten3.then((dispose) => dispose());
      unlisten4.then((dispose) => dispose());
      unlisten5.then((dispose) => dispose());
      clearTimers();
    };
  });

  function clearTimers() {
    if (fadeTimer) {
      clearTimeout(fadeTimer);
      fadeTimer = null;
    }
  }

  async function showWindow() {
    try { await getCurrentWindow().show(); } catch {}
  }

  async function hideWindow() {
    try { await getCurrentWindow().hide(); } catch {}
  }
</script>

<div class="toast-container" class:visible={state !== "idle"} aria-live="polite">
  {#if state === "transforming"}
    <div class="toast toast-transforming" role="status">
      <span class="status-icon spinner" aria-hidden="true"></span>
      <span class="toast-copy"><strong>{t("transform.transforming")}</strong><small>{t("transform.transformingDetail")}</small></span>
    </div>
  {:else if state === "done"}
    <div class="toast toast-done" role="status">
      <span class="status-icon success" aria-hidden="true">✓</span>
      <span class="toast-copy"><strong>{t("transform.ready")}</strong><small>{t("transform.replaced")}</small></span>
    </div>
  {:else if state === "error"}
    <div class="toast toast-error" role="alert">
      <span class="status-icon warning" aria-hidden="true">!</span>
      <span class="toast-copy"><strong>{t("transform.failed")}</strong><small>{errorMessage}</small></span>
    </div>
  {:else if state === "notice"}
    <div class="toast toast-done" role="status">
      <span class="status-icon success" aria-hidden="true">✓</span>
      <span class="toast-copy"><strong>{t("transform.copied")}</strong><small>{noticeMessage}</small></span>
    </div>
  {/if}
</div>

<style>
  :global(html), :global(body) { background: transparent; overflow: hidden; }
  .toast-container { display:grid; place-items:center; width:100%; height:100%; opacity:0; transform:translateY(7px) scale(.96); transition:opacity 180ms ease,transform 240ms cubic-bezier(.2,.8,.2,1); }
  .toast-container.visible { opacity:1; transform:none; }
  .toast { display:flex; align-items:center; gap:10px; width:calc(100% - 12px); min-height:58px; padding:9px 12px; color:rgba(255,255,255,.96); background:linear-gradient(135deg,rgba(43,48,59,.94),rgba(18,20,26,.96)); border:1px solid rgba(255,255,255,.18); border-radius:17px; box-shadow:0 16px 42px rgba(0,0,0,.36),inset 0 1px rgba(255,255,255,.12); backdrop-filter:blur(24px) saturate(1.35); -webkit-backdrop-filter:blur(24px) saturate(1.35); font-family:-apple-system,BlinkMacSystemFont,"SF Pro Display",system-ui,sans-serif; }
  .status-icon { display:grid; flex:0 0 28px; place-items:center; width:28px; height:28px; border-radius:10px; font-size:15px; font-weight:700; }
  .spinner { border:2px solid rgba(185,210,255,.2); border-top-color:#bcd5ff; animation:spin .8s linear infinite; }
  .success { color:#b8f5cd; background:rgba(76,196,120,.18); border:1px solid rgba(126,232,164,.22); }
  .warning { color:#ffd0d0; background:rgba(255,107,107,.16); border:1px solid rgba(255,147,147,.24); }
  .toast-copy { display:grid; min-width:0; gap:1px; }
  strong { font-size:12px; line-height:1.2; letter-spacing:-.01em; }
  small { overflow:hidden; color:rgba(245,247,251,.66); font-size:10px; line-height:1.25; text-overflow:ellipsis; white-space:nowrap; }
  .toast-error small { white-space:normal; display:-webkit-box; -webkit-box-orient:vertical; -webkit-line-clamp:2; }
  @keyframes spin { to { transform:rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .toast-container,.spinner { animation:none; transition:none; } }
</style>
