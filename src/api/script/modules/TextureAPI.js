// === TEXTURE API MODULE ===

import {
  Texture,
  CubeTexture,
  HDRCubeTexture,
  DynamicTexture,
  VideoTexture,
  RawTexture,
  RenderTargetTexture,
  MirrorTexture,
  RefractionTexture,
  NoiseProceduralTexture,
  Color3,
  Matrix
} from '@babylonjs/core';

// Procedural textures
import { WoodProceduralTexture } from '@babylonjs/procedural-textures/wood/woodProceduralTexture.js';
import { CloudProceduralTexture } from '@babylonjs/procedural-textures/cloud/cloudProceduralTexture.js';
import { FireProceduralTexture } from '@babylonjs/procedural-textures/fire/fireProceduralTexture.js';
import { GrassProceduralTexture } from '@babylonjs/procedural-textures/grass/grassProceduralTexture.js';
import { MarbleProceduralTexture } from '@babylonjs/procedural-textures/marble/marbleProceduralTexture.js';
import { BrickProceduralTexture } from '@babylonjs/procedural-textures/brick/brickProceduralTexture.js';
import { RoadProceduralTexture } from '@babylonjs/procedural-textures/road/roadProceduralTexture.js';

export class TextureAPI {
  constructor(scene) {
    this.scene = scene;
  }

  // === BASIC TEXTURE CREATION ===

  createTexture(url, options = {}) {
    return new Texture(url, this.scene, options);
  }

  createCubeTexture(rootUrl, extensions = ['_px.jpg', '_nx.jpg', '_py.jpg', '_ny.jpg', '_pz.jpg', '_nz.jpg']) {
    return new CubeTexture(rootUrl, this.scene, extensions);
  }

  createHDRCubeTexture(url, size = 512, options = {}) {
    return new HDRCubeTexture(url, this.scene, size, options.noMipmap, options.generateHarmonics);
  }

  createDynamicTexture(name, width = 512, height = 512, options = {}) {
    const texture = new DynamicTexture(name, { width, height }, this.scene, options.generateMipMaps);
    return texture;
  }

  createVideoTexture(name, urls, options = {}) {
    return new VideoTexture(name, urls, this.scene, options);
  }

  createRawTexture(data, width, height, format = 5, options = {}) {
    return new RawTexture(data, width, height, format, this.scene, options);
  }

  createRenderTargetTexture(name, size = 512, options = {}) {
    return new RenderTargetTexture(name, size, this.scene, options);
  }

  createMirrorTexture(name, size = 512, options = {}) {
    return new MirrorTexture(name, size, this.scene, options);
  }

  createRefractionTexture(name, size = 512, options = {}) {
    return new RefractionTexture(name, size, this.scene, options);
  }

  // === PROCEDURAL TEXTURES ===

  createWoodTexture(name, size = 512) {
    return new WoodProceduralTexture(name, size, this.scene);
  }

  createCloudTexture(name, size = 512) {
    return new CloudProceduralTexture(name, size, this.scene);
  }

  createFireTexture(name, size = 512) {
    return new FireProceduralTexture(name, size, this.scene);
  }

  createGrassTexture(name, size = 512) {
    return new GrassProceduralTexture(name, size, this.scene);
  }

  createMarbleTexture(name, size = 512) {
    return new MarbleProceduralTexture(name, size, this.scene);
  }

  createBrickTexture(name, size = 512) {
    return new BrickProceduralTexture(name, size, this.scene);
  }

  createRoadTexture(name, size = 512) {
    return new RoadProceduralTexture(name, size, this.scene);
  }


  // === TEXTURE PROPERTIES ===

  setTextureWrapMode(texture, wrapU = 1, wrapV = 1) {
    if (!texture) return false;
    texture.wrapU = wrapU;
    texture.wrapV = wrapV;
    return true;
  }

  setTextureFiltering(texture, magFilter = 1, minFilter = 1) {
    if (!texture) return false;
    texture.magFilter = magFilter;
    texture.minFilter = minFilter;
    return true;
  }

  setTextureOffset(texture, u = 0, v = 0) {
    if (!texture) return false;
    texture.uOffset = u;
    texture.vOffset = v;
    return true;
  }

  setTextureScale(texture, u = 1, v = 1) {
    if (!texture) return false;
    texture.uScale = u;
    texture.vScale = v;
    return true;
  }

  setTextureRotation(texture, angle = 0) {
    if (!texture) return false;
    texture.wAng = angle;
    return true;
  }

