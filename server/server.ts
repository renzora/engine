import Fastify from 'fastify';
import fastifyCors from '@fastify/cors';
import fastifyCookie from '@fastify/cookie';
import fastifyCompress from '@fastify/compress';
import fastifyView from '@fastify/view';
import path from 'path';
import nunjucks from 'nunjucks';
import { connectDB } from './database.ts';
import { initializeWebSocket } from './websocket.ts';
import { authMiddleware } from './middleware/auth.ts';
import { authRoutes } from './routes/auth.ts';
import { ajaxRoutes } from './routes/ajax.ts';
import { scenesRoutes } from './routes/scenes.ts';
import { serversRoutes } from './routes/servers.ts';
import { tilesetManagerRoutes } from './routes/tileset_manager.ts';
import { pluginRoutes } from './routes/plugins.ts';
import { editorRoutes } from './routes/editor.ts';
import { redis } from './redis.js';

const fastify = Fastify({ logger: false });
const PORT = Number(process.env.PORT) || 3000;

const viewPaths = [
  path.join(process.cwd(), 'client'),
  path.join(process.cwd(), 'client/plugins'),
];

nunjucks.configure(viewPaths, {
  autoescape: true,
  noCache: false,
});

await fastify.register(fastifyCompress, {
  global: true,
  encodings: ['gzip', 'deflate'],
  threshold: 1024,
});

await fastify.register(fastifyView, {
  engine: { nunjucks },
  root: viewPaths,
  viewExt: 'njk',
});

await fastify.register(fastifyCors, {
  origin:
    process.env.ALLOWED_ORIGINS?.split(',') || [
      'http://localhost',
      'http://localhost:80',
    ],
  credentials: true,
});

await fastify.register(fastifyCookie);

fastify.addHook('onRequest', authMiddleware);

fastify.register(authRoutes, { prefix: '/api/auth' });
fastify.register(ajaxRoutes, { prefix: '/api/ajax' });
fastify.register(scenesRoutes, { prefix: '/api/scenes' });
fastify.register(serversRoutes, { prefix: '/api/servers' });
fastify.register(tilesetManagerRoutes, { prefix: '/api/tileset_manager' });
fastify.register(pluginRoutes, { prefix: '/api/plugins' });
fastify.register(editorRoutes, { prefix: '/api/editor' });

async function startServer() {
  try {
    await connectDB();

    await redis.set('test-key', 'Hello from Redis!');
    const testVal = await redis.get('test-key');
    console.log('Redis test value:', testVal);

    const address = await fastify.listen({ port: PORT, host: '0.0.0.0' });
    console.log(`Server running at ${address}`);
  } catch (error) {
    console.error('Failed to start server:', error);
    process.exit(1);
  }
}

await startServer();

const wss = initializeWebSocket();

process.on('SIGTERM', () => {
  console.log('SIGTERM received. Shutting down gracefully...');
  fastify.close(() => {
    console.log('Server closed');
    process.exit(0);
  });
});

process.on('uncaughtException', (error) => {
  console.error('Uncaught Exception:', error);
  process.exit(1);
});

process.on('unhandledRejection', (reason, promise) => {
  console.error('Unhandled Rejection at:', promise, 'reason:', reason);
  process.exit(1);
});