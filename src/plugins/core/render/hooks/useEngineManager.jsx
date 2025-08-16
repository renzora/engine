import { createSignal } from 'solid-js'
import { Engine } from '@babylonjs/core/Engines/engine'
import { WebGPUEngine } from '@babylonjs/core/Engines/webgpuEngine'
import { editorActions } from '@/plugins/editor/stores/EditorStore'

export const useEngineManager = (canvasRefSignal, settings) => {
  const [engineInstance, setEngineInstance] = createSignal(null);

  const createEngine = async (renderingEngine) => {
    let canvas = canvasRefSignal();
    if (!canvas) return null;

    let engine;

    try {
      if (renderingEngine === 'webgpu') {
        if (!navigator.gpu) {
          engine = new Engine(canvas, true);
        } else {
          const webGPUEngine = new WebGPUEngine(canvas, {
            adaptToDeviceRatio: true,
            antialias: true
          });
          await webGPUEngine.initAsync();
          engine = webGPUEngine;
        }
      } else {
        try {
          engine = new Engine(canvas, true);
        } catch (e) {
          const newCanvas = document.createElement('canvas');
          newCanvas.style.cssText = canvas.style.cssText;
          newCanvas.tabIndex = canvas.tabIndex;
          canvas.parentElement.replaceChild(newCanvas, canvas);
          canvas = newCanvas;
          engine = new Engine(canvas, true);
        }
      }
    } catch (error) {
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
