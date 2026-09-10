// Models the documented Tauri IPC/event boundary; no real keys, microphone,
// clipboard, network providers or user's application data are touched.
export async function mockDesktop(page, label = "settings", overrides = {}) {
  await page.addInitScript(({ label, overrides }) => {
    const callbacks = new Map();
    const listeners = new Map();
    let sequence = 0;
    const config = {
      version: 5, enabled: true, launch_at_login: false, show_done_toast: true,
      theme: "dark", app_language: "en", onboarding_complete: true,
      provider: "gemini", model: "gemini-2.5-flash-lite", custom_base_url: "",
      custom_commands: [], local_models: [], dictation_provider: "local-whisper",
      dictation_model_id: "whisper-tiny-en", floating_pill_position: "bottom-center",
      ...overrides,
    };
    const mock = window.__flickTest = {
      config, calls: [], pending: [], held: [], failures: {}, models: [], activeDownload: null,
      keys: { gemini: "fake-gemini-key", openrouter: "fake-router-key" },
      emit(event, payload) {
        for (const handler of listeners.get(event) || []) callbacks.get(handler)?.({ event, payload });
      },
      resolve(command, value, index = 0) {
        const matches = mock.pending.filter((item) => item.command === command);
        const item = matches[index];
        if (!item) throw new Error(`No pending ${command}`);
        mock.pending.splice(mock.pending.indexOf(item), 1);
        item.resolve(value);
      },
    };
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label }, currentWebview: { label } },
      transformCallback(callback) { callbacks.set(++sequence, callback); return sequence; },
      unregisterCallback(id) { callbacks.delete(id); },
      async invoke(command, args = {}) {
        mock.calls.push({ command, args: structuredClone(args) });
        if (command === "plugin:event|listen") {
          listeners.set(args.event, [...(listeners.get(args.event) || []), args.handler]);
          return args.handler;
        }
        if (command === "plugin:event|unlisten") return;
        if (mock.failures[command]) throw mock.failures[command];
        if (mock.held.includes(command)) return new Promise((resolve, reject) => mock.pending.push({ command, args, resolve, reject }));
        if (command === "get_config") return structuredClone(mock.config);
        if (command === "update_config_fields") { Object.assign(mock.config, args.patch); return structuredClone(mock.config); }
        if (command === "load_api_key") return mock.keys[args.provider] || "";
        if (command === "save_api_key") { mock.keys[args.provider] = args.key; return; }
        if (command === "list_local_models") return structuredClone(mock.models);
        if (command === "active_local_model_download") return mock.activeDownload;
        if (command === "set_shortcut_capture") return;
        if (command === "generate_reply") return "Thanks, that works for me.";
        if (command === "capture_reply_context") return "Selected conversation";
        if (command === "plugin:autostart|is_enabled") return false;
        if (command.startsWith("plugin:") || ["download_local_model", "cancel_local_model_download", "apply_floating_pill_position", "insert_reply", "copy_reply", "test_api_connection"].includes(command)) return;
        if (["get_history", "list_input_devices", "list_dictation_providers"].includes(command)) return [];
        if (["active_local_model_capabilities", "dictation_runtime_info"].includes(command)) return null;
        throw new Error(`Unmocked command: ${command}`);
      },
    };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener(event, id) { callbacks.delete(id); } };
  }, { label, overrides });
}

export const emit = (page, event, payload) => page.evaluate(({ event, payload }) => window.__flickTest.emit(event, payload), { event, payload });
export const calls = (page, command) => page.evaluate((command) => window.__flickTest.calls.filter((item) => item.command === command), command);
