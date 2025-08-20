export * from './IRenderAPI.jsx';
export * from './RenderContext.jsx';

export { BabylonRenderer } from '../../render/babylon/index.jsx';
export { TorusRenderer } from '../../render/torus/index.jsx';
import { registerRenderer } from './RenderContext.jsx';
import { RendererType } from './IRenderAPI.jsx';
import { BabylonRenderer } from '../../render/babylon/index.jsx';
import { TorusRenderer } from '../../render/torus/index.jsx';

registerRenderer(RendererType.TORUS, TorusRenderer);
registerRenderer(RendererType.BABYLON, BabylonRenderer);

console.log('[Renderers] Registered renderers:', Array.from([
  RendererType.TORUS, 
  RendererType.BABYLON
]));

export function getRendererStatus() {
  return {
    [RendererType.TORUS]: {
      available: true,
      reason: 'Primary renderer (Torus)'
    },
    [RendererType.BABYLON]: {
      available: true,
      reason: 'Babylon.js renderer'
    }
  };
}