  setTextureLevel(texture, level = 1.0) {
    if (!texture) return false;
    texture.level = level;
    return true;
  }

  setTextureAlpha(texture, alpha = 1.0) {
    if (!texture) return false;
    texture.getAlphaFromRGB = alpha < 1.0;
    return true;
  }

  // === TEXTURE ANIMATION ===

  animateTextureOffset(texture, targetU, targetV, duration = 1000, loop = true) {
    if (!texture) return null;
    
    const frameRate = 60;
    const totalFrames = Math.floor((duration / 1000) * frameRate);
    
    // U Offset Animation
    const uAnimation = new Animation(
      `${texture.name}_uOffset`,
      'uOffset',
      frameRate,
      Animation.ANIMATIONTYPE_FLOAT,
      loop ? Animation.ANIMATIONLOOPMODE_CYCLE : Animation.ANIMATIONLOOPMODE_CONSTANT
    );
    
    uAnimation.setKeys([
      { frame: 0, value: texture.uOffset },
      { frame: totalFrames, value: targetU }
    ]);
    
    // V Offset Animation
    const vAnimation = new Animation(
      `${texture.name}_vOffset`,
      'vOffset',
      frameRate,
      Animation.ANIMATIONTYPE_FLOAT,
      loop ? Animation.ANIMATIONLOOPMODE_CYCLE : Animation.ANIMATIONLOOPMODE_CONSTANT
    );
    
    vAnimation.setKeys([
      { frame: 0, value: texture.vOffset },
      { frame: totalFrames, value: targetV }
    ]);
    
    texture.animations = [uAnimation, vAnimation];
    return this.scene.beginAnimation(texture, 0, totalFrames, loop, 1.0);
  }

  animateTextureRotation(texture, targetAngle, duration = 1000, loop = true) {
    if (!texture) return null;
    
    const frameRate = 60;
    const totalFrames = Math.floor((duration / 1000) * frameRate);
    
    const animation = new Animation(
      `${texture.name}_rotation`,
      'wAng',
      frameRate,
      Animation.ANIMATIONTYPE_FLOAT,
      loop ? Animation.ANIMATIONLOOPMODE_CYCLE : Animation.ANIMATIONLOOPMODE_CONSTANT
    );
    
    animation.setKeys([
      { frame: 0, value: texture.wAng },
      { frame: totalFrames, value: targetAngle }
    ]);
    
    texture.animations = [animation];
    return this.scene.beginAnimation(texture, 0, totalFrames, loop, 1.0);
  }

  // === DYNAMIC TEXTURE DRAWING ===

  drawTextOnTexture(texture, text, x, y, font = '60px Arial', color = 'white', backgroundColor = null) {
    if (!texture || !texture.drawText) return false;
    
    texture.drawText(text, x, y, font, color, backgroundColor, true);
    return true;
  }

  clearDynamicTexture(texture, color = null) {
    if (!texture || !texture.clear) return false;
    texture.clear(color);
    return true;
  }

  drawRectOnTexture(texture, x, y, width, height, color = 'white') {
    if (!texture || !texture.getContext) return false;
    
    const context = texture.getContext();
    context.fillStyle = color;
    context.fillRect(x, y, width, height);
    texture.update();
    return true;
  }

  drawCircleOnTexture(texture, centerX, centerY, radius, color = 'white') {
    if (!texture || !texture.getContext) return false;
    
    const context = texture.getContext();
    context.fillStyle = color;
    context.beginPath();
    context.arc(centerX, centerY, radius, 0, 2 * Math.PI);
    context.fill();
    texture.update();
    return true;
  }

  // === TEXTURE UV MANIPULATION ===

  createTextureMatrix(texture, offsetU = 0, offsetV = 0, scaleU = 1, scaleV = 1, rotation = 0) {
    if (!texture) return null;
    
    const matrix = Matrix.Identity();
    
    // Apply transformations
    const translationMatrix = Matrix.Translation(offsetU, offsetV, 0);
    const scaleMatrix = Matrix.Scaling(scaleU, scaleV, 1);
    const rotationMatrix = Matrix.RotationZ(rotation);
    
    matrix.multiplyInPlace(translationMatrix);
    matrix.multiplyInPlace(scaleMatrix); 
    matrix.multiplyInPlace(rotationMatrix);
    
    texture.uMatrix = matrix;
    return matrix;
  }

  // === TEXTURE SAMPLING ===

