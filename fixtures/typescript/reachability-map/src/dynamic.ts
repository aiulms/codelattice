// Dynamic patterns: contains registry/plugin patterns
// Should get dynamic-dispatch caution
const registry: Record<string, Function> = {};

export function registerPlugin(name: string, handler: Function) {
  registry[name] = handler;
}

export function dynamicDispatch(name: string) {
  const handler = registry[name];
  if (handler) {
    handler();
  }
}
