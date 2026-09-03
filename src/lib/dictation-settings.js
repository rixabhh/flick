export function parseCorrections(value) {
  return value
    .split("\n")
    .map((line) => line.split("=>"))
    .filter(([find, replace]) => find?.trim() && replace !== undefined)
    .map(([find, replace]) => ({ find: find.trim(), replace: replace.trim() }));
}

export function formatCorrections(corrections = []) {
  return corrections.map(({ find, replace }) => `${find} => ${replace}`).join("\n");
}