  setTextureSampling(texture, samplingMode) {
    if (!texture) return false;
    // TEXTURE_NEAREST_SAMPLINGMODE = 1, TEXTURE_BILINEAR_SAMPLINGMODE = 2, TEXTURE_TRILINEAR_SAMPLINGMODE = 3
    texture.updateSamplingMode(samplingMode);
    return true;
  }

  setTextureAnisotropicLevel(texture, level = 4) {
    if (!texture) return false;
    texture.anisotropicFilteringLevel = level;
    return true;
  }

  // === TEXTURE COORDINATES ===

  setTextureCoordinatesIndex(texture, index = 0) {
    if (!texture) return false;
    texture.coordinatesIndex = index;
    return true;
  }

  setTextureCoordinatesMode(texture, mode) {
    if (!texture) return false;
    // TEXTURE_EXPLICIT_MODE = 0, TEXTURE_SPHERICAL_MODE = 1, TEXTURE_PLANAR_MODE = 2, etc.
    texture.coordinatesMode = mode;
    return true;
  }

  // === RENDER TARGET TEXTURES ===

  addMeshToRenderTarget(renderTexture, mesh) {
    if (!renderTexture || !mesh || !renderTexture.renderList) return false;
    renderTexture.renderList.push(mesh);
    return true;
  }

  removeMeshFromRenderTarget(renderTexture, mesh) {
    if (!renderTexture || !mesh || !renderTexture.renderList) return false;
    const index = renderTexture.renderList.indexOf(mesh);
    if (index !== -1) {
      renderTexture.renderList.splice(index, 1);
      return true;
    }
    return false;
  }

  setRenderTargetSize(renderTexture, width, height) {
    if (!renderTexture || !renderTexture.resize) return false;
    renderTexture.resize({ width, height });
    return true;
  }

  // === TEXTURE UTILITIES ===

  getTextureSize(texture) {
    if (!texture) return null;
    
    return {
      width: texture.getBaseSize().width,
      height: texture.getBaseSize().height
    };
  }

  isTextureReady(texture) {
    if (!texture) return false;
    return texture.isReady();
  }

  cloneTexture(texture, _name = null) {
    if (!texture || !texture.clone) return null;
    return texture.clone();
  }

  disposeTexture(texture) {
    if (!texture || !texture.dispose) return false;
    texture.dispose();
    return true;
  }

  // === TEXTURE LOADING ===

  loadTextureAsync(url) {
    return new Promise((resolve, reject) => {
      const texture = new Texture(url, this.scene, true, false, Texture.TRILINEAR_SAMPLINGMODE, () => {
        resolve(texture);
      }, (message, _exception) => {
        reject(new Error(message || 'Failed to load texture'));
      });
    });
  }

  loadCubeTextureAsync(rootUrl, extensions) {
    return new Promise((resolve, reject) => {
      const texture = new CubeTexture(rootUrl, this.scene, extensions, false, null, () => {
        resolve(texture);
      }, (message, _exception) => {
        reject(new Error(message || 'Failed to load cube texture'));
      });
    });
  }

  // === TEXTURE GENERATION ===

  createColorTexture(name, color = [1, 1, 1], size = 256) {
    const texture = new DynamicTexture(name, size, this.scene);
    const context = texture.getContext();
    
    const r = Math.floor(color[0] * 255);
    const g = Math.floor(color[1] * 255);
    const b = Math.floor(color[2] * 255);
    
    context.fillStyle = `rgb(${r}, ${g}, ${b})`;
    context.fillRect(0, 0, size, size);
    texture.update();
    
    return texture;
  }

  createCheckerboardTexture(name, size = 256, squareSize = 32, color1 = [1, 1, 1], color2 = [0, 0, 0]) {
    const texture = new DynamicTexture(name, size, this.scene);
    const context = texture.getContext();
    
    const squares = Math.floor(size / squareSize);
    
    for (let x = 0; x < squares; x++) {
      for (let y = 0; y < squares; y++) {
        const color = (x + y) % 2 === 0 ? color1 : color2;
        const r = Math.floor(color[0] * 255);
        const g = Math.floor(color[1] * 255);
        const b = Math.floor(color[2] * 255);
        
        context.fillStyle = `rgb(${r}, ${g}, ${b})`;
        context.fillRect(x * squareSize, y * squareSize, squareSize, squareSize);
      }
    }
    
    texture.update();
    return texture;
  }

