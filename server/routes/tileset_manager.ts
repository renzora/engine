import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import uniqid from 'uniqid';
import { promisify } from 'util';
import { FastifyInstance, FastifyReply, FastifyRequest } from 'fastify';
import { PNG } from 'pngjs';

const readdir = promisify(fs.readdir);
const readFile = promisify(fs.readFile);
const writeFile = promisify(fs.writeFile);

async function fileExists(filePath: string) {
  try {
    await fs.promises.access(filePath, fs.constants.F_OK);
    return true;
  } catch {
    return false;
  }
}

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const clientRoot = path.join(__dirname, '..', '..', 'client');

function getSheetsDir() {
  return path.join(clientRoot, 'assets', 'img', 'sheets');
}

function getObjectDataFile() {
  return path.join(clientRoot, 'assets', 'json', 'objectData.json');
}

function getMaxTileIndex(objectData: Record<string, any>, tilesetName: string): number {
  let maxIndex = -1;
  for (const uid in objectData) {
    const arr = objectData[uid];
    if (!Array.isArray(arr)) continue;
    arr.forEach((item: any) => {
      if (item.t === tilesetName && Array.isArray(item.i)) {
        item.i.forEach((str: string) => {
          if (str.includes('-')) {
            const [_, endStr] = str.split('-');
            const endNum = parseInt(endStr, 10);
            if (endNum > maxIndex) maxIndex = endNum;
          } else {
            const val = parseInt(str, 10);
            if (val > maxIndex) maxIndex = val;
          }
        });
      }
    });
  }
  return maxIndex;
}

function expandPngHeight(oldPng: PNG, newRows: number): PNG {
  const tileSize = 16;
  const newHeight = newRows * tileSize;
  const newPng = new PNG({ width: oldPng.width, height: newHeight });
  newPng.data.fill(0);

  for (let y = 0; y < oldPng.height; y++) {
    for (let x = 0; x < oldPng.width; x++) {
      const oldIdx = (y * oldPng.width + x) << 2;
      const newIdx = (y * newPng.width + x) << 2;
      newPng.data[newIdx + 0] = oldPng.data[oldIdx + 0];
      newPng.data[newIdx + 1] = oldPng.data[oldIdx + 1];
      newPng.data[newIdx + 2] = oldPng.data[oldIdx + 2];
      newPng.data[newIdx + 3] = oldPng.data[oldIdx + 3];
    }
  }
  return newPng;
}

function copyTile(
  srcPng: PNG,
  dstPng: PNG,
  sx: number,
  sy: number,
  dx: number,
  dy: number,
  tileSize: number
) {
  for (let row = 0; row < tileSize; row++) {
    for (let col = 0; col < tileSize; col++) {
      const sX = sx + col;
      const sY = sy + row;
      const dX = dx + col;
      const dY = dy + row;

      if (sX >= srcPng.width || sY >= srcPng.height) continue;
      if (dX >= dstPng.width || dY >= dstPng.height) continue;

      const srcIdx = (sY * srcPng.width + sX) << 2;
      const dstIdx = (dY * dstPng.width + dX) << 2;
      dstPng.data[dstIdx + 0] = srcPng.data[srcIdx + 0];
      dstPng.data[dstIdx + 1] = srcPng.data[srcIdx + 1];
      dstPng.data[dstIdx + 2] = srcPng.data[srcIdx + 2];
      dstPng.data[dstIdx + 3] = srcPng.data[srcIdx + 3];
    }
  }
}

