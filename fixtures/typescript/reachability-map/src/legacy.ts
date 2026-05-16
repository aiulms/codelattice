// Legacy module: NOT imported anywhere — unreachable
export function oldHelper() {
  return 'legacy';
}

export function legacyProcess() {
  return oldHelper();
}
