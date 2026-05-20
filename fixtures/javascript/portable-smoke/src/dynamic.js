const lazyModules = {
    dashboard: () => import('./dashboard.js'),
    settings: () => import('./settings.js'),
    profile: () => import('./profile.js')
};

export async function loadModule(name) {
    const loader = lazyModules[name];
    if (loader) {
        return await loader();
    }
    throw new Error(`Unknown module: ${name}`);
}

export function getAvailableModules() {
    return Object.keys(lazyModules);
}
