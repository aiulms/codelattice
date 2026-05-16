// Logger — shared utility
export function logRequest(msg: string): void {
  console.log(`[LOG] ${msg}`);
}

export function logError(msg: string): void {
  console.error(`[ERR] ${msg}`);
}
