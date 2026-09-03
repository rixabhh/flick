<script>
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { translate } from "./i18n.js";

  let state = $state("recording");
  let language = $state("en");
  const t = (key) => translate(language, key);

  onMount(() => {
    let disposed = false;
    let unlisten = () => {};
    void (async () => {
      try {
        const config = await invoke("get_config");
        language = config.app_language === "es" ? "es" : "en";
      } catch {}
      const dispose = await listen("flick://dictation-state", (event) => {
        state = String(event.payload || "recording");
      });
      if (disposed) dispose();
      else unlisten = dispose;
    })();
    return () => { disposed = true; unlisten(); };
  });

  const label = () => state === "transcribing" ? t("dictation.transcribing") : t("dictation.recording");
</script>

<main class="overlay" aria-live="polite" aria-label={`Dictation ${label()}`}>
  <span class:working={state === "transcribing"} class="indicator" aria-hidden="true"></span>
  <div>
    <strong>{label()}</strong>
    <small>{state === "transcribing" ? t("dictation.private") : t("dictation.discard")}</small>
  </div>
</main>

<style>
  :global(html), :global(body) { background: transparent; overflow: hidden; }
  .overlay {
    height: 100vh;
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 12px 16px;
    color: #f7f8fb;
    background: rgba(20, 24, 34, 0.96);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 14px;
    box-sizing: border-box;
    font-family: Inter, system-ui, sans-serif;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
  }
  .indicator {
    width: 10px;
    height: 10px;
    border-radius: 999px;
    background: #ff5d6c;
    box-shadow: 0 0 0 0 rgba(255, 93, 108, 0.55);
    animation: pulse 1.3s ease-out infinite;
  }
  .indicator.working { background: #7ca7ff; animation: none; box-shadow: 0 0 12px rgba(124, 167, 255, 0.75); }
  div { display: grid; gap: 3px; }
  strong { font-size: 13px; line-height: 1.1; }
  small { color: rgba(247, 248, 251, 0.68); font-size: 11px; }
  @keyframes pulse { 70% { box-shadow: 0 0 0 8px rgba(255, 93, 108, 0); } 100% { box-shadow: 0 0 0 0 rgba(255, 93, 108, 0); } }
</style>
