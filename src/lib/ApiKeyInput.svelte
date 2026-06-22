<script>
  import { invoke } from "@tauri-apps/api/core";

  let { provider = "gemini", model = "gemini-2.5-flash-lite" } = $props();

  let apiKey = $state("");
  let masked = $state(true);
  let testing = $state(false);
  let testResult = $state(null); // null | 'success' | 'error'
  let testMessage = $state("");
  let saving = $state(false);
  let hasKey = $state(false);

  async function loadExistingKey() {
    try {
      const key = await invoke("load_api_key");
      if (key) {
        apiKey = key;
        hasKey = true;
      }
    } catch {
      hasKey = false;
    }
  }

  loadExistingKey();

  async function saveKey() {
    if (!apiKey.trim()) return;
    saving = true;
    try {
      await invoke("save_api_key", { key: apiKey.trim() });
      hasKey = true;
      testResult = "success";
      testMessage = "API key saved";
      clearResultAfterDelay();
    } catch (e) {
      testResult = "error";
      testMessage = `Failed to save: ${e}`;
      clearResultAfterDelay();
    }
    saving = false;
  }

  async function testConnection() {
    if (!apiKey.trim()) return;
    testing = true;
    testResult = null;
    try {
      await invoke("test_api_connection", {
        key: apiKey.trim(),
        provider,
        model,
      });
      testResult = "success";
      testMessage = "Connection successful!";
    } catch (e) {
      testResult = "error";
      testMessage = `Connection failed: ${e}`;
    }
    testing = false;
    clearResultAfterDelay();
  }

  function clearResultAfterDelay() {
    setTimeout(() => {
      testResult = null;
      testMessage = "";
    }, 3000);
  }

  function toggleMask() {
    masked = !masked;
  }
</script>

<div class="api-section">
  <div class="input-row">
    <div class="input-wrapper">
      {#if masked}
        <input
          id="api-key-input"
          type="password"
          bind:value={apiKey}
          placeholder="Enter your Gemini API key"
          autocomplete="off"
        />
      {:else}
        <input
          id="api-key-input-visible"
          type="text"
          bind:value={apiKey}
          placeholder="Enter your Gemini API key"
          autocomplete="off"
          class="mono"
        />
      {/if}
      <button class="btn btn-ghost btn-sm eye-btn" onclick={toggleMask} title={masked ? "Show key" : "Hide key"}>
        {#if masked}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
            <circle cx="12" cy="12" r="3"/>
          </svg>
        {:else}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"/>
            <line x1="1" y1="1" x2="23" y2="23"/>
          </svg>
        {/if}
      </button>
    </div>
  </div>

  <div class="button-row">
    <button class="btn btn-primary btn-sm" onclick={saveKey} disabled={saving || !apiKey.trim()}>
      {#if saving}
        Saving…
      {:else}
        Save Key
      {/if}
    </button>
    <button class="btn btn-secondary btn-sm" onclick={testConnection} disabled={testing || !apiKey.trim()}>
      {#if testing}
        Testing…
      {:else}
        Test Connection
      {/if}
    </button>
  </div>

  {#if testResult}
    <div class="test-result" class:success={testResult === 'success'} class:error={testResult === 'error'}>
      {#if testResult === 'success'}
        <svg width="14" height="14" viewBox="0 0 20 20" fill="currentColor">
          <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
        </svg>
      {:else}
        <svg width="14" height="14" viewBox="0 0 20 20" fill="currentColor">
          <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clip-rule="evenodd" />
        </svg>
      {/if}
      <span>{testMessage}</span>
    </div>
  {/if}

  <a class="api-link" href="https://aistudio.google.com/app/apikey" target="_blank" rel="noopener noreferrer">
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>
      <polyline points="15 3 21 3 21 9"/>
      <line x1="10" y1="14" x2="21" y2="3"/>
    </svg>
    Get a free Gemini API key
  </a>
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
