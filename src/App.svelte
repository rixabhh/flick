<script>
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Settings from "./lib/Settings.svelte";
  import Toast from "./lib/Toast.svelte";

  let windowLabel = $state("settings");

  async function detectWindow() {
    try {
      const win = getCurrentWindow();
      windowLabel = win.label;
    } catch {
      windowLabel = "settings";
    }
  }

  detectWindow();
</script>

{#if windowLabel === "toast"}
  <Toast />
{:else}
  <Settings />
{/if}
