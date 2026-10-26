<script>
  import { invoke } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { Eye, EyeOff, CircleCheck, CircleAlert, ExternalLink } from "@lucide/svelte";

  let {
    provider = "gemini",
    model = "gemini-2.5-flash-lite",
    customBaseUrl = "",
    showTest = true,
    providerLabel = "",
    apiKeyUrl = "",
  } = $props();

  let apiKey = $state("");
  let masked = $state(true);
  let testing = $state(false);
  let testResult = $state(null); // null | 'success' | 'error'
  let testMessage = $state("");
  let saving = $state(false);
  let hasKey = $state(false);
  let loadingKey = $state(false);
  let generation = 0;
  let resultTimer;

  async function loadExistingKey(selectedProvider, currentGeneration) {
    loadingKey = true;
    try {
      const key = await invoke("load_api_key", { provider: selectedProvider });
      if (currentGeneration !== generation) return;
      if (key) {
        apiKey = key;
        hasKey = true;
      }
    } catch {
      if (currentGeneration === generation) hasKey = false;
    } finally {
      if (currentGeneration === generation) loadingKey = false;
    }
  }

  $effect(() => {
    const selectedProvider = provider;
    const currentGeneration = ++generation;
    apiKey = "";
    hasKey = false;
    saving = testing = false;
    masked = true;
    testResult = null;
    testMessage = "";
    loadExistingKey(selectedProvider, currentGeneration);
    return () => { generation += 1; clearTimeout(resultTimer); };
  });

  async function saveKey() {
    if (loadingKey || saving || !apiKey.trim()) return;
    const currentGeneration = generation;
    saving = true;
    try {
      await invoke("save_api_key", { key: apiKey.trim(), provider });
      if (currentGeneration !== generation) return;
      hasKey = true;
      testResult = "success";
      testMessage = "API key saved";
      clearResultAfterDelay();
    } catch (e) {
      if (currentGeneration !== generation) return;
      testResult = "error";
      testMessage = `Failed to save: ${e}`;
      clearResultAfterDelay();
    }
    if (currentGeneration === generation) saving = false;
  }

  async function testConnection() {
    if (loadingKey || testing || (provider !== "custom" && !apiKey.trim()) || (provider === "custom" && !customBaseUrl.trim())) return;
    const currentGeneration = generation;
    testing = true;
    testResult = null;
    try {
      await invoke("test_api_connection", {
        key: apiKey.trim(),
        provider,
        model,
        customBaseUrl,
      });
      if (currentGeneration !== generation) return;
      testResult = "success";
      testMessage = "Connection successful!";
    } catch (e) {
      if (currentGeneration !== generation) return;
      testResult = "error";
      testMessage = `Connection failed: ${e}`;
    }
    testing = false;
    clearResultAfterDelay();
  }

  function clearResultAfterDelay() {
    clearTimeout(resultTimer);
    resultTimer = setTimeout(() => {
      testResult = null;
      testMessage = "";
    }, 3000);
  }

  function toggleMask() {
    masked = !masked;
  }

  function providerName() {
    if (providerLabel) return providerLabel;
    if (provider === "openrouter") return "OpenRouter";
    if (provider === "custom") return "OpenAI-compatible endpoint";
    return "Gemini";
  }

  function keyPlaceholder() {
    return `Enter your ${providerName()} API key`;
  }

  function keyLink() {
    if (apiKeyUrl) return apiKeyUrl;
    if (provider === "custom") return "";
    return provider === "openrouter"
      ? "https://openrouter.ai/settings/keys"
      : "https://aistudio.google.com/app/apikey";
  }

  async function openKeyLink(event) {
    event.preventDefault();
    try {
      await openUrl(keyLink());
    } catch (e) {
      console.error("Failed to open API key link:", e);
    }
  }
</script>

<div class="api-section">
  <div class="input-row">
    <div class="input-wrapper">
      {#if masked}
        <input
          id="api-key-input"
          type="password"
          disabled={loadingKey || saving}
          bind:value={apiKey}
          placeholder={keyPlaceholder()}
          autocomplete="off"
        />
      {:else}
        <input
          id="api-key-input-visible"
          type="text"
          disabled={loadingKey || saving}
          bind:value={apiKey}
          placeholder={keyPlaceholder()}
          autocomplete="off"
          class="mono"
        />
      {/if}
      <button class="btn btn-ghost btn-sm eye-btn" onclick={toggleMask} title={masked ? "Show key" : "Hide key"}>
        {#if masked}
          <Eye size={16} strokeWidth={2} />
        {:else}
          <EyeOff size={16} strokeWidth={2} />
        {/if}
      </button>
    </div>
  </div>

  <div class="button-row">
    <button class="btn btn-primary btn-sm" onclick={saveKey} disabled={loadingKey || saving || !apiKey.trim()}>
      {#if saving}
        Saving…
      {:else}
        Save Key
      {/if}
    </button>
    {#if showTest}<button
      class="btn btn-secondary btn-sm"
      onclick={testConnection}
      disabled={loadingKey || testing || (provider !== "custom" && !apiKey.trim()) || (provider === "custom" && !customBaseUrl.trim())}
    >
      {#if testing}
        Testing…
      {:else}
        Test Connection
      {/if}
    </button>{/if}
  </div>

  {#if testResult}
    <div class="test-result" class:success={testResult === 'success'} class:error={testResult === 'error'}>
      {#if testResult === 'success'}
        <CircleCheck size={14} strokeWidth={2.2} />
      {:else}
        <CircleAlert size={14} strokeWidth={2.2} />
      {/if}
      <span>{testMessage}</span>
    </div>
  {/if}

  {#if provider !== "custom"}<a class="api-link" href={keyLink()} onclick={openKeyLink}>
    <ExternalLink size={12} strokeWidth={2} />
    Get a {providerName()} API key
  </a>{/if}
</div>

<style>
  .api-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
  }

  .input-row {
    display: flex;
    gap: var(--space-sm);
  }

  .input-wrapper {
    position: relative;
    flex: 1;
  }

  .input-wrapper input {
    padding-right: 36px;
  }

  .eye-btn {
    position: absolute;
    right: 4px;
    top: 50%;
    transform: translateY(-50%);
    padding: 4px;
    color: var(--text-muted);
  }

  .eye-btn:hover {
    color: var(--text-primary);
  }

  .button-row {
    display: flex;
    gap: var(--space-sm);
  }

  .test-result {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    padding: var(--space-sm) var(--space-md);
    border-radius: var(--radius);
    font-size: 0.8rem;
    animation: fadeIn 200ms ease;
  }

  .test-result.success {
    background: var(--success-dim);
    color: var(--success);
  }

  .test-result.error {
    background: var(--error-dim);
    color: var(--error);
  }

  .api-link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 0.8rem;
    color: var(--text-secondary);
    transition: color var(--transition-fast);
  }

  .api-link:hover {
    color: var(--accent);
    text-decoration: none;
  }
</style>
