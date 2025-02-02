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
        item.i.forEach((entry: any) => {
          const strVal = String(entry);
          if (strVal.includes('-')) {
            const [_, endStr] = strVal.split('-');
            const endNum = parseInt(endStr, 10);
            if (endNum > maxIndex) maxIndex = endNum;
          } else {
            const val = parseInt(strVal, 10);
            if (val > maxIndex) maxIndex = val;
          }
        });
      }
    });
  }
  return maxIndex;
}

function parseFrameRanges(iValues: (string|number)[]): number[] {
  const frames: number[] = [];
  iValues.forEach(value => {
    if (typeof value === 'string' && value.includes('-')) {
      const [start, end] = value.split('-').map(x => parseInt(x,10));
      for (let f=start; f<=end; f++){
        frames.push(f);
      }
    } else {
      frames.push(parseInt(String(value),10));
    }
  });
  return frames.sort((a,b)=>a-b);
}

function collapseFramesToRanges(frames: number[]): string[] {
  if (!frames.length) return [];
  frames.sort((a,b)=>a-b);
  const ranges: string[] = [];
  let start = frames[0];
  let current = start;
  for (let i=1; i<frames.length; i++){
    const val = frames[i];
    if (val === current+1) {
      current = val;
    } else {
      if (start === current) {
        ranges.push(`${start}`);
      } else {
        ranges.push(`${start}-${current}`);
      }
      start = val;
      current = val;
    }
  }
  // last range
  if (start === current) {
    ranges.push(`${start}`);
  } else {
    ranges.push(`${start}-${current}`);
  }
  return ranges;
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

function copyTilesAcrossSprites(
  oldTilesetName: string, 
  newTilesetName: string,
  itemFrames: number[],
  oldPng: PNG,
  newPng: PNG,
  runningIndex: number,
  tileSize: number = 16,
  tilesPerRow: number = 150
): number {
  for (let i = 0; i < itemFrames.length; i++) {
    const oldFrame = itemFrames[i];
    const oldRow = Math.floor(oldFrame / tilesPerRow);
    const oldCol = oldFrame % tilesPerRow;
    const sx = oldCol * tileSize;
    const sy = oldRow * tileSize;
    const newRow = Math.floor(runningIndex / tilesPerRow);
    const newCol = runningIndex % tilesPerRow;
    const dx = newCol * tileSize;
    const dy = newRow * tileSize;
    copyTile(oldPng, newPng, sx, sy, dx, dy, tileSize);
    runningIndex++;
  }
  return runningIndex;
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

  fastify.post('/move', async (req: FastifyRequest, reply: FastifyReply) => {
    try {
      const { itemIds, newTileset } = req.body as {
        itemIds: string[];
        newTileset: string;
      };
      if (!Array.isArray(itemIds) || !itemIds.length || !newTileset) {
        return reply.code(400).send({
          success: false,
          message: 'Invalid request. itemIds[] and newTileset required.'
        });
      }
  
      const objectDataFile = getObjectDataFile();
      if (!(await fileExists(objectDataFile))) {
        throw new Error('objectData.json not found');
      }
      const objectDataRaw = await readFile(objectDataFile, 'utf8');
      const objectData = JSON.parse(objectDataRaw);
  
      const itemsToMoveByTileset: Record<string, {uid: string, frames: number[], item: any}[]> = {};
      const toMoveSet = new Set(itemIds);
  
      for (const [uid, arr] of Object.entries(objectData)) {
        if (!Array.isArray(arr) || !arr.length) continue;
        const item = arr[0];
        if (toMoveSet.has(uid)) {
          if (!itemsToMoveByTileset[item.t]) {
            itemsToMoveByTileset[item.t] = [];
          }
          const frames = parseFrameRanges(item.i);
          itemsToMoveByTileset[item.t].push({ uid, frames, item });
        }
      }
  
      const sheetsDir = getSheetsDir();
      const tileSize = 16;
      const tilesPerRow = 150;
  
      for (const oldTilesetName of Object.keys(itemsToMoveByTileset)) {
        const oldSheetPath = path.join(sheetsDir, `${oldTilesetName}.png`);
        const oldBuf = await readFile(oldSheetPath);
        const oldPng = PNG.sync.read(oldBuf);
  
        const newSheetPath = path.join(sheetsDir, `${newTileset}.png`);
        let newPng: PNG;
        let currentWidth: number;
        let currentHeight: number;
  
        if (await fileExists(newSheetPath)) {
          const newBuf = await readFile(newSheetPath);
          newPng = PNG.sync.read(newBuf);
          currentWidth = newPng.width;
          currentHeight = newPng.height;
        } else {
          currentWidth = tilesPerRow * tileSize;
          currentHeight = tileSize;
          newPng = new PNG({ width: currentWidth, height: currentHeight });
          newPng.data.fill(0);
        }
  
        const maxIdx = getMaxTileIndex(objectData, newTileset);
        let runningIndex = maxIdx < 0 ? 0 : maxIdx + 1;
  
        for (const record of itemsToMoveByTileset[oldTilesetName]) {
          const frames = record.frames;
          const framesCount = frames.length;
          const newRequiredRows = Math.ceil((runningIndex + framesCount) / tilesPerRow);
          const requiredHeight = newRequiredRows * tileSize;
          
          if (requiredHeight > newPng.height) {
            newPng = expandPngHeight(newPng, newRequiredRows);
          }
  
          runningIndex = copyTilesAcrossSprites(
            oldTilesetName,
            newTileset,
            frames,
            oldPng,
            newPng,
            runningIndex,
            tileSize,
            tilesPerRow
          );
  
          const startIndex = runningIndex - framesCount;
          record.item.t = newTileset;
          if (framesCount === 1) {
            record.item.i = [`${startIndex}`];
          } else {
            record.item.i = [`${startIndex}-${startIndex + framesCount - 1}`];
          }
        }
  
        const newBuf = PNG.sync.write(newPng);
        await writeFile(newSheetPath, newBuf);
      }
  
      await writeFile(objectDataFile, JSON.stringify(objectData), 'utf8');
      return reply.send({ success: true });
    } catch (err: any) {
      fastify.log.error('Error /api/tileset_manager/move:', err);
      return reply.code(500).send({ success: false, message: err.message });
    }
  });

  fastify.post('/delete', async (req: FastifyRequest, reply: FastifyReply) => {
    try {
      const { itemIds } = req.body as { itemIds: string[] };
      if (!Array.isArray(itemIds) || !itemIds.length) {
        return reply.code(400).send({ success: false, message: 'No itemIds provided' });
      }

      const objectDataFile = getObjectDataFile();
      if (!(await fileExists(objectDataFile))) {
        throw new Error('objectData.json not found');
      }
      const objectDataRaw = await readFile(objectDataFile, 'utf8');
      const objectData = JSON.parse(objectDataRaw);
      const tilesetToItemsMap: Record<string, { uid: string, frames: number[], item: any }[]> = {};
      const toRemoveSet = new Set(itemIds);

      for (const [uid, arr] of Object.entries(objectData)) {
        if (!Array.isArray(arr) || !arr.length) continue;
        const item = arr[0];
        if (toRemoveSet.has(uid)) {
          delete objectData[uid];
          continue;
        }
        if (!tilesetToItemsMap[item.t]) {
          tilesetToItemsMap[item.t] = [];
        }
        const frames = parseFrameRanges(item.i);
        tilesetToItemsMap[item.t].push({ uid, frames, item });
      }

      const tileSize = 16;
      const tilesPerRow = 150;
      const sheetsDir = getSheetsDir();
      const changedTilesets = Object.keys(tilesetToItemsMap);

      for (const tilesetName of changedTilesets) {
        const sheetPath = path.join(sheetsDir, `${tilesetName}.png`);
        if (!(await fileExists(sheetPath))) {
          continue;
        }
        const oldBuf = await readFile(sheetPath);
        const oldPng = PNG.sync.read(oldBuf);

        tilesetToItemsMap[tilesetName].sort((a,b) => {
          return Math.min(...a.frames) - Math.min(...b.frames);
        });

        let runningIndex = 0;
        const newPngRows: number[] = [];
        const newWidth = oldPng.width;
        let newHeight = tileSize;

        const newPng = new PNG({ width: newWidth, height: newHeight });
        newPng.data.fill(0);

        for (const record of tilesetToItemsMap[tilesetName]) {
          const framesOld = record.frames;
          const framesCount = framesOld.length;
          const startIndex = runningIndex;
          const endIndex = startIndex + framesCount - 1;

          for (let i=0; i<framesCount; i++){
            const oldFrame = framesOld[i];
            const oldRow = Math.floor(oldFrame / tilesPerRow);
            const oldCol = oldFrame % tilesPerRow;
            const sx = oldCol * tileSize;
            const sy = oldRow * tileSize;
            const newRow = Math.floor(runningIndex / tilesPerRow);
            const newCol = runningIndex % tilesPerRow;
            const requiredHeight = (newRow + 1) * tileSize;

            if (requiredHeight > newPng.height) {
              const temp = new PNG({ width: newPng.width, height: requiredHeight });
              temp.data.fill(0);
              for (let y=0; y<newPng.height; y++){
                for (let x=0; x<newPng.width; x++){
                  const idxOld = (y * newPng.width + x) << 2;
                  const idxNew = (y * temp.width + x) << 2;
                  temp.data[idxNew+0] = newPng.data[idxOld+0];
                  temp.data[idxNew+1] = newPng.data[idxOld+1];
                  temp.data[idxNew+2] = newPng.data[idxOld+2];
                  temp.data[idxNew+3] = newPng.data[idxOld+3];
                }
              }
              newPng.data = temp.data;
              newPng.height = temp.height;
            }

            const dx = newCol * tileSize;
            const dy = newRow * tileSize;
            copyTile(oldPng, newPng, sx, sy, dx, dy, tileSize);
            runningIndex++;
          }

          const newFramesArray = [];
          for (let f=startIndex; f<=endIndex; f++){
            newFramesArray.push(f);
          }
          const ranges = collapseFramesToRanges(newFramesArray);
          record.item.i = ranges;
        }

        const finalPngHeight = Math.ceil(runningIndex / tilesPerRow) * tileSize;
        if (finalPngHeight < newPng.height) {
          const croppedPng = new PNG({ width: newPng.width, height: finalPngHeight });
          croppedPng.data.fill(0);
          for (let y=0; y<finalPngHeight; y++){
            for (let x=0; x<newPng.width; x++){
              const idxOld = (y * newPng.width + x) << 2;
              const idxNew = (y * croppedPng.width + x) << 2;
              croppedPng.data[idxNew+0] = newPng.data[idxOld+0];
              croppedPng.data[idxNew+1] = newPng.data[idxOld+1];
              croppedPng.data[idxNew+2] = newPng.data[idxOld+2];
              croppedPng.data[idxNew+3] = newPng.data[idxOld+3];
            }
          }
          const newBuf = PNG.sync.write(croppedPng);
          await writeFile(sheetPath, newBuf);
        } else {
          const newBuf = PNG.sync.write(newPng);
          await writeFile(sheetPath, newBuf);
        }
      }

      await writeFile(objectDataFile, JSON.stringify(objectData), 'utf8');

      return reply.send({ success: true });
    } catch (err: any) {
      fastify.log.error('Error /api/tileset_manager/delete:', err);
      return reply.code(500).send({ success: false, message: err.message });
    }
  });
}
