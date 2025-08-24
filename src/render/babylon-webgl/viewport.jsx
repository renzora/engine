import { createSignal, createEffect, onCleanup } from 'solid-js';
import { Color3, Color4 } from '@babylonjs/core/Maths/math.color';
import { editorStore } from '@/layout/stores/EditorStore';
import { viewportStore } from '@/layout/stores/ViewportStore';
import { useCameraController } from './CameraController';
import { useEngineManager } from './hooks/useEngineManager';
import { useSceneManager } from './hooks/useSceneManager';
import { useKeyboardControls } from './hooks/useKeyboardControls';
import { useAssetLoader } from './hooks/useAssetLoader';
import { useViewportInteraction } from './hooks/useViewportInteraction';
import { useGrid } from './Grid';
import Stats from 'stats.js';
import { LoadingTooltip } from '@/ui';
import ModelImportDialog from '../../../components/ModelImportDialog.jsx';
import { useEngineReady } from './index';

function ViewportCanvas(props) {
  const [canvasRef, setCanvasRef] = createSignal();
  const [statsRef, setStatsRef] = createSignal(null);

  // Helper function to convert OKLCH to RGB using CSS Color Module 4 conversion
  const oklchToRgb = (l, c, h) => {
    // Convert OKLCH to OKLAB
    const hRad = h * Math.PI / 180;
    const a = c * Math.cos(hRad);
    const b = c * Math.sin(hRad);
    
    // Convert OKLAB to linear RGB using matrices from CSS Color Module 4 spec
    const l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    const m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    const s_ = l - 0.0894841775 * a - 1.2914855480 * b;
    
    const l3 = l_ * l_ * l_;
    const m3 = m_ * m_ * m_;
    const s3 = s_ * s_ * s_;
    
    let r = +4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3;
    let g = -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3;
    let bl = -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3;
    
    // Gamma correction for sRGB
    r = r > 0.0031308 ? 1.055 * Math.pow(r, 1/2.4) - 0.055 : 12.92 * r;
    g = g > 0.0031308 ? 1.055 * Math.pow(g, 1/2.4) - 0.055 : 12.92 * g;
    bl = bl > 0.0031308 ? 1.055 * Math.pow(bl, 1/2.4) - 0.055 : 12.92 * bl;
    
    return {
      r: Math.max(0, Math.min(1, r)),
      g: Math.max(0, Math.min(1, g)),
      b: Math.max(0, Math.min(1, bl))
    };
  };

  // Helper function to parse color string and convert to RGB
  const parseColorToRgb = (colorStr) => {
    if (colorStr.startsWith('oklch(')) {
      const match = colorStr.match(/oklch\(([\d.%]+)\s+([\d.]+)\s+([\d.]+)\)/);
      if (match) {
        let l = parseFloat(match[1]);
        const c = parseFloat(match[2]);
        const h = parseFloat(match[3]);
        
        // Convert percentage lightness to decimal
        if (match[1].includes('%')) {
          l = l / 100;
        }
        
        console.log(`Parsing OKLCH: l=${l}, c=${c}, h=${h}`);
        const result = oklchToRgb(l, c, h);
        console.log(`OKLCH to RGB result:`, result);
        return result;
      }
    }
    
    if (colorStr.startsWith('rgb(')) {
      const match = colorStr.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
      if (match) {
        return {
          r: parseInt(match[1]) / 255,
          g: parseInt(match[2]) / 255,
          b: parseInt(match[3]) / 255
        };
      }
    }
    
    if (colorStr.startsWith('#')) {
      const hex = colorStr.slice(1);
      return {
        r: parseInt(hex.slice(0, 2), 16) / 255,
        g: parseInt(hex.slice(2, 4), 16) / 255,
        b: parseInt(hex.slice(4, 6), 16) / 255
      };
    }
    
    return null;
  };

  // Helper function to get DaisyUI color from CSS custom properties
  const getDaisyUIColor = (colorName) => {
    const style = getComputedStyle(document.documentElement);
    // Map short names to actual DaisyUI CSS custom property names
    const colorPropertyMap = {
      'p': 'color-primary',
      's': 'color-secondary', 
      'a': 'color-accent',
      'b1': 'color-base-100',
      'b2': 'color-base-200',
      'b3': 'color-base-300',
      'bc': 'color-base-content',
      'n': 'color-neutral'
    };
    
    const propertyName = colorPropertyMap[colorName] || colorName;
    const colorValue = style.getPropertyValue(`--${propertyName}`).trim();
    
    console.log(`Getting DaisyUI color for --${propertyName}: "${colorValue}"`);
    
    if (colorValue) {
      const rgb = parseColorToRgb(colorValue);
      if (rgb) {
        console.log(`Parsed RGB for --${propertyName}:`, rgb);
        return new Color3(rgb.r, rgb.g, rgb.b);
      }
    }
    
    console.log(`Using fallback color for --${propertyName}`);
    
    // Fallback colors that match common DaisyUI themes
    switch (colorName) {
      case 'p': return new Color3(0.235, 0.506, 0.957); // primary blue
      case 's': return new Color3(0.545, 0.365, 0.957); // secondary purple
      case 'a': return new Color3(0.024, 0.714, 0.831); // accent cyan
      case 'b1': return new Color3(0.067, 0.094, 0.149); // base-100 dark
      case 'b2': return new Color3(0.122, 0.161, 0.216); // base-200
      case 'b3': return new Color3(0.220, 0.255, 0.318); // base-300
      case 'bc': return new Color3(0.9, 0.9, 0.9); // base-content light
      default: return new Color3(0.235, 0.506, 0.957); // fallback to primary
    }
  };
  
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
  
  useGrid(sceneInstance);
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
    // CLEAN SCENE: No store updates needed
    scene._camera.attachControl(canvas, false);

    await setupPointerEvents(scene);
      
      setTimeout(() => {
        if (engine && canvasRef()) {
          console.log('Canvas dimensions:', canvasRef().clientWidth, 'x', canvasRef().clientHeight);
          engine.resize();
        }
      }, 100);

      engine.runRenderLoop(() => {
        if (statsRef()) {
          statsRef().begin();
        }
        
        if (cameraController) {
          cameraController.handleKeyboardMovement();
        }
        
        scene.render();
        
        if (statsRef()) {
          statsRef().end();
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

      // Listen to panel dimension changes for proper viewport adjustment
      createEffect(() => {
        const rightWidth = editorStore.ui.rightPanelWidth;
        const bottomHeight = editorStore.ui.bottomPanelHeight;
        
        // Trigger resize when panel dimensions change
        setTimeout(() => {
          handleResize();
        }, 16);
      });

      return () => {
        window.removeEventListener('resize', handleResize);
        
        if (resizeObserver) {
          resizeObserver.disconnect();
        }
        
        if (window._resizeTimeout) {
          clearTimeout(window._resizeTimeout);
        }
        
        if (statsRef() && statsRef().dom.parentElement) {
          statsRef().dom.parentElement.removeChild(statsRef().dom);
          setStatsRef(null);
        }
        
        disposeScene();
        disposeEngine();
        
        // CLEAN SCENE: No store updates needed
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

  // Cleanup stats on component unmount
  onCleanup(() => {
    if (statsRef() && statsRef().dom.parentElement) {
      statsRef().dom.parentElement.removeChild(statsRef().dom);
      setStatsRef(null);
    }
  });


  // Function to update scene colors 
  const updateSceneColors = () => {
    const scene = sceneInstance();
    if (!scene) return;

    const backgroundSetting = settings().viewport.backgroundColor;
    
    if (backgroundSetting === 'theme' || !backgroundSetting) {
      // Use DaisyUI base-100 color for background
      const bgColor = getDaisyUIColor('b1');
      scene.clearColor = new Color4(bgColor.r, bgColor.g, bgColor.b, 1.0);
      console.log('ViewportCanvas: Using DaisyUI theme background color');
    } else {
      // Use custom color from settings
      const bgColor = Color3.FromHexString(backgroundSetting);
      scene.clearColor = new Color4(bgColor.r, bgColor.g, bgColor.b, 1.0);
      console.log('ViewportCanvas: Using custom background color:', backgroundSetting);
    }
  };

  // Watch for settings changes
  createEffect(() => {
    const scene = sceneInstance();
    if (scene) {
      updateSceneColors();
    }
  });

  // Watch for theme changes (only when using theme colors)
  createEffect(() => {
    const scene = sceneInstance();
    const backgroundSetting = settings().viewport.backgroundColor;
    
    if (scene && (backgroundSetting === 'theme' || !backgroundSetting)) {
      // Watch for theme changes on the document element
      const themeObserver = new MutationObserver((mutations) => {
        mutations.forEach((mutation) => {
          if (mutation.type === 'attributes' && mutation.attributeName === 'data-theme') {
            // Small delay to ensure CSS variables are updated
            setTimeout(() => {
              console.log('ViewportCanvas: Theme changed, updating colors');
              updateSceneColors();
            }, 50);
          }
        });
      });
      
      themeObserver.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ['data-theme']
      });
      
      onCleanup(() => {
        themeObserver.disconnect();
      });
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

  return (
    <div 
      style={{ 
        width: '100%', 
        height: '100%', 
        backgroundColor: 'oklch(var(--b1))',
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