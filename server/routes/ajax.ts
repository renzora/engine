import path from 'path';
import fs from 'fs';
import nunjucks from 'nunjucks';
import { FastifyInstance, FastifyReply, FastifyRequest } from 'fastify';
import { redis } from '../redis.js';

export async function ajaxRoutes(fastify: FastifyInstance) {
  const clientRootDir = path.join(process.cwd(), 'client');

  fastify.get('/*', async (request: FastifyRequest, reply: FastifyReply) => {
    const filePath = (request.params as Record<string, string>)['*'];
    const ext = path.extname(filePath).toLowerCase();
    const resolvedPath = path.join(clientRootDir, filePath);

    if (!fs.existsSync(resolvedPath)) {
      const msg = `File not found: ${resolvedPath}`;
      fastify.log.error(msg);
      return reply.code(404).send({ message: msg });
    }

    if (ext === '.njk') {
      try {
        let templateContent = await redis.get(`njk:${filePath}`);
        if (templateContent) {
          console.log(`✅ [REDIS] Fetched template "${filePath}" from cache`);
        } else {
          templateContent = fs.readFileSync(resolvedPath, 'utf8');
          //await redis.set(`njk:${filePath}`, templateContent);
          console.log(`🔵 [DISK] Loaded template "${filePath}" from disk, stored in Redis cache`);
        }

        const rendered = nunjucks.renderString(templateContent, {
          auth: (request as any).auth,
        });
        return reply.type('text/html').send(rendered);
      } catch (err: any) {
        fastify.log.error('Template rendering failed:', err.message);
        return reply.code(500).send({
          message: 'Template rendering failed',
          error: err.message,
        });
      }
    }

    const fileContents = fs.readFileSync(resolvedPath, 'utf8');
    if (ext === '.html') {
      return reply.type('text/html').send(fileContents);
    }
    if (ext === '.js') {
      return reply.type('application/javascript').send(fileContents);
    }

    fastify.log.warn(`Unsupported file type: ${ext}`);
    return reply.code(415).send({ message: 'Unsupported file type' });
  });
}