export async function tilesetManagerRoutes(fastify: FastifyInstance) {
  fastify.get('/list_sheets', async (req: FastifyRequest, reply: FastifyReply) => {
    try {
      const sheetsDir = getSheetsDir();
      const files = await readdir(sheetsDir);
      const pngs = files
        .filter((f) => f.toLowerCase().endsWith('.png'))
        .map((f) => f.replace(/\.png$/i, ''));
      return reply.send({ success: true, sheets: pngs });
    } catch (err: any) {
      fastify.log.error('Error listing sheets:', err);
      return reply.code(500).send({ success: false, message: err.message });
    }
  });

  fastify.post('/save', async (req: FastifyRequest, reply: FastifyReply) => {
    try {
      const { groupedData } = req.body as { groupedData: any[] };
      if (!Array.isArray(groupedData) || !groupedData.length) {
        return reply.code(400).send({
          success: false,
          message: '`groupedData` must be a non-empty array.',
        });
      }

      const objectDataFile = getObjectDataFile();
      if (!(await fileExists(objectDataFile))) {
        throw new Error('objectData.json not found');
      }

      const objectDataRaw = await readFile(objectDataFile, 'utf8');
      const objectData = JSON.parse(objectDataRaw);

      for (const group of groupedData) {
        const tilesetName = (group.tileset || 'gen1').trim();
        const items = group.items || [];
        if (!items.length) continue;

        const tileSize = 16;
        const tilesPerRow = 150;
        const sheetPath = path.join(getSheetsDir(), `${tilesetName}.png`); 
        let basePng: PNG;
        let currentWidth: number;
        let currentHeight: number;

        if (await fileExists(sheetPath)) {
          const buf = await readFile(sheetPath);
          basePng = PNG.sync.read(buf);
          currentWidth = basePng.width;
          currentHeight = basePng.height;
        } else {
          currentWidth = tilesPerRow * tileSize;
          currentHeight = tileSize;
          basePng = new PNG({ width: currentWidth, height: currentHeight });
          basePng.data.fill(0);
        }

        const maxIdx = getMaxTileIndex(objectData, tilesetName);
        let runningTileIndex = maxIdx < 0 ? 0 : maxIdx + 1;

        for (const item of items) {
          const { newObject, imageData, aCoords, bCoords } = item;
          if (!newObject || !imageData || !aCoords?.length || !bCoords?.length) {
            continue;
          }

          const base64Regex = /^data:image\/\w+;base64,/;
          const raw = imageData.replace(base64Regex, '');
          const imgBuf = Buffer.from(raw, 'base64');
          const croppedPng = PNG.sync.read(imgBuf);

          const tileCount = aCoords.length;
          const startIndex = runningTileIndex;
          const endIndex = startIndex + tileCount - 1;

          for (let i = 0; i < tileCount; i++) {
            const sx = aCoords[i] * tileSize;
            const sy = bCoords[i] * tileSize;
            const row = Math.floor(runningTileIndex / tilesPerRow);
            const col = runningTileIndex % tilesPerRow;

            const requiredHeight = (row + 1) * tileSize;
            if (requiredHeight > basePng.height) {
              basePng = expandPngHeight(basePng, row + 1);
              currentHeight = basePng.height;
            }

            const dx = col * tileSize;
            const dy = row * tileSize;
            copyTile(croppedPng, basePng, sx, sy, dx, dy, tileSize);

            runningTileIndex++;
          }

          if (tileCount === 1) {
            newObject.i = [`${startIndex}`];
          } else {
            newObject.i = [`${startIndex}-${endIndex}`];
          }

          const minA = Math.min(...aCoords);
          const maxA = Math.max(...aCoords);
          const minB = Math.min(...bCoords);
          const maxB = Math.max(...bCoords);
          newObject.a = maxA - minA;
          newObject.b = maxB - minB;
          newObject.z = 0;

          const uid = uniqid();
          objectData[uid] = [newObject];
        }

        const newPngBuf = PNG.sync.write(basePng);
        await writeFile(sheetPath, newPngBuf);
      }

      await writeFile(objectDataFile, JSON.stringify(objectData), 'utf8');

      return reply.send({ success: true });
    } catch (err: any) {
      fastify.log.error('Error /api/tileset_manager/save:', err);
      return reply.code(500).send({ success: false, message: err.message });
    }
  });
}
