// Internal helper — NOT re-exported from index.ts
export function internalHelper(): string {
  return "internal";
}

export function processInternalData(data: string): number {
  return data.length;
}
