import { FastifyInstance, FastifyReply, FastifyRequest } from 'fastify';
import { IScene, Scene } from '../models/Scenes';

export async function editorRoutes(fastify: FastifyInstance) {

  fastify.get('/scene/:id', async (request: FastifyRequest, reply: FastifyReply) => {
    try {
      const { id } = request.params as { id: string };
      const scene: IScene | null = await Scene.findById(id);
      if (!scene) {
        return reply.status(404).send({ message: 'Scene not found', error: true });
      }
      return reply.send(scene);
    } catch (error: any) {
      return reply.status(500).send({
        message: 'Error fetching scene',
        error: error.message,
      });
    }
  });

  fastify.post('/scene/save', async (request: FastifyRequest, reply: FastifyReply) => {
    try {
      if (!request.user) {
        return reply.status(401).send({ message: 'Unauthorized', error: true });
      }

      const { sceneid, roomData, editorLayers } = request.body as {
        sceneid: string;
        roomData: object;
        editorLayers: any;
      };

      if (!sceneid || !roomData) {
        return reply
          .status(400)
          .send({ message: 'sceneid or roomData not provided', error: true });
      }

      const updatedScene: IScene | null = await Scene.findOneAndUpdate(
        { _id: sceneid },
        { 
          $set: { 
            roomData: roomData,
            ...(editorLayers !== undefined && { editorLayers: editorLayers }),
          } 
        },
        { new: true },
      );

      if (!updatedScene) {
        return reply.status(404).send({ message: 'Scene not found', error: true });
      }

      return reply.send({
        message: 'Room data (and layers) saved successfully',
        sceneId: updatedScene._id,
        editorLayers: updatedScene.editorLayers,
      });
    } catch (error: any) {
      return reply.status(500).send({
        message: 'Error saving room data',
        error: error.message,
      });
    }
  });

  fastify.post('/scene/dimensions', async (request: FastifyRequest, reply: FastifyReply) => {
    try {
      const { sceneId, width, height } = request.body as {
        sceneId: string;
        width: number;
        height: number;
      };

      if (!sceneId || width === undefined || height === undefined) {
        return reply.status(400).send({ message: 'Invalid input data', error: true });
      }

      const updatedScene: IScene | null = await Scene.findOneAndUpdate(
        { _id: sceneId },
        {
          $set: {
            width: parseInt(width.toString(), 10),
            height: parseInt(height.toString(), 10),
          },
        },
        { new: true },
      );

      if (!updatedScene) {
        return reply.status(404).send({ message: 'Scene not found', error: true });
      }

      return reply.send({
        message: 'Scene dimensions updated successfully.',
        width: updatedScene.width,
        height: updatedScene.height,
        error: false,
      });
    } catch (error: any) {
      return reply.status(500).send({
        message: 'Error updating scene dimensions',
        error: error.message,
      });
    }
  });

  fastify.post('/scene/position', async (request: FastifyRequest, reply: FastifyReply) => {
    try {
      let { sceneId, startingX, startingY } = request.body as {
        sceneId: string;
        startingX: number;
        startingY: number;
      };

      if (!sceneId || startingX === undefined || startingY === undefined) {
        return reply.status(400).send({ message: 'Invalid input data', error: true });
      }

      startingX = Math.round((startingX * 16) / 16) * 16;
      startingY = Math.round((startingY * 16) / 16) * 16;

      const updatedScene: IScene | null = await Scene.findOneAndUpdate(
        { _id: sceneId },
        { $set: { startingX, startingY } },
        { new: true },
      );

      if (!updatedScene) {
        return reply.status(404).send({ message: 'Scene not found', error: true });
      }

      return reply.send({
        message: 'Starting position updated successfully.',
        startingX: updatedScene.startingX,
        startingY: updatedScene.startingY,
        error: false,
      });
    } catch (error: any) {
      return reply.status(500).send({
        message: 'Error updating scene starting position',
        error: error.message,
      });
    }
  });

}
