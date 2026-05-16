// Route setup: reachable from app
export function setupRoutes() {
  console.log('Routes configured');
}

export function handleRequest(path: string) {
  return { status: 200, path };
}
