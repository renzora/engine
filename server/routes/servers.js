import { Servers } from '../models/Servers.js';
import { Scene } from '../models/Scenes.js';

export const serversRoutes = async (fastify, opts) => {

  fastify.post('/create_server', async (request, reply) => {
    try {
      if (!request.auth || !request.user) {
        return reply.code(401).send({ message: 'Unauthorized', error: true });
      }

      const { name = 'new server' } = request.body;
      const userId = request.user._id;

      const newServer = new Servers({
        name,
        created_by: userId,
        created_at: new Date(),
        public: true
      });

      await newServer.save();

      return reply.code(200).send({
        message: 'success',
        server: {
          id: newServer._id,
          name: newServer.name,
          created_by: newServer.created_by,
          created_at: newServer.created_at,
          public: newServer.public
        }
      });
    } catch (error) {
      fastify.log.error(error);
      return reply.code(500).send({
        message: 'Error creating server',
        error: error.message
      });
    }
  });

  fastify.get('/get_servers', async (request, reply) => {
    try {
      const servers = await Servers.find().sort({ created_at: -1 });
      const serverList = servers.map((s) => ({
        id: s._id,
        name: s.name,
        created_at: s.created_at,
        public: s.public
      }));

      return reply.code(200).send({
        message: 'success',
        servers: serverList
      });
    } catch (error) {
      fastify.log.error(error);
      return reply.code(500).send({
        message: 'Error fetching servers',
        error: error.message
      });
    }
  });

  fastify.post('/edit_server', async (request, reply) => {
    try {
      if (!request.auth || !request.user) {
        return reply.code(401).send({ message: 'Unauthorized', error: true });
      }

      const { id: serverId, name } = request.body;
      const userId = request.user._id;

      if (!serverId || !name) {
        return reply
          .code(400)
          .send({ message: 'Invalid input', error: true });
      }

      const server = await Servers.findById(serverId);
      if (!server) {
        return reply
          .code(404)
          .send({ message: 'Server not found', error: true });
      }

      if (String(server.created_by) !== String(userId)) {
        return reply
          .code(403)
          .send({ message: 'Unauthorized', error: true });
      }

      server.name = name;
      const savedServer = await server.save();

      return reply.code(200).send({
        message: 'success',
        server: {
          id: savedServer._id,
          name: savedServer.name
        }
      });
    } catch (error) {
      fastify.log.error(error);
      return reply.code(500).send({
        message: 'Error updating server',
        error: error.message
      });
    }
  });

  fastify.post('/delete_server', async (request, reply) => {
    try {
      if (!request.auth || !request.user) {
        return reply.code(401).send({ message: 'Unauthorized', error: true });
      }

      const { id: serverId } = request.body;
      const userId = request.user._id;

      if (!serverId) {
        return reply
          .code(400)
          .send({ message: 'Invalid input', error: true });
      }

      const server = await Servers.findById(serverId);
      if (!server) {
        return reply
          .code(404)
          .send({ message: 'Server not found', error: true });
      }

      if (String(server.created_by) !== String(userId)) {
        return reply
          .code(403)
          .send({ message: 'Unauthorized', error: true });
      }

      await Scene.deleteMany({ server_id: serverId });

      const result = await Servers.deleteOne({ _id: serverId });
      if (result.deletedCount > 0) {
        return reply.code(200).send({ message: 'success' });
      } else {
        return reply.code(500).send({ message: 'Error deleting server', error: true });
      }
    } catch (error) {
      fastify.log.error(error);
      return reply.code(500).send({
        message: 'Error deleting server',
        error: error.message
      });
    }
  });
};
