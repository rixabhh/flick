<script>
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { translate } from "./i18n.js";

  const BAR_SHAPE = [0.58, 0.82, 1, 0.74, 0.92, 0.66, 0.48];
  let state = $state("idle");
  let language = $state("en");
  let cloudTranscription = $state(false);
  let level = $state(0);
  let elapsed = $state(0);
  let hasSession = false;
  const t = (key) => translate(language, key);

  async function refreshPresentationConfig() {
    try {
      const config = await invoke("get_config");
      if (hasSession) return;
      language = config.app_language === "es" ? "es" : "en";
      cloudTranscription = config.dictation_provider === "cloud-openai-compatible";
    } catch {}
  }

  function barHeight(index) {
    const energy = Math.min(1, Math.max(0, level * 2.8));
    return `${Math.round(3 + energy * 15 * BAR_SHAPE[index])}px`;
  }

  function clock() {
    const minutes = Math.floor(elapsed / 60);
    return `${minutes}:${String(elapsed % 60).padStart(2, "0")}`;
  }

  onMount(() => {
    let disposed = false;
    const disposers = [];
    const levelTimer = setInterval(async () => {
      if (state !== "recording") return;
      try {
        const next = Number(await invoke("dictation_input_level")) || 0;
        level = level * 0.62 + next * 0.38;
      } catch {}
    }, 70);
    const clockTimer = setInterval(() => { if (state === "recording") elapsed += 1; }, 1000);

    void (async () => {
      const disposeSession = await listen("flick://dictation-session", ({ payload }) => {
        hasSession = true;
        const nextState = payload.state;
        if (nextState === "starting" || (nextState === "recording" && state !== "recording")) {
          elapsed = 0;
          level = 0;
        }
        state = nextState;
        language = payload.app_language === "es" ? "es" : "en";
        cloudTranscription = payload.provider_id === "cloud-openai-compatible";
      });
      if (disposed) { disposeSession(); return; }
      disposers.push(disposeSession);
      const disposeState = await listen("flick://dictation-state", (event) => {
        state = String(event.payload || "recording");
      });
      if (disposed) disposeState();
      else disposers.push(disposeState);
    })().catch(() => {});
    void refreshPresentationConfig();
    return () => {
      disposed = true;
      clearInterval(levelTimer);
      clearInterval(clockTimer);
      disposers.forEach((dispose) => dispose());
    };
  });

  const label = () => state === "starting" || state === "idle"
    ? t("dictation.starting")
    : state === "transcribing"
      ? t(cloudTranscription ? "dictation.transcribingCloud" : "dictation.transcribing")
      : t("dictation.recording");
</script>

