// Export the render API interface
export * from '../api/index.js';

// Export renderer implementations
export { BabylonRenderer } from './babylon/index.js';
export { ThreeRenderer } from './three/index.js';
export { TorusRenderer } from './torus/index.js';
export { PixiRenderer } from './pixi/index.jsx';
export { PlayCanvasRenderer } from './playcanvas/index.jsx';
export { PhaserRenderer } from './phaser/index.jsx';

// Auto-register available renderers
import { registerRenderer } from '../api/RenderContext.jsx';
import { RendererType } from '../api/IRenderAPI.js';
import { BabylonRenderer } from './babylon/index.js';
import { ThreeRenderer } from './three/index.js';
import { TorusRenderer } from './torus/index.js';
import { PixiRenderer } from './pixi/index.jsx';
import { PlayCanvasRenderer } from './playcanvas/index.jsx';
import { PhaserRenderer } from './phaser/index.jsx';

// Register Torus (primary custom renderer)
registerRenderer(RendererType.TORUS, TorusRenderer);

// Register Babylon.js 
registerRenderer(RendererType.BABYLON, BabylonRenderer);

// Register Three.js 
registerRenderer(RendererType.THREE, ThreeRenderer);

// Register PixiJS
registerRenderer(RendererType.PIXI, PixiRenderer);

// Register PlayCanvas
registerRenderer(RendererType.PLAYCANVAS, PlayCanvasRenderer);

// Register Phaser
registerRenderer(RendererType.PHASER, PhaserRenderer);

console.log('[Renderers] Registered renderers:', Array.from([
  RendererType.TORUS, 
  RendererType.BABYLON, 
  RendererType.THREE,
  RendererType.PIXI,
  RendererType.PLAYCANVAS,
  RendererType.PHASER
]));

// Utility function to check renderer availability
export function getRendererStatus() {
  return {
    [RendererType.TORUS]: {
      available: true,
      reason: 'Primary renderer (Torus)'
    },
    [RendererType.BABYLON]: {
      available: true,
      reason: 'Babylon.js renderer'
    },
    [RendererType.THREE]: {
      available: true,
      reason: 'Three.js renderer'
    },
    [RendererType.PIXI]: {
      available: true,
      reason: 'PixiJS 2D/WebGL renderer'
    },
    [RendererType.PLAYCANVAS]: {
      available: true,
      reason: 'PlayCanvas 3D engine'
    },
    [RendererType.PHASER]: {
      available: true,
      reason: 'Phaser game engine'
    },
    [RendererType.WEBGPU]: {
      available: navigator.gpu !== undefined,
      reason: navigator.gpu ? 'WebGPU supported' : 'WebGPU not supported in this browser'
    }
  };
}