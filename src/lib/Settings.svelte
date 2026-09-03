<script>
  import { invoke } from "@tauri-apps/api/core";
  import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import ApiKeyInput from "./ApiKeyInput.svelte";
  import CommandList from "./CommandList.svelte";
  import Models from "./Models.svelte";
  import History from "./History.svelte";
  import { formatCorrections, parseCorrections } from "./dictation-settings.js";
  import { supportedLanguages, translate } from "./i18n.js";
  import logoUrl from "../assets/flick-logo.png";

  let config = $state({
    enabled: true,
    launch_at_login: false,
    show_done_toast: true,
    theme: "system",
    app_language: "en",
    onboarding_complete: false,
    provider: "gemini",
    model: "gemini-2.5-flash-lite",
    custom_base_url: "",
    custom_commands: [],
    composer_shortcut: "Ctrl+Shift+Space",
    copy_last_result_shortcut: "Ctrl+Alt+C",
    paste_plain_text_shortcut: "Ctrl+Alt+V",
    dictation_shortcut: "Ctrl+Space",
    dictation_mode: "hold-or-toggle",
    dictation_device_id: "",
    dictation_model_id: "whisper-tiny-en",
    dictation_language: "en",
    dictation_translate_to_english: false,
    dictation_filler_cleanup: true,
    dictation_llm_post_process: false,
    dictation_corrections: [],
    append_trailing_space: false,
    history_enabled: true,
    retain_recordings: false,
    recording_retention_count: 20,
    history_limit: 100,
    local_models: [],
    disabled_apps: [],
    auto_submit_apps: [],
  });

  let activeTab = $state("home");
  let version = $state("2.0.0-beta");
  let inputDevices = $state([]);
  let inputLevel = $state(0);
  let dictationRuntime = $state(null);
  let diagnosticsPath = $state("");
  let keyRemovalMessage = $state("");
  let recordingDeletionMessage = $state("");
  let templatePath = $state("");
  let templateMessage = $state("");

  const tabs = [
    { id: "home", labelKey: "tab.home", icon: "info" },
    { id: "write", labelKey: "tab.write", icon: "key" },
    { id: "commands", labelKey: "tab.commands", icon: "terminal" },
    { id: "dictate", labelKey: "tab.dictate", icon: "sliders" },
    { id: "models", labelKey: "tab.models", icon: "terminal" },
    { id: "history", labelKey: "tab.history", icon: "info" },
    { id: "privacy", labelKey: "tab.privacy", icon: "info" },
    { id: "advanced", labelKey: "tab.advanced", icon: "sliders" },
  ];

  const t = (key) => translate(config.app_language, key);

  function selectTab(index) {
    activeTab = tabs[index].id;
  }

  function handleTabKeydown(event, index) {
    let next = index;
    if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
    else if (event.key === "ArrowLeft") next = (index - 1 + tabs.length) % tabs.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = tabs.length - 1;
    else return;
    event.preventDefault();
    selectTab(next);
    document.getElementById(`settings-tab-${tabs[next].id}`)?.focus();
  }

  const defaultModels = {
    gemini: "gemini-2.5-flash-lite",
    openrouter: "openrouter/free",
    custom: "",
  };

  async function refreshInputDevices() {
    try {
      inputDevices = await invoke("list_input_devices");
    } catch (error) {
      console.error("Failed to list microphones:", error);
      inputDevices = [];
    }
  }

  async function refreshInputLevel() {
    try {
      inputLevel = await invoke("preview_input_level");
    } catch (error) {
      console.error("Failed to test microphone level:", error);
      inputLevel = 0;
    }
  }

  async function refreshDictationRuntime() {
    try { dictationRuntime = await invoke("dictation_runtime_info"); }
    catch (error) { console.error("Failed to read dictation runtime:", error); }
  }

  function saveCorrections(value) {
    updateConfig("dictation_corrections", parseCorrections(value));
  }

  async function exportDiagnostics() {
    try { diagnosticsPath = await invoke("export_diagnostics"); }
    catch (error) { console.error("Failed to export diagnostics:", error); }
  }

  async function exportTemplates() {
    try {
      templatePath = await invoke("export_command_templates");
      templateMessage = "Templates exported. Keep this file private if its prompts are sensitive.";
    } catch (error) {
      templateMessage = `Could not export templates: ${error}`;
    }
  }

  async function importTemplates() {
    if (!templatePath.trim()) {
      templateMessage = "Enter the full path to a Flick template JSON file.";
      return;
    }
    try {
      const added = await invoke("import_command_templates", { path: templatePath.trim() });
      const cfg = await invoke("get_config");
      config = { ...config, ...cfg };
      templateMessage = added ? `Imported ${added} template${added === 1 ? "" : "s"}.` : "No new templates to import.";
    } catch (error) {
      templateMessage = `Could not import templates: ${error}`;
    }
  }

  async function removeApiKey() {
    if (!confirm("Remove the stored API key from your operating system keychain?")) return;
    try { await invoke("delete_api_key", { provider: config.provider }); keyRemovalMessage = "Stored API key removed."; }
    catch (error) { keyRemovalMessage = `Could not remove key: ${error}`; }
  }

  async function removeRecordings() {
    if (!confirm("Delete all locally retained Flick recording files? This cannot be undone.")) return;
    try { await invoke("clear_retained_recordings"); recordingDeletionMessage = "Retained recording files deleted."; }
    catch (error) { recordingDeletionMessage = `Could not delete recordings: ${error}`; }
  }

  function applyTheme(theme) {
    const resolved = theme === "system"
      ? (window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark")
      : theme;
    document.documentElement.dataset.theme = resolved;
  }

  function applyLanguage(language) {
    document.documentElement.lang = supportedLanguages.some((item) => item.id === language) ? language : "en";
  }

  async function completeOnboarding() {
    await updateConfig("onboarding_complete", true);
  }

  onMount(() => {
    // onMount only honours a synchronous cleanup return. Keep the async setup
    // inside a detached function so repeated settings-window mounts cannot
    // leave duplicate event listeners behind.
    let disposed = false;
    let unlisten = () => {};
    void (async () => {
      const disposeHistoryListener = await listen("flick://open-history", () => activeTab = "history");
      if (disposed) {
        disposeHistoryListener();
        return;
      }
      unlisten = disposeHistoryListener;
      try {
        const cfg = await invoke("get_config");
        if (cfg) {
          config = {
            ...config,
            ...cfg,
          };
        }
        applyTheme(config.theme);
        applyLanguage(config.app_language);
        refreshDictationRuntime();

        try {
          const autostartEnabled = await isEnabled();
          if (autostartEnabled !== config.launch_at_login) {
            config.launch_at_login = autostartEnabled;
            await invoke("save_config", { config });
          }
        } catch (autostartError) {
          console.error("Failed to read launch-at-login state:", autostartError);
        }
      } catch (e) {
        console.error("Failed to load config:", e);
      }
    })();
    return () => {
      disposed = true;
      unlisten();
    };
  });

  async function updateConfig(field, value) {
    config[field] = value;
    if (field === "theme") applyTheme(value);
    if (field === "app_language") applyLanguage(value);
    try {
      await invoke("save_config", { config });
    } catch (e) {
      console.error("Failed to save config:", e);
    }
  }

  async function handleProviderChange() {
    const currentModel = config.model || "";
    if (config.provider === "gemini" && !currentModel.startsWith("gemini-")) {
      config.model = defaultModels.gemini;
    }
    if (config.provider === "openrouter" && currentModel.startsWith("gemini-")) {
      config.model = defaultModels.openrouter;
    }
    if (config.provider === "custom" && !currentModel) config.model = "local-model";

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
    const previousVal = config.launch_at_login;
    config.launch_at_login = newVal;

    try {
      if (newVal) {
        await enable();
      } else {
        await disable();
      }
      await invoke("save_config", { config });
    } catch (e) {
      config.launch_at_login = previousVal;
      console.error("Failed to update launch at login:", e);
    }
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

  async function openExternal(event, url) {
    event.preventDefault();
    try {
      await openUrl(url);
    } catch (e) {
      console.error("Failed to open external link:", e);
    }
  }
</script>

<div class="settings-window">
  <!-- Title bar -->
  <div class="title-bar" data-tauri-drag-region>
    <div class="title-bar-content">
      <div class="app-brand">
        <span class="app-icon" aria-hidden="true">
          <img src={logoUrl} alt="" />
        </span>
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
  <div class="tab-nav" role="tablist" aria-label="Flick settings sections">
    {#each tabs as tab, index}
      <button
        id={`settings-tab-${tab.id}`}
        role="tab"
        aria-selected={activeTab === tab.id}
        aria-controls="settings-tabpanel"
        class="tab-btn"
        class:active={activeTab === tab.id}
        onclick={() => selectTab(index)}
        onkeydown={(event) => handleTabKeydown(event, index)}
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
        <span>{t(tab.labelKey)}</span>
      </button>
    {/each}
  </div>

  <!-- Tab Content -->
  <div id="settings-tabpanel" class="tab-content" role="tabpanel" aria-labelledby={`settings-tab-${activeTab}`} tabindex="-1">
    {#if activeTab === "home"}
      <div class="panel-section animate-fade-in">
        <div class="section-header"><h2 class="section-title">{t("home.title")}</h2></div>
        {#if !config.onboarding_complete}
          <div class="panel quick-card setup-card">
            <span class="badge badge-accent">{t("home.setup")}</span>
            <strong>Get Flick ready in three private steps</strong>
            <ol>
              <li><button class="setup-link" onclick={() => activeTab = "write"}>Choose an AI provider</button> for writing and reply drafts.</li>
              <li><button class="setup-link" onclick={() => activeTab = "models"}>Download a verified local speech model</button> for offline dictation.</li>
              <li><button class="setup-link" onclick={() => { activeTab = "dictate"; refreshInputDevices(); }}>Choose a microphone</button>; your operating system may ask for permission the first time you record.</li>
            </ol>
            <button class="btn btn-primary btn-sm" onclick={completeOnboarding}>I’ll finish this later</button>
          </div>
        {/if}
        <p class="section-desc">Flick is ready for text commands and AI replies. Select a message anywhere, then press <span class="mono">{config.composer_shortcut}</span> to draft a reply.</p>
        <div class="panel quick-card"><span class="badge badge-accent">Reply composer</span><strong>Selected-text context only</strong><span class="text-secondary">Nothing is read from the screen or saved as conversation history.</span></div>
        <div class="panel quick-card"><span class="badge badge-muted">Offline dictation</span><strong>Local speech setup</strong><span class="text-secondary">Shortcut: <span class="mono">{config.dictation_shortcut}</span> · mode: {config.dictation_mode}</span></div>
      </div>
    {:else if activeTab === "write"}
      <div class="panel-section animate-fade-in">
        <div class="section-header">
          <svg class="section-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/>
          </svg>
          <h2 class="section-title">API Configuration</h2>
        </div>
        <p class="section-desc">
          Choose the provider and model used for text transformations.
          Your API key is stored securely in the OS keychain.
        </p>
        <div class="provider-controls">
          <label>
            <span>Provider</span>
            <select bind:value={config.provider} onchange={handleProviderChange}>
              <option value="gemini">Gemini</option>
              <option value="openrouter">OpenRouter</option>
              <option value="custom">OpenAI-compatible / local</option>
            </select>
          </label>
          <label>
            <span>Model</span>
            {#if config.provider === "gemini"}
              <select bind:value={config.model} onchange={() => updateConfig("model", config.model)}>
                <option value="gemini-2.5-flash-lite">gemini-2.5-flash-lite</option>
                <option value="gemini-3-flash-preview">gemini-3-flash-preview</option>
                <option value="gemini-3.1-flash-lite-preview">gemini-3.1-flash-lite-preview</option>
              </select>
            {:else}
              <input
                type="text"
                bind:value={config.model}
                oninput={() => updateConfig("model", config.model)}
                placeholder="e.g. openai/gpt-4o-mini"
              />
            {/if}
          </label>
        </div>
        {#if config.provider === "custom"}
          <label class="custom-url"><span>Base URL</span><input type="text" bind:value={config.custom_base_url} onblur={() => updateConfig("custom_base_url", config.custom_base_url)} placeholder="http://localhost:11434/v1" /></label>
        {/if}
        <ApiKeyInput provider={config.provider} model={config.model} customBaseUrl={config.custom_base_url} />
        {#if config.provider === "custom"}<p class="section-desc">A key is optional for local endpoints. Leave it blank if your endpoint does not require authentication.</p>{/if}
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
          Type any trigger at the end of your text to transform it instantly. Custom commands let you define your own instruction prompts.
        </p>
        <CommandList
          bind:customCommands={config.custom_commands}
          onUpdate={handleCustomCommandsUpdate}
        />
      </div>

    {:else if activeTab === "dictate"}
      <div class="panel-section animate-fade-in">
        <div class="section-header"><h2 class="section-title">Dictation</h2></div>
        <p class="section-desc">Configure the local speech workflow. Audio and transcript handling remain on this device.</p>
        <label class="setting-field"><span>Dictation shortcut</span><input type="text" bind:value={config.dictation_shortcut} onblur={() => updateConfig("dictation_shortcut", config.dictation_shortcut)} /></label>
        <label class="setting-field"><span>Activation</span><select bind:value={config.dictation_mode} onchange={() => updateConfig("dictation_mode", config.dictation_mode)}><option value="hold-or-toggle">Hold or toggle</option><option value="push-to-talk">Push to talk</option><option value="toggle">Toggle</option></select></label>
        <label class="setting-field"><span>Microphone</span><select bind:value={config.dictation_device_id} onchange={() => updateConfig("dictation_device_id", config.dictation_device_id)}><option value="">System default</option>{#each inputDevices as device}<option value={device.id}>{device.name}{device.is_default ? " (default)" : ""}</option>{/each}</select></label>
        <label class="setting-field"><span>Spoken language</span><select bind:value={config.dictation_language} onchange={() => updateConfig("dictation_language", config.dictation_language)}><option value="en">English</option><option value="es">Spanish</option><option value="hi">Hindi</option><option value="fr">French</option><option value="de">German</option><option value="auto">Detect automatically</option></select></label>
        <div class="toggle-container"><div class="toggle-label"><span class="toggle-label-text">Translate speech to English</span><span class="toggle-label-desc">Requires the multilingual model</span></div><label class="toggle"><input type="checkbox" checked={config.dictation_translate_to_english} onchange={() => updateConfig("dictation_translate_to_english", !config.dictation_translate_to_english)} /><span class="toggle-slider"></span></label></div>
        <div class="toggle-container"><div class="toggle-label"><span class="toggle-label-text">Remove common filler words</span><span class="toggle-label-desc">Locally removes um, uh, erm, and ah from final text</span></div><label class="toggle"><input type="checkbox" checked={config.dictation_filler_cleanup} onchange={() => updateConfig("dictation_filler_cleanup", !config.dictation_filler_cleanup)} /><span class="toggle-slider"></span></label></div>
        <div class="toggle-container"><div class="toggle-label"><span class="toggle-label-text">AI cleanup after transcription</span><span class="toggle-label-desc">Off by default. Sends only the finished transcript to your configured provider; audio stays local.</span></div><label class="toggle"><input type="checkbox" checked={config.dictation_llm_post_process} onchange={() => updateConfig("dictation_llm_post_process", !config.dictation_llm_post_process)} /><span class="toggle-slider"></span></label></div>
        <div class="toggle-container"><div class="toggle-label"><span class="toggle-label-text">Keep local recording files</span><span class="toggle-label-desc">Off by default. Saves a local WAV after dictation for review.</span></div><label class="toggle"><input type="checkbox" checked={config.retain_recordings} onchange={() => updateConfig("retain_recordings", !config.retain_recordings)} /><span class="toggle-slider"></span></label></div>
        {#if config.retain_recordings}<label class="setting-field"><span>Recording retention</span><select bind:value={config.recording_retention_count} onchange={() => updateConfig("recording_retention_count", Number(config.recording_retention_count))}><option value={5}>5 recordings</option><option value={20}>20 recordings</option><option value={50}>50 recordings</option></select></label>{/if}
        <label class="setting-field"><span>Personal corrections</span><textarea value={formatCorrections(config.dictation_corrections)} onblur={(event) => saveCorrections(event.currentTarget.value)} placeholder="Acme => ACME&#10;Jon => John"></textarea><small>One replacement per line: <span class="mono">find =&gt; replacement</span></small></label>
        <div class="microphone-tools"><button class="btn btn-secondary" onclick={refreshInputDevices}>Refresh microphones</button><button class="btn btn-secondary" onclick={refreshInputLevel}>Test microphone</button><div class="input-level" aria-label="Measured microphone input level"><span style={`width: ${Math.min(100, inputLevel * 100)}%`}></span></div></div>
        {#if dictationRuntime}<div class="panel quick-card"><strong>Local runtime: {dictationRuntime.acceleration}</strong><span class="text-secondary">{dictationRuntime.details}</span><button class="btn btn-secondary btn-sm" onclick={refreshDictationRuntime}>Refresh runtime</button></div>{/if}
        <div class="toggle-container"><div class="toggle-label"><span class="toggle-label-text">Add trailing space</span><span class="toggle-label-desc">Add a space after a pasted transcript</span></div><label class="toggle"><input type="checkbox" checked={config.append_trailing_space} onchange={() => updateConfig("append_trailing_space", !config.append_trailing_space)} /><span class="toggle-slider"></span></label></div>
        <label class="setting-field"><span>Auto-submit after dictation</span><input type="text" value={config.auto_submit_apps.join(", ")} onblur={(event) => updateConfig("auto_submit_apps", event.currentTarget.value.split(",").map((value) => value.trim()).filter(Boolean))} placeholder="Optional app names, comma-separated" /><small>Off by default. Flick presses Enter only after a successful paste into one of these apps.</small></label>
        <div class="panel quick-card"><strong>Local models</strong><span class="text-secondary">Install a verified speech model from the Models section once the selected engine is available for this platform.</span></div>
      </div>
    {:else if activeTab === "models"}
      <div class="panel-section animate-fade-in"><Models /></div>
    {:else if activeTab === "history"}
      <div class="panel-section animate-fade-in"><History /></div>
    {:else if activeTab === "privacy"}
      <div class="panel-section animate-fade-in">
        <div class="section-header"><h2 class="section-title">{t("privacy.title")}</h2></div>
        <div class="panel quick-card"><strong>Reply context is explicit</strong><span class="text-secondary">Flick uses only selected or manually supplied context. Drafts and context are not persisted.</span></div>
        <div class="panel quick-card"><strong>Protected targets</strong><span class="text-secondary">Flick refuses actions in recognized dedicated credential-manager apps and honors your app exclusion list. On Windows, it also checks the focused control’s native password flag. It never reads password-field contents.</span></div>
        <div class="panel quick-card"><strong>Provider credential</strong><span class="text-secondary">Your API key is kept in the operating system keychain, not Flick’s settings file.</span><button class="btn btn-secondary" onclick={removeApiKey}>Remove stored API key</button>{#if keyRemovalMessage}<small>{keyRemovalMessage}</small>{/if}</div>
        <div class="panel quick-card"><strong>Retained recordings</strong><span class="text-secondary">Deletes only the opt-in local WAV files captured by Flick; transcript history is unaffected.</span><button class="btn btn-secondary" onclick={removeRecordings}>Delete retained recordings</button>{#if recordingDeletionMessage}<small>{recordingDeletionMessage}</small>{/if}</div>
        <div class="toggle-container"><div class="toggle-label"><span class="toggle-label-text">Keep local activity history</span><span class="toggle-label-desc">Controls future transcript and action history.</span></div><label class="toggle"><input type="checkbox" checked={config.history_enabled} onchange={() => updateConfig("history_enabled", !config.history_enabled)} /><span class="toggle-slider"></span></label></div>
        <label class="setting-field"><span>Unsaved history limit</span><select bind:value={config.history_limit} onchange={() => updateConfig("history_limit", Number(config.history_limit))}><option value={25}>25 items</option><option value={100}>100 items</option><option value={500}>500 items</option></select></label>
        <label class="setting-field"><span>Apps to exclude</span><input type="text" value={config.disabled_apps.join(", ")} onblur={(event) => updateConfig("disabled_apps", event.currentTarget.value.split(",").map((value) => value.trim()).filter(Boolean))} placeholder="Comma-separated app names" /></label>
      </div>
    {:else if activeTab === "advanced"}
      <div class="panel-section animate-fade-in">
        <div class="section-header"><h2 class="section-title">Advanced</h2></div>
        <div class="toggle-container"><div class="toggle-label"><span class="toggle-label-text">Enable Flick</span><span class="toggle-label-desc">Listen for commands and the reply shortcut</span></div><label class="toggle"><input type="checkbox" checked={config.enabled} onchange={toggleEnabled} /><span class="toggle-slider"></span></label></div>
        <div class="toggle-container"><div class="toggle-label"><span class="toggle-label-text">Launch at login</span><span class="toggle-label-desc">Start Flick automatically when you sign in</span></div><label class="toggle"><input type="checkbox" checked={config.launch_at_login} onchange={toggleLaunchAtLogin} /><span class="toggle-slider"></span></label></div>
        <div class="toggle-container"><div class="toggle-label"><span class="toggle-label-text">Show completion toast</span><span class="toggle-label-desc">Confirm successful transformations</span></div><label class="toggle"><input type="checkbox" checked={config.show_done_toast} onchange={toggleShowDoneToast} /><span class="toggle-slider"></span></label></div>
        <label class="setting-field"><span>Copy last result shortcut</span><input type="text" bind:value={config.copy_last_result_shortcut} onblur={() => updateConfig("copy_last_result_shortcut", config.copy_last_result_shortcut)} /><small>Copies the newest optional local history entry. Default: Ctrl+Alt+C.</small></label>
        <label class="setting-field"><span>Paste as plain text shortcut</span><input type="text" bind:value={config.paste_plain_text_shortcut} onblur={() => updateConfig("paste_plain_text_shortcut", config.paste_plain_text_shortcut)} /><small>Pastes the clipboard’s text representation without source formatting. Default: Ctrl+Alt+V.</small></label>
        <label class="setting-field"><span>Theme</span><select bind:value={config.theme} onchange={() => updateConfig("theme", config.theme)}><option value="system">System</option><option value="dark">Dark</option><option value="light">Light</option></select></label>
        <label class="setting-field"><span>{t("settings.language")}</span><select bind:value={config.app_language} onchange={() => updateConfig("app_language", config.app_language)}>{#each supportedLanguages as language}<option value={language.id}>{t(language.labelKey)}</option>{/each}</select><small>{t("language.note")}</small></label>
        <div class="panel quick-card"><strong>Diagnostics</strong><span class="text-secondary">Creates a local support bundle without API keys, drafts, clipboard data, or transcript history.</span><button class="btn btn-secondary" onclick={exportDiagnostics}>Export diagnostics</button>{#if diagnosticsPath}<small class="mono">Saved: {diagnosticsPath}</small>{/if}</div>
        <div class="panel quick-card"><strong>Command templates</strong><span class="text-secondary">Export or add validated custom command templates. API keys and history are never included.</span><button class="btn btn-secondary" onclick={exportTemplates}>Export templates</button><label class="setting-field"><span>Import JSON path</span><input type="text" bind:value={templatePath} placeholder="C:\\path\\to\\flick-command-templates.json" /></label><button class="btn btn-secondary" onclick={importTemplates}>Import templates</button>{#if templateMessage}<small>{templateMessage}</small>{/if}</div>
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
            <span class="about-icon" aria-hidden="true">
              <img src={logoUrl} alt="" />
            </span>
            <h2 class="about-name">Flick</h2>
            <span class="about-tagline">Type. Trigger. Done.</span>
            <span class="about-version badge badge-muted">v{version}</span>
          </div>

          <div class="about-links">
            <a class="about-link" href="https://github.com/rixabhh/flick" onclick={(event) => openExternal(event, "https://github.com/rixabhh/flick")}>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
              </svg>
              Repository
            </a>
            <a class="about-link" href="https://github.com/rixabhh" onclick={(event) => openExternal(event, "https://github.com/rixabhh")}>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
              </svg>
              rixabhh
            </a>
          </div>

          <p class="about-footer text-muted">
            Built with Rust + Tauri + Svelte<br/>
            Built by <a href="https://github.com/rixabhh" onclick={(event) => openExternal(event, "https://github.com/rixabhh")}>Rishabh</a>
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
    background: var(--bg-primary);
    border-bottom: 1px solid var(--border);
    padding: 0 var(--space-lg);
    height: 48px;
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
    gap: 10px;
  }

  .app-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: 1px solid var(--border-hover);
    border-radius: 6px;
    background: var(--bg-surface);
    color: var(--text-primary);
    font-size: 0.76rem;
    font-weight: 700;
    overflow: hidden;
  }

  .app-icon img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .app-name {
    font-weight: 700;
    font-size: 0.95rem;
    color: var(--text-primary);
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
    gap: 4px;
    padding: 10px var(--space-lg);
    background: var(--bg-surface);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .tab-btn {
    display: flex;
    align-items: center;
    gap: 7px;
    min-height: 34px;
    padding: 7px var(--space-md);
    background: transparent;
    border: none;
    border-radius: 6px;
    color: var(--text-secondary);
    font-size: 0.8rem;
    font-weight: 600;
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
    color: var(--text-primary);
    background: var(--bg-elevated);
    box-shadow: inset 0 0 0 1px var(--border-hover);
  }

  /* ===== Tab Content ===== */
  .tab-content {
    flex: 1;
    overflow-y: auto;
    padding: 22px var(--space-xl) var(--space-xl);
  }

  .panel-section {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .section-desc {
    font-size: 0.82rem;
    color: var(--text-secondary);
    line-height: 1.6;
  }

  .provider-controls {
    display: grid;
    grid-template-columns: minmax(132px, 0.72fr) minmax(0, 1.28fr);
    gap: var(--space-md);
  }

  .provider-controls label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .setting-field, .custom-url {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .setting-field > span, .custom-url > span {
    color: var(--text-secondary);
    font-size: 0.74rem;
    font-weight: 700;
    text-transform: uppercase;
  }

  .quick-card {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: var(--space-lg);
  }

  .provider-controls label > span {
    color: var(--text-secondary);
    font-size: 0.74rem;
    font-weight: 700;
    text-transform: uppercase;
  }

  .toggles-list {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
  }

  .microphone-tools {
    display: grid;
    grid-template-columns: auto auto minmax(5rem, 1fr);
    align-items: center;
    gap: var(--space-sm);
    margin: var(--space-sm) 0 var(--space-md);
  }

  .input-level {
    height: 8px;
    border-radius: 999px;
    background: var(--bg-elevated);
    overflow: hidden;
  }

  .input-level span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
    transition: width 100ms linear;
  }

  /* ===== About ===== */
  .about-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 24px;
    padding: 28px 0;
  }

  .about-hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-sm);
  }

  .about-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 56px;
    height: 56px;
    border: 1px solid var(--border-hover);
    border-radius: 14px;
    background: var(--bg-surface);
    color: var(--text-primary);
    font-size: 1.4rem;
    font-weight: 800;
    overflow: hidden;
  }

  .about-icon img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .about-name {
    font-size: 1.5rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .about-tagline {
    font-size: 0.9rem;
    color: var(--text-secondary);
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
    border-color: var(--border-hover);
    color: var(--text-primary);
    background: var(--bg-elevated);
    text-decoration: none;
  }

  .about-footer {
    text-align: center;
    font-size: 0.78rem;
    line-height: 1.8;
  }

  .about-footer a {
    color: var(--text-secondary);
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .about-footer a:hover {
    color: var(--text-primary);
  }

  .setup-card {
    gap: var(--space-sm);
    border-color: var(--accent);
  }

  .setup-card ol {
    margin: 0;
    padding-left: 1.25rem;
    color: var(--text-secondary);
    font-size: 0.82rem;
    line-height: 1.65;
  }

  .setup-link {
    appearance: none;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--accent);
    font: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
</style>
