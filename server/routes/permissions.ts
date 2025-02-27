// routes/permissions.ts
import { FastifyInstance, FastifyReply, FastifyRequest } from 'fastify';
import { Permission } from '../models/Permissions.js';
import { User } from '../models/User.js';

export async function permissionRoutes(fastify: FastifyInstance) {
    // Get all permissions
    fastify.get('/list', async (request: FastifyRequest, reply: FastifyReply) => {
        if (!request.auth || !request.user?.permissions.includes('manage_permissions')) {
            return reply.code(403).send({ message: 'Forbidden' });
        }

        try {
            const permissions = await Permission.find({});
            return reply.send(permissions);
        } catch (error) {
            console.error('Error listing permissions:', error);
            return reply.code(500).send({ message: 'Internal server error' });
        }
    });

    // Add new permission
    fastify.post('/add', async (request: FastifyRequest, reply: FastifyReply) => {
        if (!request.auth || !request.user?.permissions.includes('manage_permissions')) {
            return reply.code(403).send({ message: 'Forbidden' });
        }

        const { key, description } = request.body as { key: string; description: string };
        
        if (!key || !description) {
            return reply.code(400).send({ message: 'Missing required fields' });
        }

        try {
            const existingPermission = await Permission.findOne({ key });
            if (existingPermission) {
                return reply.code(400).send({ message: 'Permission already exists' });
            }

            const permission = new Permission({ key, description });
            await permission.save();

            return reply.send({ message: 'Permission added successfully' });
        } catch (error) {
            console.error('Error adding permission:', error);
            return reply.code(500).send({ message: 'Internal server error' });
        }
    });

    // Delete permission
    fastify.delete('/:key', async (request: FastifyRequest, reply: FastifyReply) => {
        if (!request.auth || !request.user?.permissions.includes('manage_permissions')) {
            return reply.code(403).send({ message: 'Forbidden' });
        }

        const { key } = request.params as { key: string };

        try {
            const permission = await Permission.findOne({ key });
            if (!permission) {
                return reply.code(404).send({ message: 'Permission not found' });
            }

            // Remove permission from all users who have it
            await User.updateMany(
                { permissions: key },
                { $pull: { permissions: key } }
            );

            // Delete the permission itself
            await Permission.deleteOne({ key });

            return reply.send({ message: 'Permission deleted successfully' });
        } catch (error) {
            console.error('Error deleting permission:', error);
            return reply.code(500).send({ message: 'Internal server error' });
        }
    });
}