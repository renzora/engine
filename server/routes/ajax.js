import path from 'path';
import fs from 'fs';

const clientRootDir = path.join(process.cwd(), 'client');

export const ajaxRoutes = async (fastify, opts) => {
    fastify.get('/*', async (request, reply) => {
        const filePath = request.params['*'];
        const ext = path.extname(filePath).toLowerCase();
        const resolvedPath = path.join(clientRootDir, filePath);

        if (!fs.existsSync(resolvedPath)) {
            console.error(`File not found: ${resolvedPath}`);
            reply.code(404).send({ message: `File not found: ${resolvedPath}` });
            return;
        }

        if (ext === '.njk') {
            try {
                return reply.view(filePath, {
                    auth: request.auth,
                });
            } catch (err) {
                console.error('Template rendering failed:', err.message);
                reply.code(500).send({ message: 'Template rendering failed', error: err.message });
            }
            return;
        }

        const fileContents = fs.readFileSync(resolvedPath, 'utf8');

        if (ext === '.html') {
            reply.type('text/html').send(fileContents);
            return;
        }

        if (ext === '.js') {
            reply.type('application/javascript').send(fileContents);
            return;
        }

        console.warn(`Unsupported file type: ${ext}`);
        reply.code(415).send({ message: 'Unsupported file type' });
    });
};
