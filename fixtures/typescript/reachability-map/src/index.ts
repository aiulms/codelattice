// Entry point: imports app and starts it
import { createApp } from './app';
import { liveHandler } from './live';

const app = createApp();
app.start();

export { liveHandler };
