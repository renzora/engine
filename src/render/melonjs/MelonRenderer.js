// MelonJS Renderer Implementation

import { BaseRenderer } from '../../api/render/BaseRenderer.js';

export default class MelonRenderer extends BaseRenderer {
  constructor(config) {
    super(config);
    this.game = null;
  }

  async initialize(canvas) {
    try {
      const me = await import('melonjs');
      
      // Initialize MelonJS engine first
      await me.boot();
      
      // Initialize MelonJS video system
      if (!me.video.init(canvas.clientWidth, canvas.clientHeight, {
        parent: canvas.parentElement,
        canvas: canvas,
        renderer: me.video.AUTO,
        preferWebGL1: false,
        scale: 'auto',
        scaleMethod: 'fill-min'
      })) {
        throw new Error('MelonJS video initialization failed');
      }
      
      // Set and initialize the "Play" Screen Object
      me.state.set(me.state.PLAY, new me.Stage());
      
      // Switch to the PLAY state
      me.state.change(me.state.PLAY);
      
      this.game = me;
      this._notifyReady();
      return true;
    } catch (error) {
      console.error('MelonJS initialization failed:', error);
      return false;
    }
  }

  render() {
    if (this.initialized && this.game) {
      // MelonJS handles its own render loop
      return true;
    }
    return false;
  }

  resize(width, height) {
    if (this.game && this.game.video) {
      this.game.video.updateDisplaySize(width, height);
      this.game.game.viewport.resize(width, height);
    }
  }

  dispose() {
    if (this.game) {
      this.game.video.renderer.flush();
    }
  }

  getGame() {
    return this.game;
  }

  getStage() {
    return this.game ? this.game.game.world : null;
  }

  // API implementation
  async loadScene(sceneData) {
    return true;
  }

  async updateScene(sceneData) {
    return true;
  }

  async updateCamera(cameraData) {
    if (this.game && this.game.game && this.game.game.viewport) {
      if (cameraData.position) {
        this.game.game.viewport.move(cameraData.position.x, cameraData.position.y);
      }
    }
    return true;
  }

  async updateLights(lightData) {
    return true;
  }

  async addObject(objectData) {
    return null;
  }

  async removeObject(objectId) {
    return true;
  }

  async updateObject(objectId, objectData) {
    return true;
  }

  async updateMaterial(materialId, materialData) {
    return true;
  }

  getStats() {
    return {
      renderer: 'MelonJS',
      fps: this.game?.timer?.fps || 0,
      objects: 0
    };
  }

  async captureFrame() {
    return null;
  }
}