export class Logger {
  private tag: string = "App";

  static info(message: string): void {
    console.log(`[INFO] ${message}`);
  }

  static error(message: string): void {
    console.error(`[ERROR] ${message}`);
  }
}

export function formatLog(tag: string, msg: string): string {
  return `[${tag}] ${msg}`;
}
