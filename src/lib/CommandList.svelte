<script>
  import { invoke } from "@tauri-apps/api/core";

  const BUILTIN_COMMANDS = [
    { trigger: "!fix", description: "Fix grammar, spelling, and punctuation" },
    { trigger: "!formal", description: "Rewrite in formal, professional tone" },
    { trigger: "!casual", description: "Rewrite in casual, friendly tone" },
    { trigger: "!shorter", description: "Make text shorter and more concise" },
    { trigger: "!longer", description: "Expand with more detail and context" },
    { trigger: "!improve", description: "Improve clarity, flow, and readability" },
    { trigger: "!rephrase", description: "Rephrase keeping same meaning" },
    { trigger: "!bullet", description: "Convert into bullet point list" },
    { trigger: "!explain", description: "Rewrite in simple, easy language" },
    { trigger: "!translate:<lang>", description: "Translate to specified language" },
  ];

  let { customCommands = $bindable([]), onUpdate = () => {} } = $props();

  let editingIndex = $state(-1);
  let newTrigger = $state("");
  let newPrompt = $state("");
  let editTrigger = $state("");
  let editPrompt = $state("");
  let showAddForm = $state(false);
  let commandMessage = $state("");
  let commandMessageType = $state("error");

  const BUILTIN_TRIGGER_NAMES = new Set(
    BUILTIN_COMMANDS.map((cmd) => cmd.trigger.replace(/^!/, "").split(":")[0])
  );

  function normalizeTrigger(value) {
    return value.trim().replace(/^!/, "").toLowerCase();
  }

  function showMessage(message, type = "error") {
    commandMessage = message;
    commandMessageType = type;
  }

  function clearMessage() {
    commandMessage = "";
  }

  function validateCommand(trigger, prompt, currentIndex = -1) {
    if (!trigger) return "Add a trigger name.";
    if (!/^[a-z][a-z0-9_-]{1,31}$/.test(trigger)) {
      return "Use 2-32 lowercase letters, numbers, dashes, or underscores.";
    }
    if (BUILTIN_TRIGGER_NAMES.has(trigger)) {
      return `!${trigger} is already a built-in command.`;
    }
    if (!prompt) return "Add a system prompt.";
    if (prompt.length > 2000) {
      return "System prompt must be 2000 characters or fewer.";
    }
    const duplicateIndex = customCommands.findIndex((cmd) => cmd.trigger === trigger);
    if (duplicateIndex !== -1 && duplicateIndex !== currentIndex) {
      return `!${trigger} already exists.`;
    }
    return "";
  }

  function startAdd() {
    clearMessage();
    showAddForm = true;
    newTrigger = "";
    newPrompt = "Rewrite the text with better clarity while keeping the same language and meaning.";
    editingIndex = -1;
  }

  function cancelAdd() {
    clearMessage();
    showAddForm = false;
    newTrigger = "";
    newPrompt = "";
  }

  async function addCommand() {
    const trigger = normalizeTrigger(newTrigger);
    const prompt = newPrompt.trim();
    const validationError = validateCommand(trigger, prompt);
    if (validationError) {
      showMessage(validationError);
      return;
    }

    try {
      await invoke("add_custom_command", { trigger, prompt });
      customCommands = [...customCommands, { trigger, prompt }];
      onUpdate(customCommands);
      showMessage(`Added !${trigger}`, "success");
      showAddForm = false;
      newTrigger = "";
      newPrompt = "";
    } catch (e) {
      showMessage(`Failed to add command: ${e}`);
      console.error("Failed to add command:", e);
    }
  }

  function startEdit(index) {
    clearMessage();
    editingIndex = index;
    editTrigger = customCommands[index].trigger;
    editPrompt = customCommands[index].prompt;
    showAddForm = false;
  }

  function cancelEdit() {
    clearMessage();
    editingIndex = -1;
    editTrigger = "";
    editPrompt = "";
  }

  async function saveEdit(index) {
    const trigger = normalizeTrigger(editTrigger);
    const prompt = editPrompt.trim();
    const validationError = validateCommand(trigger, prompt, index);
    if (validationError) {
      showMessage(validationError);
      return;
    }

    try {
      await invoke("update_custom_command", { index, trigger, prompt });
      customCommands[index] = { trigger, prompt };
      customCommands = [...customCommands];
      onUpdate(customCommands);
      cancelEdit();
    } catch (e) {
      showMessage(`Failed to update command: ${e}`);
      console.error("Failed to update command:", e);
    }
  }

  async function deleteCommand(index) {
    try {
      await invoke("delete_custom_command", { index });
      customCommands = customCommands.filter((_, i) => i !== index);
      onUpdate(customCommands);
      if (editingIndex === index) cancelEdit();
    } catch (e) {
      showMessage(`Failed to delete command: ${e}`);
      console.error("Failed to delete command:", e);
    }
  }
</script>

