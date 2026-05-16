// Public API — exported from package entry point
export function createClient(config: Record<string, unknown>) {
  return new PublicClient(config);
}

export class PublicClient {
  private config: Record<string, unknown>;
  constructor(config: Record<string, unknown>) { this.config = config; }
  connect() { return "connected"; }
}

// CLI entry — referenced in package.json bin
export function cliMain() {
  const client = createClient({});
  client.connect();
}
