// Service layer — core logic
import { transform } from "../domain/transform";
import { fetchConfig } from "../infra/config";

export function handleRequest(input: string): string {
  const config = fetchConfig();
  return transform(input, config);
}

export function handleBatch(inputs: string[]): string[] {
  return inputs.map(handleRequest);
}
