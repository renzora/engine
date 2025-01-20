import { FastifyInstance, FastifyReply, FastifyRequest } from 'fastify';
import { Scene, IScene } from '../models/Scenes.js';

export async function scenesRoutes(fastify: FastifyInstance) {
  fastify.get('/:sceneId', async (request: FastifyRequest, reply: FastifyReply) => {
    const { sceneId } = request.params as { sceneId: string };
    try {
      const scene = await Scene.findById(sceneId);
      if (!scene) {
        return reply.code(404).send({ message: 'Scene not found' });
      }
      return reply.code(200).send({ message: 'success', ...scene.toObject() });
    } catch (err: any) {
      fastify.log.error(`Error fetching scene with ID ${sceneId}:`, err);
      return reply.code(500).send({ message: 'server_error' });
    }
  });

  fastify.get('/scenes', async (request: FastifyRequest, reply: FastifyReply) => {
    try {
      const scenes = await Scene.find();
      return reply.code(200).send(scenes);
    } catch (err: any) {
      fastify.log.error('Error fetching scenes:', err);
      return reply.code(500).send({ message: 'server_error' });
    }
  });

  fastify.post('/create_scene', async (request: FastifyRequest, reply: FastifyReply) => {
    try {
      if (!request.auth || !request.user) {
        return reply.code(401).send({ message: 'Unauthorized', error: true });
      }

      const { serverId, name = 'new scene' } = request.body as {
        serverId: string;
        name?: string;
      };

      if (!serverId) {
        return reply.code(400).send({ message: 'Missing serverId', error: true });
      }

      const sceneCount = await Scene.countDocuments({ server_id: serverId });

      const newScene = await Scene.create({
        server_id: serverId,
        name,
        created_by: request.user._id,
        created_at: Date.now(),
        order: sceneCount,
      });

      return reply.code(200).send({
        message: 'success',
        scene: {
          id: newScene._id,
          name: newScene.name,
          order: newScene.order,
          server_id: newScene.server_id,
        },
      });
    } catch (err: any) {
      fastify.log.error('Error creating scene:', err);
      return reply.code(500).send({ message: 'server_error', error: err });
    }
  });

  fastify.post('/edit_scene', async (request: FastifyRequest, reply: FastifyReply) => {
    try {
      if (!request.auth || !request.user) {
        return reply.code(401).send({ message: 'Unauthorized', error: true });
      }

      const { sceneId, name } = request.body as { sceneId: string; name: string };
      if (!sceneId || !name) {
        return reply
          .code(400)
          .send({ message: 'Invalid sceneId or name.', error: true });
      }

      const scene = await Scene.findById(sceneId);
      if (!scene) {
        return reply.code(404).send({ message: 'Scene not found' });
      }

      scene.name = name;
      await scene.save();

      return reply.code(200).send({ message: 'success' });
    } catch (err: any) {
      fastify.log.error('Error editing scene:', err);
      return reply.code(500).send({ message: 'server_error', error: err });
    }
  });

  fastify.post('/scenes', async (request: FastifyRequest, reply: FastifyReply) => {
    try {
      const { serverId } = request.body as { serverId: string };
      if (!serverId) {
        return reply.code(400).send({ message: 'Missing serverId', error: true });
      }

      const scenes = await Scene.find({ server_id: serverId }).sort({ order: 1 });
      return reply.code(200).send({ message: 'success', scenes });
    } catch (err: any) {
      fastify.log.error('Error fetching scenes:', err);
      return reply.code(500).send({ message: 'server_error', error: err });
    }
  });

  fastify.post('/reorder_scenes', async (request: FastifyRequest, reply: FastifyReply) => {
    try {
      if (!request.auth || !request.user) {
        return reply.code(401).send({ message: 'Unauthorized', error: true });
      }

      const { serverId, orderedSceneIds } = request.body as {
        serverId: string;
        orderedSceneIds: string[];
      };
      if (!serverId || !Array.isArray(orderedSceneIds)) {
        return reply.code(400).send({ message: 'Invalid input', error: true });
      }

      const updates = orderedSceneIds.map((sceneId, index) => {
        return Scene.findOneAndUpdate(
          { _id: sceneId, server_id: serverId },
          { $set: { order: index } },
        );
      });

      await Promise.all(updates);

      return reply.code(200).send({ message: 'success' });
    } catch (err: any) {
      fastify.log.error('Error reordering scenes:', err);
      return reply.code(500).send({ message: 'server_error', error: err });
    }
  });
}
