<script>
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import ApiKeyInput from "./ApiKeyInput.svelte";
  import CommandList from "./CommandList.svelte";

  let config = $state({
    enabled: true,
    launch_at_login: false,
    show_done_toast: true,
    custom_commands: [],
  });

  let activeTab = $state("api");
  let version = $state("1.0.0");

  const tabs = [
    { id: "api", label: "API Key", icon: "key" },
    { id: "commands", label: "Commands", icon: "terminal" },
    { id: "behavior", label: "Behavior", icon: "sliders" },
    { id: "about", label: "About", icon: "info" },
  ];

  onMount(async () => {
    try {
      const cfg = await invoke("get_config");
      if (cfg) config = cfg;
    } catch (e) {
      console.error("Failed to load config:", e);
    }
  });

  async function updateConfig(field, value) {
    config[field] = value;
    try {
      await invoke("save_config", { config });
    } catch (e) {
      console.error("Failed to save config:", e);
    }
  }

  async function toggleEnabled() {
    const newVal = !config.enabled;
    config.enabled = newVal;
    try {
      await invoke("toggle_enabled", { enabled: newVal });
    } catch (e) {
      console.error("Failed to toggle:", e);
    }
  }

  async function toggleLaunchAtLogin() {
    const newVal = !config.launch_at_login;
    await updateConfig("launch_at_login", newVal);
  }

  async function toggleShowDoneToast() {
    await updateConfig("show_done_toast", !config.show_done_toast);
  }

  function handleCustomCommandsUpdate(commands) {
    config.custom_commands = commands;
  }

  async function closeWindow() {
    try {
      const win = getCurrentWindow();
      await win.hide();
    } catch {}
  }
</script>

