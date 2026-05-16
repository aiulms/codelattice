// API layer — entry point
import { handleRequest } from "../service/handler";
import { validateInput } from "../domain/validator";
import { logRequest } from "../infra/logger";

export function processRequest(input: string): string {
  logRequest(input);
  if (!validateInput(input)) {
    return "invalid";
  }
  return handleRequest(input);
}

export function healthCheck(): string {
  return "ok";
}
