const Server = require('../models/Servers');
const Scene = require('../models/Scenes');

async function routes(fastify, opts) {
    // List servers
    fastify.post('/list', async (request, reply) => {
        try {
            if (!request.user) {
                return reply.status(401).send({
                    message: 'Unauthorized',
                    error: true
                });
            }

            const userId = request.user.id;
            const { tabType = 'public' } = request.body;

            let filter = {};
            switch (tabType) {
                case 'public':
                    filter = { public: true };
                    break;
                case 'private':
                    filter = { public: false };
                    break;
                case 'events':
                    filter = { events: true };
                    break;
                case 'me':
                    filter = { created_by: userId };
                    break;
                default:
                    throw new Error('Invalid tab type specified');
            }

            const servers = await Server.find(filter)
                .sort({ created_at: -1 })
                .exec();

            const serverList = servers.map(server => ({
                id: server._id.toString(),
                name: server.name,
                created_at: server.created_at,
                public: server.public
            }));

            return {
                message: 'success',
                servers: serverList
            };

        } catch (error) {
            console.error('Error fetching servers:', error);
            return reply.status(500).send({
                message: 'Error fetching servers',
                error: error.message
            });
        }
    });

    // Create server
    fastify.post('/create', async (request, reply) => {
        try {
            if (!request.user) {
                return reply.status(401).send({
                    message: 'Unauthorized',
                    error: true
                });
            }

            const { name = 'default server' } = request.body;
            const playerId = request.user.id;

            const newServer = new Server({
                name,
                created_by: playerId,
                created_at: Date.now(),
                public: true
            });

            const savedServer = await newServer.save();

            return {
                message: 'success',
                server: {
                    id: savedServer._id.toString(),
                    name: savedServer.name,
                    created_by: savedServer.created_by,
                    created_at: savedServer.created_at,
                    public: savedServer.public
                }
            };
        } catch (error) {
            console.error('Error creating server:', error);
            return reply.status(500).send({
                message: 'Error creating server',
                error: error.message
            });
        }
    });

    // Delete server
    fastify.post('/delete', async (request, reply) => {
        try {
            if (!request.user) {
                return reply.status(401).send({
                    message: 'Unauthorized',
                    error: true
                });
            }

            const { id: serverId } = request.body;
            const userId = request.user.id;

            if (!serverId) {
                return reply.status(400).send({ message: 'Invalid input' });
            }

            const server = await Server.findById(serverId);

            if (!server || server.created_by.toString() !== userId.toString()) {
                return reply.status(401).send({ message: 'Unauthorized' });
            }

            // Delete related scenes
            await Scene.deleteMany({ server_id: serverId });

            // Delete the server
            const deleteResult = await Server.deleteOne({ _id: serverId });

            if (deleteResult.deletedCount > 0) {
                return { message: 'success' };
            } else {
                return reply.status(500).send({ message: 'Error deleting server' });
            }
        } catch (error) {
            console.error('Error deleting server:', error);
            return reply.status(500).send({
                message: 'Error deleting server',
                error: error.message
            });
        }
    });

    // Update server
    fastify.post('/update', async (request, reply) => {
        try {
            if (!request.user) {
                return reply.status(401).send({
                    message: 'Unauthorized',
                    error: true
                });
            }

            const { id: serverId, name } = request.body;
            const userId = request.user.id;

            if (!serverId || !name) {
                return reply.status(400).send({
                    message: 'Invalid input',
                    error: 'Server ID or name is missing.'
                });
            }

            const server = await Server.findById(serverId);

            if (!server || server.created_by.toString() !== userId.toString()) {
                return reply.status(401).send({
                    message: 'Unauthorized',
                    error: 'You do not have permission to update this server.'
                });
            }

            const updateResult = await Server.updateOne(
                { _id: serverId },
                { $set: { name: name }}
            );

            if (updateResult.modifiedCount > 0) {
                return { message: 'success' };
            } else {
                return {
                    message: 'success',
                    error: 'No documents were modified.'
                };
            }
        } catch (error) {
            console.error('Error updating server:', error);
            return reply.status(500).send({
                message: 'Error updating server',
                error: error.message
            });
        }
    });
}

module.exports = routes;