import path from 'path';
import Fastify from 'fastify';
import fastifyCors from '@fastify/cors';
import fastifyCookie from '@fastify/cookie';
import fastifyCompress from '@fastify/compress';
import fastifyFormbody from '@fastify/formbody';
import fastifyView from '@fastify/view';
import nunjucks from 'nunjucks';
import dotenv from 'dotenv';
import { connectDB } from './database.js';
import { initializeWebSocket } from './websocket.js';
import { authMiddleware } from './middleware/auth.js';
import { authRoutes } from './routes/auth.js';
import { ajaxRoutes } from './routes/ajax.js';
import { scenesRoutes } from './routes/scenes.js';
import { serversRoutes } from './routes/servers.js';
import { tilesetManagerRoutes } from './routes/tileset_manager.js';
import { editorRoutes } from './routes/editor.js';

dotenv.config();

const fastify = Fastify({ logger: false });
const PORT = process.env.PORT || 3000;

const viewPaths = [
    path.join(process.cwd(), 'client'),
    path.join(process.cwd(), 'client/plugins'),
];

nunjucks.configure(viewPaths, {
    autoescape: true,
    watch: process.env.NODE_ENV === 'development',
    noCache: process.env.NODE_ENV === 'development',
});

await fastify.register(fastifyCompress, {
    global: true,
    encodings: ['gzip', 'deflate'],
    threshold: 1024
});

await fastify.register(fastifyView, {
    engine: { nunjucks },
    root: viewPaths,
    viewExt: 'njk',
});

await fastify.register(fastifyCors, {
    origin: process.env.ALLOWED_ORIGINS?.split(',') || ['http://localhost', 'http://localhost:80'],
    credentials: true,
});

await fastify.register(fastifyCookie);
await fastify.register(fastifyFormbody);

fastify.addHook('onRequest', authMiddleware);

fastify.register(authRoutes, { prefix: '/api/auth' });
fastify.register(ajaxRoutes, { prefix: '/api/ajax' });
fastify.register(scenesRoutes, { prefix: '/api/scenes' });
fastify.register(serversRoutes, { prefix: '/api/servers' });
fastify.register(tilesetManagerRoutes, { prefix: '/api/tileset_manager' });
fastify.register(editorRoutes, { prefix: '/api/editor' });

async function startServer() {
    try {
        await connectDB();
        const address = await fastify.listen({ port: PORT, host: '0.0.0.0' });
        console.log(`Server running at ${address}`);
    } catch (error) {
        console.error('Failed to start server:', error);
        process.exit(1);
    }
}

startServer();
initializeWebSocket(fastify.server);

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