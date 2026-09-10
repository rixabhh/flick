export function normalizeShortcut(value) {
  const mods = new Set();
  let key = "";
  for (const part of value.split("+")) {
    const token = part.trim().toUpperCase();
    const modifier = ({ CTRL: "Ctrl", CONTROL: "Ctrl", ALT: "Alt", OPTION: "Alt", SHIFT: "Shift", CMD: "Cmd", COMMAND: "Cmd", META: "Cmd" })[token];
    if (modifier) {
      if (mods.has(modifier)) throw new Error("Do not repeat a modifier.");
      mods.add(modifier);
    } else {
      if (key || !/^(?:[A-Z0-9]|SPACE|F(?:[1-9]|1[0-2]))$/.test(token)) throw new Error("Use one letter, number, Space, or F1–F12.");
      key = token === "SPACE" ? "Space" : token;
    }
  }
  if (!key || !["Ctrl", "Alt", "Cmd"].some((mod) => mods.has(mod))) throw new Error("Include Control, Alt/Option, or Command and one key.");
  const result = [...["Ctrl", "Alt", "Shift", "Cmd"].filter((mod) => mods.has(mod)), key].join("+");
  if (["Alt+F4", "Cmd+Q", "Cmd+L"].includes(result)) throw new Error("Choose a shortcut that is not reserved by the operating system.");
  return result;
}

export function shortcutFromEvent(event) {
  if (["Control", "Alt", "Shift", "Meta"].includes(event.key)) return null;
  const key = event.code === "Space" ? "Space" : event.code.replace(/^(Key|Digit)/, "");
  return normalizeShortcut([event.ctrlKey && "Ctrl", event.altKey && "Alt", event.shiftKey && "Shift", event.metaKey && "Cmd", key].filter(Boolean).join("+"));
}
