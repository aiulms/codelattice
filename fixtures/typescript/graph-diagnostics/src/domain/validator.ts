// Domain layer — pure logic
export function validateInput(input: string): boolean {
  return input.length > 0 && input.length < 1000;
}

export function transform(input: string, config: { prefix: string }): string {
  return config.prefix + input.toUpperCase();
}

export function computeHash(input: string): number {
  let hash = 0;
  for (let i = 0; i < input.length; i++) {
    hash = ((hash << 5) - hash) + input.charCodeAt(i);
    hash |= 0;
  }
  return hash;
}
