import { createSignal, createEffect, onCleanup } from 'solid-js';
import { Color3 } from '@babylonjs/core/Maths/math.color';
import { editorStore } from '@/plugins/editor/stores/EditorStore';
import { viewportStore } from '@/plugins/editor/stores/ViewportStore';
import { sceneActions, babylonScene } from '../store';
import { useCameraController } from '../CameraController';
import { useGrid } from '../Grid';
import { useEngineManager } from '../hooks/useEngineManager';
import { useSceneManager } from '../hooks/useSceneManager';
import { useKeyboardControls } from '../hooks/useKeyboardControls';
import { useAssetLoader } from '../hooks/useAssetLoader';
import { useViewportInteraction } from '../hooks/useViewportInteraction';
import Stats from 'stats.js';
import LoadingTooltip from '@/plugins/editor/ui/LoadingTooltip';
import ModelImportDialog from '@/plugins/editor/ui/ModelImportDialog';
import { useEngineReady } from '../index';

function ViewportCanvas(props) {
  const [canvasRef, setCanvasRef] = createSignal();
  let statsRef;
  
  const settings = () => editorStore.settings;
  const viewport = () => viewportStore;
  
  const [canvasInstance, setCanvasInstance] = createSignal(null);
  const [canvasElement, setCanvasElement] = createSignal(null);
  const [importDialog, setImportDialog] = createSignal({
    isOpen: false,
    modelName: '',
    assetData: null,
    position: null,
    modelAnalysis: null
  });
  
  const { engineInstance, createEngine, disposeEngine } = useEngineManager(canvasRef, settings);
  const { sceneInstance, createScene, disposeScene } = useSceneManager();
  
  const cameraController = useCameraController(
    () => sceneInstance()?._camera, 
    canvasInstance, 
    sceneInstance
  );
  
  useGrid(sceneInstance());
  useKeyboardControls(sceneInstance, cameraController);
  
  const { loadingTooltip, handleDragOver, handleDrop } = useAssetLoader(sceneInstance, canvasRef);
  const { setupPointerEvents } = useViewportInteraction(sceneInstance, cameraController);

const initializeViewport = async () => {
  try {
    disposeScene();
    disposeEngine();

    const { engine, canvas } = await createEngine(settings().viewport.renderingEngine || 'webgl');
    if (!engine) return;

    if (canvas && canvas !== canvasRef()) {
      setCanvasRef(canvas);
      setCanvasElement(canvas);
      setCanvasInstance(canvas);
    }

    const scene = await createScene(engine);
    sceneActions.updateBabylonScene(scene);
    scene._camera.attachControl(canvas, false);

    await setupPointerEvents(scene);
      
      setTimeout(() => {
        if (engine && canvasRef()) {
          console.log('Canvas dimensions:', canvasRef().clientWidth, 'x', canvasRef().clientHeight);
          engine.resize();
        }
      }, 100);

      engine.runRenderLoop(() => {
        if (statsRef) {
          statsRef.begin();
        }
        
        if (cameraController) {
          cameraController.handleKeyboardMovement();
        }
        
        scene.render();
        
        if (statsRef) {
          statsRef.end();
        }
      });

      const handleResize = () => {
        if (canvasRef() && engine) {
          engine.resize();
          
          if (scene._camera) {
            const canvas = canvasRef();
            const aspectRatio = canvas.clientWidth / canvas.clientHeight;
            
            if (scene._camera.fov) {
              scene._camera.fov = Math.PI / 3;
            }
          }
        }
      };
      
      window.addEventListener('resize', handleResize);
      
      let resizeObserver = null;
      if (canvasRef() && window.ResizeObserver) {
        resizeObserver = new ResizeObserver((entries) => {
          clearTimeout(window._resizeTimeout);
          window._resizeTimeout = setTimeout(() => {
            handleResize();
          }, 16);
        });
        
        resizeObserver.observe(canvasRef());
        
        if (canvasRef().parentElement) {
          resizeObserver.observe(canvasRef().parentElement);
        }
      }

      return () => {
        window.removeEventListener('resize', handleResize);
        
        if (resizeObserver) {
          resizeObserver.disconnect();
        }
        
        if (window._resizeTimeout) {
          clearTimeout(window._resizeTimeout);
        }
        
        if (statsRef && statsRef.dom.parentElement) {
          statsRef.dom.parentElement.removeChild(statsRef.dom);
          statsRef = null;
        }
        
        disposeScene();
        disposeEngine();
        
        sceneActions.updateBabylonScene(null);
      };
    } catch (error) {
      console.error('Failed to initialize viewport:', error);
    }
  };

  const { isEngineReady } = useEngineReady();
  createEffect(() => {
    const canvas = canvasElement();
    console.log('ViewportCanvas: Effect triggered - canvas:', canvas, 'canvasInstance:', canvasInstance(), 'isEngineReady:', isEngineReady()); // Use the context value
    if (!canvas || canvasInstance() === canvas || !isEngineReady()) {
      console.log('ViewportCanvas: Skipping initialization - no canvas, already initialized, or engine not ready');
      return;
    }
    
    console.log('ViewportCanvas: Setting up canvas instance and initializing viewport');
    setCanvasInstance(canvas);
    
    let cleanup;
    initializeViewport().then(cleanupFn => {
      cleanup = cleanupFn;
    });

    onCleanup(() => {
      if (cleanup) cleanup();
    });
  });


  createEffect(() => {
    const scene = sceneInstance();
    if (scene) {
      const bgColor = Color3.FromHexString(settings().viewport.backgroundColor || '#1a202c');
      scene.clearColor = bgColor;
    }
  });

  createEffect(() => {
    const scene = sceneInstance();
    if (scene && scene._applyRenderMode) {
      const renderMode = viewport().renderMode || 'solid';
      scene._applyRenderMode(renderMode);
    }
  });

  createEffect(() => {
    if (!canvasRef()) return;

    if (settings().editor.showStats && !statsRef) {
      const stats = new Stats();
      stats.showPanel(0);
      stats.dom.style.position = 'absolute';
      stats.dom.style.left = '10px';
      stats.dom.style.bottom = '10px';
      stats.dom.style.top = 'auto';
      stats.dom.style.zIndex = '1000';
      
      const viewportContainer = canvasRef().parentElement;
      viewportContainer.appendChild(stats.dom);
      statsRef = stats;
    } else if (!settings().editor.showStats && statsRef) {
      if (statsRef.dom.parentElement) {
        statsRef.dom.parentElement.removeChild(statsRef.dom);
      }
      statsRef = null;
    }
  });

  return (
    <div 
      style={{ 
        width: '100%', 
        height: '100%', 
        backgroundColor: '#333333',
        position: 'relative',
        ...props.style 
      }}
      onClick={() => {
        canvasRef()?.focus();
      }}
      onContextMenu={(e) => {
        e.preventDefault();
      }}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
    >
      <canvas
        ref={el => {
          setCanvasRef(el);
          setCanvasElement(el);
        }}
        style={{ 
          width: '100%', 
          height: '100%',
          outline: 'none',
          display: 'block'
        }}
        tabIndex={0}
      />
      
      {props.children}
      
      <LoadingTooltip
        isVisible={loadingTooltip().isVisible}
        message={loadingTooltip().message}
        position={loadingTooltip().position}
        progress={loadingTooltip().progress}
      />
      
      <ModelImportDialog
        isOpen={importDialog().isOpen}
        onClose={() => setImportDialog(prev => ({ ...prev, isOpen: false }))}
        modelName={importDialog().modelName}
        modelAnalysis={importDialog().modelAnalysis}
      />
    </div>
  );
}

export default ViewportCanvas;