// PixiJS Renderer Implementation

export default class PixiRenderer {
  constructor(config) {
    this.config = config;
    this.app = null;
    this.initialized = false;
  }

  async initialize(canvas) {
    try {
      const PIXI = await import('pixi.js');
      
      // Create PixiJS application
      this.app = new PIXI.Application();
      await this.app.init({
        canvas: canvas,
        width: canvas.clientWidth,
        height: canvas.clientHeight,
        backgroundColor: 0x1a1a26,
        antialias: true,
        resolution: window.devicePixelRatio,
        autoDensity: true
      });
      
      this.initialized = true;
      return true;
    } catch (error) {
      console.error('PixiJS initialization failed:', error);
      return false;
    }
  }

  render() {
    if (this.initialized && this.app) {
      // PixiJS handles its own render loop via ticker
      return true;
    }
    return false;
  }

  resize(width, height) {
    if (this.app) {
      this.app.renderer.resize(width, height);
    }
  }

  dispose() {
    if (this.app) {
      this.app.destroy(true);
    }
  }

  getApp() {
    return this.app;
  }

  getStage() {
    return this.app ? this.app.stage : null;
  }
}