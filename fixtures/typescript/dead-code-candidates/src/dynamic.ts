// Dynamic module — contains registry/plugin patterns
// Should trigger dynamic caution

const registry: Map<string, Function> = new Map();

export function registerPlugin(name: string, handler: Function): void {
  registry.set(name, handler);
}

export function getPlugin(name: string): Function | undefined {
  return registry.get(name);
}

function dynamicDispatch(name: string): unknown {
  const plugin = registry.get(name);
  if (plugin) {
    return plugin();
  }
  return null;
}
