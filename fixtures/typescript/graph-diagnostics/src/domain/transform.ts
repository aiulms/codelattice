// Domain transform — separate module for cross-layer testing
import { computeHash } from "./validator";
import { fetchConfig } from "../infra/config";

export function transformWithHash(input: string): string {
  const hash = computeHash(input);
  return `hashed:${hash}`;
}

// Creates a cycle: domain -> infra -> domain
export function domainNeedsInfra(): string {
  return fetchConfig().prefix;
}
