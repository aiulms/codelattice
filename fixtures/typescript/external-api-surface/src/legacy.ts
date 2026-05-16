// Legacy module — not exported from index, not in package.json
export function legacyHandler(): void {
  console.log("legacy");
}

export function deprecatedFunction(): void {
  console.log("deprecated");
}
