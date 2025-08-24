import { createSignal } from 'solid-js'
import { Engine } from '@babylonjs/core/Engines/engine'
import { WebGPUEngine } from '@babylonjs/core/Engines/webgpuEngine'

export const useEngineManager = (canvasRefSignal, settings) => {
  const [engineInstance, setEngineInstance] = createSignal(null);

  const createEngine = async (renderingEngine = 'webgl') => {
    let canvas = canvasRefSignal();
    if (!canvas) return null;

    let engine;

    try {
      // Force WebGL for this renderer
      console.log('🌐 Creating WebGL engine (forced)...');
      engine = new Engine(canvas, true, {
        powerPreference: 'high-performance',
        antialias: true,
        stencil: true,
        preserveDrawingBuffer: false
      });
      console.log('✅ WebGL engine created successfully');
    } catch (error) {
      console.error('WebGL engine creation failed, creating fallback canvas:', error);
      const newCanvas = document.createElement('canvas');
      newCanvas.style.cssText = canvas.style.cssText;
      newCanvas.tabIndex = canvas.tabIndex;
      canvas.parentElement.replaceChild(newCanvas, canvas);
      canvas = newCanvas;
      engine = new Engine(canvas, true);
    }

    if (engine) {
      engine.onDisposeObservable.add(() => setEngineInstance(null));
      setEngineInstance(engine);
    }

    return { engine, canvas };
  };

  const disposeEngine = () => {
    const engine = engineInstance();
    if (engine && !engine.isDisposed) {
      try { engine.dispose(); } catch {}
    }
    setEngineInstance(null);
  };

  return { engineInstance, createEngine, disposeEngine };
};
