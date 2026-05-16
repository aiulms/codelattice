// Infra layer — external dependencies
import { logRequest } from "./logger";

export function fetchConfig(): { prefix: string } {
  return { prefix: "APP_" };
}

export function saveResult(result: string): void {
  logRequest(result);
}

export function connectDatabase(): string {
  return "connected";
}
