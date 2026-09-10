<script>
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { translate } from "./i18n.js";

  let state = $state("idle");
  let errorMessage = $state("");
  let noticeMessage = $state("");
  let language = $state("en");
  let fadeTimer = $state(null);
  const t = (key) => translate(language, key);

  onMount(() => {
    void invoke("get_config").then((config) => language = config.app_language === "es" ? "es" : "en").catch(() => {});
    const subscriptions = [
      listen("flick://transforming", () => present("transforming")),
      listen("flick://done", () => present("done", 950)),
      listen("flick://error", (event) => { errorMessage = String(event.payload?.message || "Something went wrong"); present("error", 3200); }),
      listen("flick://transform-finished", () => dismiss()),
      listen("flick://toast", (event) => { noticeMessage = String(event.payload || t("transform.copied")); present("notice", 1400); }),
    ];
    return () => { subscriptions.forEach((subscription) => subscription.then((dispose) => dispose())); clearTimer(); };
  });

  function clearTimer() { if (fadeTimer) { clearTimeout(fadeTimer); fadeTimer = null; } }
  function present(next, duration) { clearTimer(); state = next; void getCurrentWindow().show().catch(() => {}); if (duration) fadeTimer = setTimeout(dismiss, duration); }
  function dismiss() { clearTimer(); state = "idle"; void getCurrentWindow().hide().catch(() => {}); }
</script>

<div class="stage" class:visible={state !== "idle"} aria-live="polite">
  {#if state !== "idle"}
    <section class="pill" class:error={state === "error"} role={state === "error" ? "alert" : "status"}>
      {#if state === "transforming"}
        <span class="spinner" aria-hidden="true"></span><strong>{t("transform.transforming")}</strong>
      {:else if state === "error"}
        <span class="icon warning" aria-hidden="true">!</span><span class="copy"><strong>{t("transform.failed")}</strong><small>{errorMessage}</small></span>
      {:else}
        <span class="icon success" aria-hidden="true">✓</span><strong>{state === "notice" ? noticeMessage : t("transform.replaced")}</strong>
      {/if}
    </section>
  {/if}
</div>

<style>
  :global(html),:global(body){margin:0;background:transparent;overflow:hidden}
  .stage{width:100vw;height:100vh;display:grid;place-items:center;opacity:0;transform:translateY(5px) scale(.92);transition:opacity 160ms ease,transform 360ms cubic-bezier(.22,1,.36,1);font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text","Helvetica Neue",system-ui,sans-serif}
  .stage.visible{opacity:1;transform:none}
  .pill{position:relative;isolation:isolate;display:flex;align-items:center;justify-content:center;gap:8px;max-width:274px;min-width:132px;height:40px;padding:0 13px;box-sizing:border-box;overflow:hidden;color:rgba(255,255,255,.9);background:linear-gradient(145deg,rgba(49,50,56,.8),rgba(18,19,23,.86));border:1px solid rgba(255,255,255,.2);border-radius:999px;box-shadow:0 10px 30px rgba(0,0,0,.3),inset 0 1px rgba(255,255,255,.14);backdrop-filter:blur(26px) saturate(1.55);-webkit-backdrop-filter:blur(26px) saturate(1.55);animation:material-in 380ms cubic-bezier(.22,1,.36,1) both}
  .pill::before{content:"";position:absolute;z-index:-1;inset:0;border-radius:inherit;background:radial-gradient(100px 30px at 28% -6%,rgba(255,255,255,.17),transparent 72%)}
  .pill.error{width:274px;height:52px;justify-content:flex-start;border-radius:18px;padding:0 11px}
  .icon{display:grid;flex:0 0 20px;place-items:center;width:20px;height:20px;border-radius:50%;font-size:11px;font-weight:760}
  .success{color:#bdf3d0;background:rgba(73,190,116,.18);border:1px solid rgba(133,229,167,.2)}
  .warning{color:#ffd0d2;background:rgba(255,91,103,.14);border:1px solid rgba(255,139,148,.2)}
  .spinner{width:13px;height:13px;box-sizing:border-box;border:1.5px solid rgba(203,216,245,.18);border-top-color:rgba(220,231,255,.92);border-radius:50%;animation:spin .72s linear infinite}
  .copy{display:grid;min-width:0;gap:2px}
  strong{min-width:0;overflow:hidden;font-size:11.5px;font-weight:560;line-height:1.15;letter-spacing:-.01em;text-overflow:ellipsis;white-space:nowrap}
  small{display:-webkit-box;overflow:hidden;color:rgba(245,247,251,.58);font-size:9.5px;line-height:1.25;-webkit-box-orient:vertical;-webkit-line-clamp:2}
  @keyframes spin{to{transform:rotate(1turn)}}
  @keyframes material-in{from{opacity:0;transform:scale(.9);filter:blur(3px)}to{opacity:1;transform:none;filter:none}}
  @media (prefers-color-scheme:light){.pill{color:rgba(22,24,30,.8);background:linear-gradient(145deg,rgba(255,255,255,.8),rgba(235,237,243,.75));border-color:rgba(34,39,52,.13);box-shadow:0 10px 28px rgba(30,37,51,.17),inset 0 1px rgba(255,255,255,.88)}small{color:rgba(28,31,39,.54)}.spinner{border-color:rgba(38,49,75,.15);border-top-color:rgba(43,56,87,.78)}}
  :global(:root[data-theme="light"]) .pill{color:rgba(22,24,30,.8);background:linear-gradient(145deg,rgba(255,255,255,.8),rgba(235,237,243,.75));border-color:rgba(34,39,52,.13);box-shadow:0 10px 28px rgba(30,37,51,.17),inset 0 1px rgba(255,255,255,.88)}
  :global(:root[data-theme="light"]) small{color:rgba(28,31,39,.54)}
  :global(:root[data-theme="light"]) .spinner{border-color:rgba(38,49,75,.15);border-top-color:rgba(43,56,87,.78)}
  @media (prefers-reduced-motion:reduce){.stage,.pill,.spinner{animation:none;transition:none}}
</style>
