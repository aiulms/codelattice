// App factory: imports routes and live handler
import { setupRoutes } from './routes';
import { liveHandler } from './live';

export function createApp() {
  const app = {
    start: () => {
      setupRoutes();
      liveHandler();
    }
  };
  return app;
}