  createGradientTexture(name, size = 256, color1 = [1, 0, 0], color2 = [0, 0, 1], direction = 'horizontal') {
    const texture = new DynamicTexture(name, size, this.scene);
    const context = texture.getContext();
    
    let gradient;
    if (direction === 'horizontal') {
      gradient = context.createLinearGradient(0, 0, size, 0);
    } else if (direction === 'vertical') {
      gradient = context.createLinearGradient(0, 0, 0, size);
    } else {
      gradient = context.createLinearGradient(0, 0, size, size);
    }
    
    const color1Str = `rgb(${Math.floor(color1[0] * 255)}, ${Math.floor(color1[1] * 255)}, ${Math.floor(color1[2] * 255)})`;
    const color2Str = `rgb(${Math.floor(color2[0] * 255)}, ${Math.floor(color2[1] * 255)}, ${Math.floor(color2[2] * 255)})`;
    
    gradient.addColorStop(0, color1Str);
    gradient.addColorStop(1, color2Str);
    
    context.fillStyle = gradient;
    context.fillRect(0, 0, size, size);
    texture.update();
    
    return texture;
  }

  createNoiseTexture(name, size = 256) {
    const texture = new NoiseProceduralTexture(name, size, this.scene);
    return texture;
  }

  // === ADVANCED PROCEDURAL TEXTURES ===

  createWoodTextureAdvanced(name, size = 512, options = {}) {
    const texture = new WoodProceduralTexture(name, size, this.scene);
    if (options.woodColor) texture.woodColor = new Color3(...options.woodColor);
    if (options.ampScale !== undefined) texture.ampScale = options.ampScale;
    return texture;
  }

  createFireTextureAdvanced(name, size = 512, options = {}) {
    const texture = new FireProceduralTexture(name, size, this.scene);
    if (options.fireColors) {
      texture.fireColors = options.fireColors.map(c => new Color3(...c));
    }
    if (options.speed !== undefined) texture.speed = options.speed;
    return texture;
  }

  createGrassTextureAdvanced(name, size = 512, options = {}) {
    const texture = new GrassProceduralTexture(name, size, this.scene);
    if (options.grassColors) {
      texture.grassColors = options.grassColors.map(c => new Color3(...c));
    }
    if (options.groundColor) texture.groundColor = new Color3(...options.groundColor);
    return texture;
  }

  createMarbleTextureAdvanced(name, size = 512, options = {}) {
    const texture = new MarbleProceduralTexture(name, size, this.scene);
    if (options.marbleColor) texture.marbleColor = new Color3(...options.marbleColor);
    if (options.jointColor) texture.jointColor = new Color3(...options.jointColor);
    if (options.turbulencePower !== undefined) texture.turbulencePower = options.turbulencePower;
    return texture;
  }

  createBrickTextureAdvanced(name, size = 512, options = {}) {
    const texture = new BrickProceduralTexture(name, size, this.scene);
    if (options.brickColor) texture.brickColor = new Color3(...options.brickColor);
    if (options.jointColor) texture.jointColor = new Color3(...options.jointColor);
    if (options.numberOfBricksHeight !== undefined) texture.numberOfBricksHeight = options.numberOfBricksHeight;
    if (options.numberOfBricksWidth !== undefined) texture.numberOfBricksWidth = options.numberOfBricksWidth;
    return texture;
  }

  // === TEXTURE COORDINATES ===

  transformTextureCoordinates(texture, matrix) {
    if (!texture || !matrix) return false;
    
    const transformMatrix = Array.isArray(matrix) ? Matrix.FromArray(matrix) : matrix;
    texture.uMatrix = transformMatrix;
    return true;
  }

  setTextureMatrix(texture, offsetU, offsetV, scaleU, scaleV, rotation) {
    if (!texture) return false;
    
    const matrix = Matrix.Identity();
    Matrix.TranslationToRef(offsetU, offsetV, 0, matrix);
    matrix.multiplyInPlace(Matrix.Scaling(scaleU, scaleV, 1));
    matrix.multiplyInPlace(Matrix.RotationZ(rotation));
    
    texture.uMatrix = matrix;
    return true;
  }

  // === VIDEO TEXTURE CONTROLS ===

  playVideoTexture(texture) {
    if (!texture || !texture.video) return false;
    texture.video.play();
    return true;
  }

  pauseVideoTexture(texture) {
    if (!texture || !texture.video) return false;
    texture.video.pause();
    return true;
  }

  setVideoTextureTime(texture, time) {
    if (!texture || !texture.video) return false;
    texture.video.currentTime = time;
    return true;
  }

