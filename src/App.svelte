<script>
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import Settings from "./lib/Settings.svelte";
  import Toast from "./lib/Toast.svelte";

  let windowLabel = $state("settings");

  async function detectWindow() {
    try {
      const win = getCurrentWindow();
      windowLabel = win.label;

      if (windowLabel === "settings") {
        await win.show();
        await win.setFocus();
      }
    } catch {
      windowLabel = "settings";
    }
  }

  onMount(() => {
    detectWindow();
  });
</script>

{#if windowLabel === "toast"}
  <Toast />
{:else}
  <Settings />
{/if}
