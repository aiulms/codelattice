// Live module — used by index.ts
export function liveFunction(input: string): string {
  return processInput(input);
}

function processInput(input: string): string {
  return input.toUpperCase();
}