  setVideoTextureVolume(texture, volume) {
    if (!texture || !texture.video) return false;
    texture.video.volume = Math.max(0, Math.min(1, volume));
    return true;
  }

  // === TEXTURE INFO ===

  getTextureInfo(texture) {
    if (!texture) return null;
    
    return {
      name: texture.name || 'unnamed',
      url: texture.url || '',
      size: {
        width: texture.getBaseSize().width,
        height: texture.getBaseSize().height
      },
      ready: texture.isReady(),
      hasAlpha: texture.hasAlpha,
      format: texture.format,
      type: texture.type,
      samplingMode: texture.samplingMode,
      wrapU: texture.wrapU,
      wrapV: texture.wrapV,
      uOffset: texture.uOffset,
      vOffset: texture.vOffset,
      uScale: texture.uScale,
      vScale: texture.vScale,
      rotation: texture.wAng,
      level: texture.level
    };
  }

  getAllTextures() {
    return this.scene.textures.map(texture => ({
      name: texture.name || 'unnamed',
      type: texture.getClassName(),
      url: texture.url || '',
      ready: texture.isReady()
    }));
  }

  findTextureByName(name) {
    return this.scene.textures.find(texture => texture.name === name) || null;
  }

  // === TEXTURE ATLAS ===

  createTextureAtlas(name, sources, atlasSize = 1024) {
    const texture = new DynamicTexture(name, atlasSize, this.scene);
    const context = texture.getContext();
    
    const tilesPerRow = Math.ceil(Math.sqrt(sources.length));
    const tileSize = atlasSize / tilesPerRow;
    
    sources.forEach((source, index) => {
      const x = (index % tilesPerRow) * tileSize;
      const y = Math.floor(index / tilesPerRow) * tileSize;
      
      if (typeof source === 'string') {
        // Load image and draw to atlas
        const img = new Image();
        img.onload = () => {
          context.drawImage(img, x, y, tileSize, tileSize);
          texture.update();
        };
        img.src = source;
      } else if (source.color) {
        // Draw solid color
        const color = source.color;
        const r = Math.floor(color[0] * 255);
        const g = Math.floor(color[1] * 255);
        const b = Math.floor(color[2] * 255);
        context.fillStyle = `rgb(${r}, ${g}, ${b})`;
        context.fillRect(x, y, tileSize, tileSize);
        texture.update();
      }
    });
    
    return texture;
  }

  // === TEXTURE EFFECTS ===

  applyTextureFilter(texture, filter) {
    if (!texture || !texture.getContext) return false;
    
    const context = texture.getContext();
    context.filter = filter; // e.g., 'blur(5px)', 'brightness(1.5)', 'contrast(1.2)'
    texture.update();
    return true;
  }

  invertTextureColors(texture) {
    if (!texture || !texture.getContext) return false;
    
    const context = texture.getContext();
    const imageData = context.getImageData(0, 0, texture.getSize().width, texture.getSize().height);
    const data = imageData.data;
    
    for (let i = 0; i < data.length; i += 4) {
      data[i] = 255 - data[i];     // Red
      data[i + 1] = 255 - data[i + 1]; // Green
      data[i + 2] = 255 - data[i + 2]; // Blue
      // Alpha stays the same
    }
    
    context.putImageData(imageData, 0, 0);
    texture.update();
    return true;
  }

  adjustTextureBrightness(texture, brightness = 1.0) {
    if (!texture || !texture.getContext) return false;
    
    const context = texture.getContext();
    const imageData = context.getImageData(0, 0, texture.getSize().width, texture.getSize().height);
    const data = imageData.data;
    
    for (let i = 0; i < data.length; i += 4) {
      data[i] = Math.min(255, data[i] * brightness);     // Red
      data[i + 1] = Math.min(255, data[i + 1] * brightness); // Green
      data[i + 2] = Math.min(255, data[i + 2] * brightness); // Blue
    }
    
    context.putImageData(imageData, 0, 0);
    texture.update();
    return true;
  }

  // === TEXTURE COMPRESSION ===

  setTextureFormat(texture, format) {
    if (!texture) return false;
    // TEXTUREFORMAT_ALPHA = 0, TEXTUREFORMAT_LUMINANCE = 1, TEXTUREFORMAT_RGB = 4, TEXTUREFORMAT_RGBA = 5
    texture.format = format;
    return true;
  }

  enableTextureCompression(texture, format) {
    if (!texture) return false;
    // Use compressed texture formats like DXT1, DXT5, etc.
    texture.format = format;
    return true;
  }
}