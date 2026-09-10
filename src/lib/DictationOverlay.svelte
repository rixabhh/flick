<script>
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { translate } from "./i18n.js";

  let state = $state("recording");
  let language = $state("en");
  let cloudTranscription = $state(false);
  const t = (key) => translate(language, key);

  async function refreshPresentationConfig() {
    try {
      const config = await invoke("get_config");
      language = config.app_language === "es" ? "es" : "en";
      cloudTranscription = config.dictation_provider === "cloud-openai-compatible";
    } catch {}
  }

  onMount(() => {
    let disposed = false;
    let unlisten = () => {};
    void (async () => {
      await refreshPresentationConfig();
      const dispose = await listen("flick://dictation-state", (event) => {
        state = String(event.payload || "recording");
        if (state === "transcribing") void refreshPresentationConfig();
      });
      if (disposed) dispose();
      else unlisten = dispose;
    })();
    return () => { disposed = true; unlisten(); };
  });

  const label = () => state === "transcribing"
    ? t(cloudTranscription ? "dictation.transcribingCloud" : "dictation.transcribing")
    : t("dictation.recording");
  const detail = () => state === "transcribing"
    ? t(cloudTranscription ? "dictation.cloud" : "dictation.private")
    : t("dictation.discard");
</script>

<main class:processing={state === "transcribing"} class="overlay" aria-live="polite" aria-label={`Dictation ${label()}`}>
  <div class="signal" aria-hidden="true">
    {#if state === "transcribing"}
      <span class="spinner"></span>
    {:else}
      <span class="wave"><i></i><i></i><i></i><i></i><i></i></span>
    {/if}
  </div>
  <div class="copy">
    <strong>{label()}</strong>
    <small><span class="privacy-dot"></span>{detail()}</small>
  </div>
</main>

<style>
  :global(html), :global(body) { background: transparent; overflow: hidden; }
  .overlay {
    height: 100vh;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    color: rgba(255,255,255,.96);
    background:
      radial-gradient(circle at 18% 0%, rgba(255,255,255,.15), transparent 32%),
      linear-gradient(135deg, rgba(41,46,57,.92), rgba(16,18,24,.94));
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 22px;
    box-sizing: border-box;
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Display", system-ui, sans-serif;
    box-shadow: 0 18px 44px rgba(0, 0, 0, .34), inset 0 1px rgba(255,255,255,.12);
    backdrop-filter: blur(24px) saturate(1.35);
    -webkit-backdrop-filter: blur(24px) saturate(1.35);
    animation: settle .3s cubic-bezier(.2,.8,.2,1) both;
  }
  .overlay.processing {
    background:
      radial-gradient(circle at 18% 0%, rgba(166,194,255,.2), transparent 35%),
      linear-gradient(135deg, rgba(35,43,64,.94), rgba(16,18,25,.95));
  }
  .signal { display:grid; place-items:center; width:32px; height:32px; border-radius:12px; background:rgba(255,91,109,.16); border:1px solid rgba(255,130,145,.2); box-shadow:inset 0 1px rgba(255,255,255,.12); }
  .processing .signal { background:rgba(137,177,255,.16); border-color:rgba(151,188,255,.25); }
  .wave { display:flex; align-items:center; gap:2.5px; height:18px; }
  .wave i { width:2.5px; height:7px; border-radius:99px; background:#ff8290; box-shadow:0 0 9px rgba(255,98,120,.75); animation:level 800ms ease-in-out infinite alternate; }
  .wave i:nth-child(2){height:14px;animation-delay:-320ms}.wave i:nth-child(3){height:18px;animation-delay:-120ms}.wave i:nth-child(4){height:12px;animation-delay:-470ms}.wave i:nth-child(5){height:8px;animation-delay:-240ms}
  .spinner { width:17px; height:17px; border:2px solid rgba(170,201,255,.22); border-top-color:#afd0ff; border-radius:50%; animation:spin .85s linear infinite; }
  .copy { display:grid; gap:3px; min-width:0; }
  strong { font-size:13px; line-height:1.1; letter-spacing:-.01em; }
  small { display:flex; align-items:center; gap:5px; color:rgba(247,248,251,.66); font-size:10px; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .privacy-dot { width:5px; height:5px; border-radius:50%; background:#85e5ac; box-shadow:0 0 7px rgba(133,229,172,.7); }
  @keyframes level { to { transform:scaleY(.38); opacity:.58; } }
  @keyframes spin { to { transform:rotate(360deg); } }
  @keyframes settle { from { opacity:0; transform:translateY(8px) scale(.94); } to { opacity:1; transform:none; } }
  @media (prefers-reduced-motion: reduce) { .overlay,.wave i,.spinner { animation:none; } .wave i{transform:scaleY(.7)} }
</style>
