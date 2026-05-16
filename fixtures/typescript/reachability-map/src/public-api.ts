// Public API: exported but not used internally
// Should get public-api caution when detected as unreachable candidate
export function publicUtility() {
  return 'public';
}

export function publicHelper() {
  return publicUtility();
}
