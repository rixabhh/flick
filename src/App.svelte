<script>
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import Settings from "./lib/Settings.svelte";
  import Toast from "./lib/Toast.svelte";
  import ReplyComposer from "./lib/ReplyComposer.svelte";
  import DictationOverlay from "./lib/DictationOverlay.svelte";

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

  async function applySavedTheme() {
    try {
      const config = await invoke("get_config");
      const theme = config.theme === "system"
        ? (window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark")
        : (config.theme || "dark");
      document.documentElement.dataset.theme = theme;
      document.documentElement.lang = config.app_language === "es" ? "es" : "en";
    } catch {
      document.documentElement.dataset.theme = "dark";
      document.documentElement.lang = "en";
    }
  }

  onMount(() => {
    detectWindow();
    applySavedTheme();
  });
</script>

{#if windowLabel === "toast"}
  <Toast />
{:else if windowLabel === "composer"}
  <ReplyComposer />
{:else if windowLabel === "dictation"}
  <DictationOverlay />
{:else}
  <Settings />
{/if}
