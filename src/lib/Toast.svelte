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
    <div class="toast toast-transforming">
      <span class="spinner" aria-hidden="true"></span>
      <span class="toast-text">Transforming</span>
    </div>
  {:else if state === "done"}
    <div class="toast toast-done">
      <svg class="toast-icon" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
      </svg>
      <span class="toast-text">Done</span>
    </div>
  {:else if state === "error"}
    <div class="toast toast-error">
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
    transform: translateY(4px);
    transition: opacity 160ms ease, transform 160ms ease;
  }

  .toast-container.visible {
    opacity: 1;
    transform: translateY(0);
  }

  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    width: fit-content;
    min-height: 42px;
    padding: 10px 14px;
    border: 1px solid var(--border-hover);
    border-radius: 10px;
    background: rgba(23, 25, 28, 0.96);
    box-shadow: 0 12px 28px rgba(0, 0, 0, 0.32);
    white-space: nowrap;
    font-size: 13px;
    font-weight: 600;
    font-family: var(--font-body);
    max-width: calc(100% - 16px);
  }

  .toast-transforming {
    color: var(--text-primary);
  }

  .toast-done {
    color: var(--success);
  }

  .toast-error {
    border-color: rgba(255, 107, 107, 0.32);
    color: var(--error);
  }

  .toast-text {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .toast-error .toast-text {
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    white-space: normal;
    overflow-wrap: anywhere;
  }

  .toast-icon {
    width: 15px;
    height: 15px;
    flex-shrink: 0;
  }

  .spinner {
    width: 15px;
    height: 15px;
    border: 2px solid var(--border-hover);
    border-top-color: var(--text-primary);
    border-radius: 50%;
    animation: spin 720ms linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
