import { createSignal, createEffect, onMount, onCleanup } from 'solid-js';
import { editorStore } from '@/layout/stores/EditorStore';
import { useCameraController } from '@/api/render/CameraController';
import { rendererAPI } from '@/api/render/RendererAPI';
import PlayCanvasRenderer from './PlayCanvasRenderer';
import Stats from 'stats.js';

export const PlayCanvasViewport = (props) => {
  const [canvasRef, setCanvasRef] = createSignal(null);
  const [initialized, setInitialized] = createSignal(false);
  const [renderer, setRenderer] = createSignal(null);
  const [frameCount, setFrameCount] = createSignal(0);
  const [statsRef, setStatsRef] = createSignal(null);

  // Use the universal camera controller
  const cameraController = useCameraController(canvasRef);

  // Initialize PlayCanvas renderer
  const initializeRenderer = async () => {
    try {
      const canvas = canvasRef();
      if (!canvas) return;

      console.log('🎯 Initializing PlayCanvas renderer...');
      
      // Create renderer instance
      const playCanvasRenderer = new PlayCanvasRenderer({
        id: 'playcanvas',
        name: 'PlayCanvas'
      });

      // Register and set as active renderer
      rendererAPI.registerRenderer('playcanvas', playCanvasRenderer);
      await rendererAPI.setActiveRenderer('playcanvas');

      // Initialize the renderer
      const success = await rendererAPI.initialize(canvas);
      if (!success) {
        throw new Error('Failed to initialize PlayCanvas renderer');
      }

      // Set up scene content (cube, ground, lights)
      await setupScene(playCanvasRenderer);

      setRenderer(playCanvasRenderer);
      setInitialized(true);

    } catch (error) {
      console.error('🎯 Failed to initialize PlayCanvas:', error);
    }
  };

  // Set up the demo scene
  const setupScene = async (playCanvasRenderer) => {
    const app = playCanvasRenderer.getApp();
    const pc = await import('playcanvas');

    // Create light entity
    const light = new pc.Entity('light');
    light.addComponent('light', {
      type: pc.LIGHTTYPE_DIRECTIONAL,
      color: new pc.Color(1, 1, 1),
      intensity: 1,
      castShadows: true,
      shadowBias: 0.1,
      shadowDistance: 25
    });
    light.setEulerAngles(45, 135, 0);
    app.root.addChild(light);
    
    // Create ambient light
    const ambientLight = new pc.Entity('ambient');
    ambientLight.addComponent('light', {
      type: pc.LIGHTTYPE_AMBIENT,
      color: new pc.Color(0.4, 0.4, 0.4),
      intensity: 0.3
    });
    app.root.addChild(ambientLight);
    
    // Create cube entity
    const cube = new pc.Entity('cube');
    cube.addComponent('render', {
      type: 'box',
      castShadows: true,
      receiveShadows: true
    });
    
    // Create material
    const material = new pc.StandardMaterial();
    material.diffuse = new pc.Color(1, 0.4, 0.4);
    material.shininess = 60;
    material.useMetalness = true;
    material.metalness = 0.3;
    material.update();
    
    cube.render.material = material;
    cube.setPosition(0, -1.5, 0); // Position cube on the floor
    app.root.addChild(cube);
    
    // Create ground plane
    const ground = new pc.Entity('ground');
    ground.addComponent('render', {
      type: 'plane',
      receiveShadows: true
    });
    
    const groundMaterial = new pc.StandardMaterial();
    groundMaterial.diffuse = new pc.Color(0.5, 0.5, 0.5);
    groundMaterial.update();
    
    ground.render.material = groundMaterial;
    ground.setPosition(0, -2, 0);
    ground.setLocalScale(10, 1, 10);
    app.root.addChild(ground);

    // Start the application
    app.start();

    // Update loop for frame counting
    app.on('update', (dt) => {
      if (statsRef()) {
        statsRef().begin();
      }
      setFrameCount(prev => prev + 1);
    });
    
    app.on('postrender', () => {
      if (statsRef()) {
        statsRef().end();
      }
    });
  };

  // Handle canvas mounting
  onMount(() => {
    const canvas = canvasRef();
    if (canvas) {
      initializeRenderer();
    }
  });

  // Stats integration using same pattern as Babylon.js
  createEffect(() => {
    if (!canvasRef()) return;

    const settings = () => editorStore.settings;
    if (settings().editor.showStats && !statsRef()) {
      const stats = new Stats();
      stats.showPanel(0);
      stats.dom.style.position = 'absolute';
      stats.dom.style.left = '10px';
      stats.dom.style.bottom = '10px';
      stats.dom.style.top = 'auto';
      stats.dom.style.zIndex = '1000';
      
      const viewportContainer = canvasRef().parentElement;
      if (viewportContainer) {
        viewportContainer.appendChild(stats.dom);
        setStatsRef(stats);
      }
    } else if (!settings().editor.showStats && statsRef()) {
      if (statsRef().dom.parentElement) {
        statsRef().dom.parentElement.removeChild(statsRef().dom);
      }
      setStatsRef(null);
    }
  });

  // Cleanup on unmount
  onCleanup(() => {
    rendererAPI.dispose();
  });

  return (
    <div style={props.style} class="relative w-full h-full">
      <canvas
        ref={setCanvasRef}
        style={{
          width: '100%',
          height: '100%',
          display: 'block'
        }}
        onContextMenu={props.onContextMenu}
      />
      
      <div style={{
        position: 'absolute',
        top: '10px',
        left: '10px',
        color: '#ffffff',
        'font-family': 'monospace',
        'font-size': '12px',
        'background': 'rgba(0,0,0,0.7)',
        padding: '4px 8px',
        'border-radius': '4px'
      }}>
        🎯 PlayCanvas {initialized() ? `(Frame ${frameCount()}) API` : '(Loading...)'}
      </div>
    </div>
  );
};