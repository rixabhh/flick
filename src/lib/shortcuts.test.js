import { expect, test } from 'vitest';
import { normalizeShortcut, shortcutFromEvent } from './shortcuts.js';
test('normalizes custom key combinations and rejects unsafe or ambiguous input', () => {
  expect(normalizeShortcut('control + option + r')).toBe('Ctrl+Alt+R');
  expect(normalizeShortcut('Cmd+Shift+2')).toBe('Shift+Cmd+2');
  for (const invalid of ['', 'R', 'Shift+A', 'Ctrl++R', 'Ctrl+R+T', 'Alt+F4', 'Ctrl+Ctrl+R']) expect(() => normalizeShortcut(invalid)).toThrow();
});
test('records physical letters, digits, and space while ignoring modifier-only presses', () => {
  expect(shortcutFromEvent({ key: '®', code: 'KeyR', ctrlKey: true, altKey: true })).toBe('Ctrl+Alt+R');
  expect(shortcutFromEvent({ key: '@', code: 'Digit2', ctrlKey: true, shiftKey: true })).toBe('Ctrl+Shift+2');
  expect(shortcutFromEvent({ key: 'Control', code: 'ControlLeft', ctrlKey: true })).toBeNull();
});
