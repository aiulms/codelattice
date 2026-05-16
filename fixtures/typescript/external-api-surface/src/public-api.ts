// Public API: externally consumable functions and classes

export function createClient(config: any): PublicClient {
  return new PublicClient(config);
}

export class PublicClient {
  private config: any;

  constructor(config: any) {
    this.config = config;
  }

  fetchData(): string {
    return "data";
  }
}

export function helperUsedInternally(): number {
  return 42;
}
