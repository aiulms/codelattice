// CLI entry point — referenced in package.json bin
export function main(): void {
  console.log("CLI running");
}

export function parseArgs(args: string[]): Record<string, string> {
  return {};
}