<main class="stage overlay" aria-live="polite" aria-label={`Dictation ${label()}`}>
  <section class="pill" class:recording={state === "recording"}>
    <span class="status-dot" class:live={state === "recording"} aria-hidden="true"></span>
    {#if state === "recording"}
      <span class="wave" aria-hidden="true">
        {#each BAR_SHAPE as _, index}<i style:height={barHeight(index)}></i>{/each}
      </span>
      <span class="timer">{clock()}</span>
      <strong class="sr-only">{label()}</strong>
    {:else}
      <span class="spinner" aria-hidden="true"></span>
      <strong>{label()}</strong>
    {/if}
  </section>
</main>

<style>
  :global(html), :global(body) { margin:0; background:transparent; overflow:hidden; }
  .stage { width:100vw; height:100vh; display:flex; align-items:center; justify-content:center; color:rgba(255,255,255,.94); font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text","Helvetica Neue",system-ui,sans-serif; }
  .pill { position:relative; isolation:isolate; width:204px; height:40px; display:grid; grid-template-columns:20px 16px minmax(0,1fr); align-items:center; gap:7px; padding:0 11px; box-sizing:border-box; overflow:hidden; border:1px solid rgba(255,255,255,.2); border-radius:999px; background:linear-gradient(145deg,rgba(48,49,55,.8),rgba(19,20,24,.84)); box-shadow:0 10px 30px rgba(0,0,0,.3),inset 0 1px rgba(255,255,255,.14),inset 0 -1px rgba(0,0,0,.18); backdrop-filter:blur(26px) saturate(1.55); -webkit-backdrop-filter:blur(26px) saturate(1.55); animation:material-in 420ms cubic-bezier(.22,1,.36,1) both; transition:width 380ms cubic-bezier(.22,1,.36,1),background 260ms ease,border-color 260ms ease; }
  .pill::before { content:""; position:absolute; z-index:-1; inset:0; border-radius:inherit; background:radial-gradient(110px 34px at 28% -8%,rgba(255,255,255,.17),transparent 72%); pointer-events:none; }
  .pill.recording { width:172px; grid-template-columns:18px 1fr auto; background:linear-gradient(145deg,rgba(48,48,53,.78),rgba(18,19,22,.86)); }
  .status-dot { width:7px; height:7px; justify-self:center; border-radius:50%; background:rgba(185,190,202,.48); box-shadow:0 0 0 4px rgba(190,198,214,.06); }
  .status-dot.live { background:#ff6b78; box-shadow:0 0 0 4px rgba(255,93,111,.09),0 0 10px rgba(255,89,108,.46); animation:pulse 1.9s ease-out infinite; }
  .wave { height:19px; display:flex; align-items:center; justify-content:center; gap:3px; }
  .wave i { width:3px; min-height:3px; max-height:18px; border-radius:99px; background:linear-gradient(180deg,#ff9ba4,#ff6372); box-shadow:0 0 6px rgba(255,94,112,.18); transition:height 70ms linear,opacity 120ms ease; }
  .timer { color:rgba(244,245,249,.58); font-size:11px; font-variant-numeric:tabular-nums; letter-spacing:.01em; }
  .spinner { width:12px; height:12px; box-sizing:border-box; border:1.5px solid rgba(205,218,246,.2); border-top-color:rgba(219,230,255,.9); border-radius:50%; animation:spin .72s linear infinite; }
  strong { min-width:0; overflow:hidden; color:rgba(250,250,252,.82); font-size:11.5px; font-weight:560; letter-spacing:-.01em; text-overflow:ellipsis; white-space:nowrap; }
  .sr-only { position:absolute; width:1px; height:1px; padding:0; margin:-1px; overflow:hidden; clip:rect(0,0,0,0); white-space:nowrap; border:0; }
  @keyframes material-in { from { opacity:0; transform:translateY(5px) scale(.9); filter:blur(3px); } to { opacity:1; transform:none; filter:none; } }
  @keyframes spin { to { transform:rotate(1turn); } }
  @keyframes pulse { 0% { box-shadow:0 0 0 0 rgba(255,93,111,.28),0 0 10px rgba(255,89,108,.4); } 70%,100% { box-shadow:0 0 0 7px transparent,0 0 10px rgba(255,89,108,.32); } }
  @media (prefers-color-scheme:light) { .stage { color:#202126; } .pill,.pill.recording { background:linear-gradient(145deg,rgba(255,255,255,.78),rgba(235,237,243,.74)); border-color:rgba(34,39,52,.13); box-shadow:0 10px 28px rgba(30,37,51,.18),inset 0 1px rgba(255,255,255,.86); } strong { color:rgba(22,24,30,.78); } .timer { color:rgba(28,30,38,.52); } .spinner { border-color:rgba(38,49,75,.15); border-top-color:rgba(43,56,87,.78); } }
  :global(:root[data-theme="light"]) .pill { background:linear-gradient(145deg,rgba(255,255,255,.78),rgba(235,237,243,.74)); border-color:rgba(34,39,52,.13); box-shadow:0 10px 28px rgba(30,37,51,.18),inset 0 1px rgba(255,255,255,.86); }
  :global(:root[data-theme="light"]) strong { color:rgba(22,24,30,.78); }
  :global(:root[data-theme="light"]) .timer { color:rgba(28,30,38,.52); }
  :global(:root[data-theme="light"]) .spinner { border-color:rgba(38,49,75,.15); border-top-color:rgba(43,56,87,.78); }
  @media (prefers-reduced-motion:reduce) { .pill,.status-dot,.spinner { animation:none; } .wave i { transition:none; } }
</style>
