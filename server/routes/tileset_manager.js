import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { createCanvas, loadImage } from 'canvas';
import uniqid from 'uniqid';
import { promisify } from 'util';

const readFile = promisify(fs.readFile);
const writeFile = promisify(fs.writeFile);
const exists = promisify(fs.exists);

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export const tilesetManagerRoutes = async (fastify, opts) => {
  fastify.post('/save', async (request, reply) => {
    try {

      const input = request.body;
      if (!input) {
        return reply
          .code(400)
          .send({ success: false, message: 'Invalid JSON in request body' });
      }

      const newObject = input.newObject;
      const aCoords = newObject.a;
      const bCoords = newObject.b;
      const imageDataBase64 = input.imageData;

      const base64Regex = /^data:image\/\w+;base64,/;
      const imageBuffer = Buffer.from(
        imageDataBase64.replace(base64Regex, ''),
        'base64'
      );

      const uploadedImage = await loadImage(imageBuffer);
      const objectDataFile = path.join(__dirname, '..', '..', 'assets', 'json', 'objectData.json');
      const metaDataFile = path.join(__dirname, '..', '..', 'assets', 'json', 'meta.json');
      const tilesetImagePath = path.join(__dirname, '..', '..', 'assets', 'img', 'sheets', 'gen1.png');
      const objectDataFileExists = await exists(objectDataFile);

      if (!objectDataFileExists) {
        return reply.code(400).send({ success: false, message: 'Object data file not found.' });
      }

      const metaDataFileExists = await exists(metaDataFile);
      if (!metaDataFileExists) {
        return reply.code(400).send({ success: false, message: 'Meta data file not found.' });
      }

      const tilesetExists = await exists(tilesetImagePath);
      if (!tilesetExists) {
        return reply.code(400).send({ success: false, message: 'Tileset image file not found.' });
      }

      const objectDataRaw = await readFile(objectDataFile, 'utf8');
      const objectData = JSON.parse(objectDataRaw);
      const metaDataRaw = await readFile(metaDataFile, 'utf8');
      const metaData = JSON.parse(metaDataRaw);
      const uniqueId = uniqid();
      const initialTileIndex = metaData.tile_count || 0;
      const startIndex = initialTileIndex;
      const endIndex = initialTileIndex + aCoords.length - 1;
      newObject.i = [`${startIndex}-${endIndex}`];
      const uniqueXValues = Array.from(new Set(aCoords));
      const uniqueYValues = Array.from(new Set(bCoords));
      const columnCount = uniqueXValues.length;
      const rowCount = uniqueYValues.length;
      newObject.a = columnCount;
      newObject.b = rowCount;
      objectData[uniqueId] = [newObject];
      const objectDataWithAdjustedAB = JSON.parse(JSON.stringify(objectData));
      objectDataWithAdjustedAB[uniqueId][0].a -= 1;
      objectDataWithAdjustedAB[uniqueId][0].b -= 1;
      metaData.tile_count = (metaData.tile_count || 0) + aCoords.length;

      await writeFile(objectDataFile, JSON.stringify(objectDataWithAdjustedAB, null, 2), 'utf8');
      await writeFile(metaDataFile, JSON.stringify(metaData, null, 2), 'utf8');

      const tilesetBuffer = await readFile(tilesetImagePath);
      const tilesetImage = await loadImage(tilesetBuffer);
      const tileSize = 16;
      const tilesPerRow = 150;
      const currentWidth = tilesetImage.width;
      const currentHeight = tilesetImage.height;
      const currentTileCount = (currentHeight / tileSize) * tilesPerRow;
      const newTileCount = initialTileIndex + aCoords.length;
      const requiredRows = Math.ceil(newTileCount / tilesPerRow);
      const requiredHeight = requiredRows * tileSize;
      let finalCanvasHeight = Math.max(requiredHeight, currentHeight);
      const finalCanvas = createCanvas(currentWidth, finalCanvasHeight);
      const ctx = finalCanvas.getContext('2d');
      ctx.clearRect(0, 0, currentWidth, finalCanvasHeight);
      ctx.drawImage(tilesetImage, 0, 0);

      for (let i = 0; i < aCoords.length; i++) {
        const a = aCoords[i];
        const b = bCoords[i];
        const srcX = a * tileSize;
        const srcY = b * tileSize;
        const destIndex = initialTileIndex + i;
        const destX = (destIndex % tilesPerRow) * tileSize;
        const destY = Math.floor(destIndex / tilesPerRow) * tileSize;

        ctx.drawImage(
          uploadedImage,
          srcX,
          srcY,
          tileSize,
          tileSize,
          destX,
          destY,
          tileSize,
          tileSize
        );
      }

      const updatedTilesetBuffer = finalCanvas.toBuffer('image/png');
      await writeFile(tilesetImagePath, updatedTilesetBuffer);

      return reply.send({ success: true });
    } catch (err) {
      fastify.log.error('Error in /api/tileset_manager/save:', err);
      return reply.code(500).send({ success: false, message: err.message });
    }
  });

  fastify.post('/save_item', async (request, reply) => {
    try {

      const data = request.body;
      if (!data) {
        return reply
          .code(400)
          .send({ success: false, message: 'Invalid JSON in request body' });
      }

      const objectDataPath = path.join(__dirname, '..', '..', 'assets', 'json', 'objectData.json');

      const objectDataFileExists = await exists(objectDataPath);
      if (!objectDataFileExists) {
        return reply.code(400).send({
          success: false,
          message: 'File does not exist',
          path: objectDataPath
        });
      }

      const writeResult = await writeFile(
        objectDataPath,
        JSON.stringify(data, null, 2),
        'utf8'
      );

      return reply.send({
        success: true,
        message: 'Object data saved successfully',
        received_data: data
      });
    } catch (err) {
      fastify.log.error('Error in /api/tileset_manager/save_item:', err);
      return reply.code(500).send({
        success: false,
        message: 'Failed to save object data',
        error: err.message
      });
    }
  });
};
