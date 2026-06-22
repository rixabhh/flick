<script>
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  let state = $state("idle"); // idle | transforming | done | error
  let errorMessage = $state("");
  let fadeTimer = $state(null);

  onMount(() => {
    const unlisten1 = listen("flick://transforming", () => {
      clearTimers();
      state = "transforming";
      showWindow();
    });

    const unlisten2 = listen("flick://done", () => {
      clearTimers();
      state = "done";
      fadeTimer = setTimeout(() => {
        state = "idle";
        hideWindow();
      }, 800);
    });

    const unlisten3 = listen("flick://error", (event) => {
      clearTimers();
      state = "error";
      errorMessage = event.payload?.message || "Something went wrong";
      fadeTimer = setTimeout(() => {
        state = "idle";
        hideWindow();
      }, 2000);
    });

    return () => {
      unlisten1.then(f => f());
      unlisten2.then(f => f());
      unlisten3.then(f => f());
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
    try {
      const win = getCurrentWindow();
      await win.show();
    } catch {}
  }

  async function hideWindow() {
    try {
      const win = getCurrentWindow();
      await win.hide();
    } catch {}
  }
</script>

<div class="toast-container" class:visible={state !== "idle"} class:fade-out={state === "idle"}>
  {#if state === "transforming"}
    <div class="toast toast-transforming animate-fade-in">
      <div class="pulse-dot"></div>
      <span class="toast-text">Flick is thinking…</span>
    </div>
  {:else if state === "done"}
    <div class="toast toast-done animate-fade-in">
      <svg class="toast-icon" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
      </svg>
      <span class="toast-text">Done</span>
    </div>
  {:else if state === "error"}
    <div class="toast toast-error animate-fade-in">
      <svg class="toast-icon" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clip-rule="evenodd" />
      </svg>
      <span class="toast-text">{errorMessage}</span>
    </div>
  {/if}
</div>

<style>
  .toast-container {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    opacity: 0;
    transition: opacity 300ms ease;
  }

  .toast-container.visible {
    opacity: 1;
  }

  .toast {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    border-radius: 24px;
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    white-space: nowrap;
    font-size: 13px;
    font-weight: 500;
    font-family: var(--font-body);
  }

  .toast-transforming {
    background: rgba(0, 229, 255, 0.12);
    border: 1px solid rgba(0, 229, 255, 0.25);
    color: #00e5ff;
  }

  .toast-done {
    background: rgba(0, 229, 160, 0.12);
    border: 1px solid rgba(0, 229, 160, 0.25);
    color: #00e5a0;
  }

  .toast-error {
    background: rgba(255, 68, 68, 0.12);
    border: 1px solid rgba(255, 68, 68, 0.25);
    color: #ff4444;
  }

  .toast-text {
    letter-spacing: 0.01em;
  }

  .toast-icon {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }

  .pulse-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #00e5ff;
    animation: pulse 1.2s ease-in-out infinite;
    flex-shrink: 0;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.4; transform: scale(0.8); }
  }
</style>