<div class="settings-window">
  <!-- Title bar -->
  <div class="title-bar" data-tauri-drag-region>
    <div class="title-bar-content">
      <div class="app-brand">
        <span class="app-icon">⚡</span>
        <span class="app-name">Flick</span>
      </div>
      <button class="close-btn" onclick={closeWindow} title="Close">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <line x1="18" y1="6" x2="6" y2="18"/>
          <line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
    </div>
  </div>

  <!-- Tab Navigation -->
  <nav class="tab-nav">
    {#each tabs as tab}
      <button
        class="tab-btn"
        class:active={activeTab === tab.id}
        onclick={() => activeTab = tab.id}
      >
        {#if tab.icon === "key"}
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/>
          </svg>
        {:else if tab.icon === "terminal"}
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="4 17 10 11 4 5"/>
            <line x1="12" y1="19" x2="20" y2="19"/>
          </svg>
        {:else if tab.icon === "sliders"}
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="4" y1="21" x2="4" y2="14"/>
            <line x1="4" y1="10" x2="4" y2="3"/>
            <line x1="12" y1="21" x2="12" y2="12"/>
            <line x1="12" y1="8" x2="12" y2="3"/>
            <line x1="20" y1="21" x2="20" y2="16"/>
            <line x1="20" y1="12" x2="20" y2="3"/>
            <line x1="1" y1="14" x2="7" y2="14"/>
            <line x1="9" y1="8" x2="15" y2="8"/>
            <line x1="17" y1="16" x2="23" y2="16"/>
          </svg>
        {:else}
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"/>
            <line x1="12" y1="16" x2="12" y2="12"/>
            <line x1="12" y1="8" x2="12.01" y2="8"/>
          </svg>
        {/if}
        <span>{tab.label}</span>
      </button>
    {/each}
  </nav>

  <!-- Tab Content -->
  <div class="tab-content">
    {#if activeTab === "api"}
      <div class="panel-section animate-fade-in">
        <div class="section-header">
          <svg class="section-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/>
          </svg>
          <h2 class="section-title">API Configuration</h2>
        </div>
        <p class="section-desc">
          Flick uses Google's Gemini Flash model for text transformations.
          Your API key is stored securely in the OS keychain.
        </p>
        <ApiKeyInput />
      </div>

    {:else if activeTab === "commands"}
      <div class="panel-section animate-fade-in">
        <div class="section-header">
          <svg class="section-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="4 17 10 11 4 5"/>
            <line x1="12" y1="19" x2="20" y2="19"/>
          </svg>
          <h2 class="section-title">Commands</h2>
        </div>
        <p class="section-desc">
          Type any trigger at the end of your text to transform it instantly. Custom commands let you define your own prompts.
        </p>
        <CommandList
          bind:customCommands={config.custom_commands}
          onUpdate={handleCustomCommandsUpdate}
        />
      </div>

    {:else if activeTab === "behavior"}
      <div class="panel-section animate-fade-in">
        <div class="section-header">
          <svg class="section-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="4" y1="21" x2="4" y2="14"/>
            <line x1="4" y1="10" x2="4" y2="3"/>
            <line x1="12" y1="21" x2="12" y2="12"/>
            <line x1="12" y1="8" x2="12" y2="3"/>
            <line x1="20" y1="21" x2="20" y2="16"/>
            <line x1="20" y1="12" x2="20" y2="3"/>
            <line x1="1" y1="14" x2="7" y2="14"/>
            <line x1="9" y1="8" x2="15" y2="8"/>
            <line x1="17" y1="16" x2="23" y2="16"/>
          </svg>
          <h2 class="section-title">Behavior</h2>
        </div>

        <div class="toggles-list">
          <div class="toggle-container">
            <div class="toggle-label">
              <span class="toggle-label-text">Enable Flick</span>
              <span class="toggle-label-desc">Listen for triggers and transform text</span>
            </div>
            <label class="toggle">
              <input type="checkbox" checked={config.enabled} onchange={toggleEnabled} />
              <span class="toggle-slider"></span>
            </label>
          </div>

          <hr class="divider" />

          <div class="toggle-container">
            <div class="toggle-label">
              <span class="toggle-label-text">Launch at login</span>
              <span class="toggle-label-desc">Start Flick automatically when you log in</span>
            </div>
            <label class="toggle">
              <input type="checkbox" checked={config.launch_at_login} onchange={toggleLaunchAtLogin} />
              <span class="toggle-slider"></span>
            </label>
          </div>

          <hr class="divider" />

          <div class="toggle-container">
            <div class="toggle-label">
              <span class="toggle-label-text">Show "Done" toast</span>
              <span class="toggle-label-desc">Brief confirmation after a successful transform</span>
            </div>
            <label class="toggle">
              <input type="checkbox" checked={config.show_done_toast} onchange={toggleShowDoneToast} />
              <span class="toggle-slider"></span>
            </label>
          </div>
        </div>
      </div>

    {:else if activeTab === "about"}
      <div class="panel-section animate-fade-in">
        <div class="about-content">
          <div class="about-hero">
            <span class="about-icon">⚡</span>
            <h2 class="about-name">Flick</h2>
            <span class="about-tagline">Type. Trigger. Done.</span>
            <span class="about-version badge badge-muted">v{version}</span>
          </div>

          <div class="about-links">
            <a class="about-link" href="https://github.com/flick-app/flick" target="_blank" rel="noopener noreferrer">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
              </svg>
              GitHub
            </a>
          </div>

          <p class="about-footer text-muted">
            Built with Rust + Tauri + Svelte<br/>
            Powered by Gemini Flash
          </p>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .settings-window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg-primary);
    overflow: hidden;
  }

  /* ===== Title Bar ===== */
  .title-bar {
    -webkit-app-region: drag;
    background: var(--bg-surface);
    border-bottom: 1px solid var(--border);
    padding: 0 var(--space-lg);
    height: 44px;
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  .title-bar-content {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
  }

  .app-brand {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
  }

  .app-icon {
    font-size: 16px;
  }

  .app-name {
    font-weight: 700;
    font-size: 0.95rem;
    letter-spacing: 0.02em;
    background: linear-gradient(135deg, var(--accent), var(--accent-dim));
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  .close-btn {
    -webkit-app-region: no-drag;
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 6px;
    border-radius: var(--radius-sm);
    transition: all var(--transition-fast);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    background: var(--bg-elevated);
    color: var(--text-primary);
  }

  /* ===== Tab Navigation ===== */
  .tab-nav {
    display: flex;
    gap: 2px;
    padding: var(--space-sm) var(--space-lg);
    background: var(--bg-surface);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .tab-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: var(--space-sm) var(--space-md);
    background: transparent;
    border: none;
    border-radius: var(--radius);
    color: var(--text-secondary);
    font-size: 0.82rem;
    font-weight: 500;
    cursor: pointer;
    transition: all var(--transition-fast);
    flex: 1;
    justify-content: center;
  }

  .tab-btn:hover {
    color: var(--text-primary);
    background: var(--bg-elevated);
  }

  .tab-btn.active {
    color: var(--accent);
    background: var(--accent-glow);
  }

  /* ===== Tab Content ===== */
  .tab-content {
    flex: 1;
    overflow-y: auto;
    padding: var(--space-xl);
  }

  .panel-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-lg);
  }

  .section-desc {
    font-size: 0.82rem;
    color: var(--text-secondary);
    line-height: 1.6;
  }

  .toggles-list {
    display: flex;
    flex-direction: column;
  }

  /* ===== About ===== */
  .about-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2xl);
    padding: var(--space-2xl) 0;
  }

  .about-hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-sm);
  }

  .about-icon {
    font-size: 48px;
    filter: drop-shadow(0 0 20px rgba(0, 229, 255, 0.3));
  }

  .about-name {
    font-size: 1.8rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    background: linear-gradient(135deg, var(--accent), #00ffcc);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  .about-tagline {
    font-size: 0.9rem;
    color: var(--text-secondary);
    font-style: italic;
  }

  .about-version {
    margin-top: var(--space-xs);
  }

  .about-links {
    display: flex;
    gap: var(--space-lg);
  }

  .about-link {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    padding: var(--space-sm) var(--space-lg);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--text-secondary);
    font-size: 0.85rem;
    transition: all var(--transition-fast);
    text-decoration: none;
  }

  .about-link:hover {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--accent-glow);
    text-decoration: none;
  }

  .about-footer {
    text-align: center;
    font-size: 0.78rem;
    line-height: 1.8;
  }
</style>