<div class="commands-section">
  <!-- Built-in Commands -->
  <div class="subsection">
    <h3 class="subsection-title">Built-in Commands</h3>
    <div class="command-grid">
      {#each BUILTIN_COMMANDS as cmd}
        <div class="command-row">
          <span class="command-trigger mono badge badge-accent">{cmd.trigger}</span>
          <span class="command-desc">{cmd.description}</span>
        </div>
      {/each}
    </div>
  </div>

  <hr class="divider" />

  <!-- Custom Commands -->
  <div class="subsection">
    <div class="flex-between">
      <h3 class="subsection-title">Custom Commands</h3>
      {#if !showAddForm}
        <button class="btn btn-secondary btn-sm" onclick={startAdd}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="12" y1="5" x2="12" y2="19"/>
            <line x1="5" y1="12" x2="19" y2="12"/>
          </svg>
          Add Command
        </button>
      {/if}
    </div>

    {#if customCommands.length === 0 && !showAddForm}
      <div class="empty-state">
        <span class="text-muted">No custom commands yet. Add one to extend Flick with your own triggers.</span>
      </div>
    {/if}

    {#if commandMessage}
      <div class="command-message" class:success={commandMessageType === "success"}>
        {commandMessage}
      </div>
    {/if}

    {#each customCommands as cmd, i}
      <div class="custom-command" class:editing={editingIndex === i}>
        {#if editingIndex === i}
          <div class="edit-form animate-fade-in">
            <div class="edit-row">
              <label class="edit-label" for="edit-trigger-{i}">Trigger</label>
              <div class="trigger-input-row">
                <span class="trigger-prefix mono">!</span>
                <input id="edit-trigger-{i}" type="text" bind:value={editTrigger} placeholder="command name" class="mono" />
              </div>
            </div>
            <div class="edit-row">
              <label class="edit-label" for="edit-prompt-{i}">System prompt</label>
              <textarea id="edit-prompt-{i}" bind:value={editPrompt} rows="4" placeholder="Tell Flick what this command should do"></textarea>
            </div>
            <div class="edit-actions">
              <button class="btn btn-primary btn-sm" onclick={() => saveEdit(i)}>Save</button>
              <button class="btn btn-ghost btn-sm" onclick={cancelEdit}>Cancel</button>
            </div>
          </div>
        {:else}
          <div class="command-row custom-row">
            <span class="command-trigger mono badge badge-accent">!{cmd.trigger}</span>
            <span class="command-desc prompt-preview">{cmd.prompt}</span>
            <div class="command-actions">
              <button class="btn btn-ghost btn-sm" onclick={() => startEdit(i)} title="Edit">
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/>
                  <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/>
                </svg>
              </button>
              <button class="btn btn-ghost btn-sm text-error" onclick={() => deleteCommand(i)} title="Delete">
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="3 6 5 6 21 6"/>
                  <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                </svg>
              </button>
            </div>
          </div>
        {/if}
      </div>
    {/each}

    {#if showAddForm}
      <div class="edit-form new-form animate-slide-up">
        <div class="edit-row">
          <label class="edit-label" for="new-trigger">Trigger</label>
          <div class="trigger-input-row">
            <span class="trigger-prefix mono">!</span>
            <input id="new-trigger" type="text" bind:value={newTrigger} placeholder="e.g. summarize" class="mono" />
          </div>
        </div>
        <div class="edit-row">
          <label class="edit-label" for="new-prompt">System prompt</label>
          <textarea id="new-prompt" bind:value={newPrompt} rows="4" placeholder="Example: Summarize the text into three crisp bullets"></textarea>
        </div>
        <div class="edit-actions">
          <button class="btn btn-primary btn-sm" onclick={addCommand} disabled={!newTrigger.trim()}>
            Add
          </button>
          <button class="btn btn-ghost btn-sm" onclick={cancelAdd}>Cancel</button>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .commands-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-sm);
  }

  .subsection {
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
  }

  .subsection-title {
    font-size: 0.8rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-secondary);
  }

  .command-grid {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .command-row {
    display: flex;
    align-items: center;
    gap: var(--space-md);
    padding: 6px var(--space-sm);
    border-radius: var(--radius-sm);
    transition: background var(--transition-fast);
  }

  .command-row:hover {
    background: var(--bg-elevated);
  }

  .custom-row {
    padding: var(--space-sm);
  }

  .command-trigger {
    flex-shrink: 0;
    min-width: 110px;
  }

  .command-desc {
    font-size: 0.82rem;
    color: var(--text-secondary);
    flex: 1;
  }

  .prompt-preview {
    font-size: 0.78rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 200px;
  }

  .command-actions {
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity var(--transition-fast);
    flex-shrink: 0;
  }

  .command-row:hover .command-actions {
    opacity: 1;
  }

  .empty-state {
    text-align: center;
    padding: var(--space-xl);
    font-size: 0.85rem;
  }

  .command-message {
    padding: var(--space-sm) var(--space-md);
    border: 1px solid rgba(255, 107, 107, 0.28);
    border-radius: var(--radius);
    background: var(--error-dim);
    color: var(--error);
    font-size: 0.8rem;
  }

  .command-message.success {
    border-color: rgba(121, 217, 159, 0.28);
    background: var(--success-dim);
    color: var(--success);
  }

  .edit-form {
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
    padding: var(--space-lg);
    background: var(--bg-elevated);
    border-radius: var(--radius);
    border: 1px solid var(--border);
  }

  .new-form {
    border-color: var(--accent);
    border-style: dashed;
  }

  .edit-row {
    display: flex;
    flex-direction: column;
    gap: var(--space-xs);
  }

  .edit-label {
    font-size: 0.78rem;
    font-weight: 500;
    color: var(--text-secondary);
  }

  .trigger-input-row {
    display: flex;
    align-items: center;
    gap: 0;
  }

  .trigger-prefix {
    background: var(--bg-primary);
    border: 1px solid var(--border);
    border-right: none;
    border-radius: var(--radius) 0 0 var(--radius);
    padding: var(--space-sm) var(--space-sm);
    color: var(--accent);
    font-weight: 600;
    line-height: 1.5;
  }

  .trigger-input-row input {
    border-radius: 0 var(--radius) var(--radius) 0;
    flex: 1;
  }

  .edit-actions {
    display: flex;
    gap: var(--space-sm);
  }
</style>
