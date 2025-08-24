import { createSignal } from 'solid-js'
import { Engine } from '@babylonjs/core/Engines/engine'
import { WebGPUEngine } from '@babylonjs/core/Engines/webgpuEngine'

export const useEngineManager = (canvasRefSignal, settings) => {
  const [engineInstance, setEngineInstance] = createSignal(null);

  const createEngine = async (renderingEngine = 'webgpu') => {
    let canvas = canvasRefSignal();
    if (!canvas) return null;

    let engine;

    try {
      // Force WebGPU for this renderer
      if (!navigator.gpu) {
        console.log('⚠️ WebGPU not supported, falling back to WebGL');
        engine = new Engine(canvas, true, {
          powerPreference: 'high-performance',
          antialias: true
        });
      } else {
        console.log('🔮 Creating WebGPU engine (forced)...');
        const webGPUEngine = new WebGPUEngine(canvas, {
          adaptToDeviceRatio: true,
          antialias: true,
          powerPreference: 'high-performance'
        });
        await webGPUEngine.initAsync();
        engine = webGPUEngine;
        console.log('✅ WebGPU engine created successfully');
      }
    } catch (error) {
      console.error('WebGPU engine creation failed, falling back to WebGL:', error);
      engine = new Engine(canvas, true, {
        powerPreference: 'high-performance',
        antialias: true
      });
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
