import { createSignal, createEffect, onCleanup, onMount, Show } from 'solid-js';
import { editorStore } from '@/layout/stores/EditorStore';
import { viewportStore } from '@/layout/stores/ViewportStore';
import { useRenderContext } from '@/api';
import Stats from 'stats.js';
import { LoadingTooltip } from '@/ui';

function ViewportCanvasInner(props) {
  const [canvasRef, setCanvasRef] = createSignal();
  const renderContext = useRenderContext();
  let statsRef;

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
        
        return oklchToRgb(l, c, h);
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
    
    if (colorValue) {
      const rgb = parseColorToRgb(colorValue);
      if (rgb) {
        return rgb;
      }
    }
    
    // Fallback colors that match common DaisyUI themes
    switch (colorName) {
      case 'p': return { r: 0.235, g: 0.506, b: 0.957 }; // primary blue
      case 's': return { r: 0.545, g: 0.365, b: 0.957 }; // secondary purple
      case 'a': return { r: 0.024, g: 0.714, b: 0.831 }; // accent cyan
      case 'b1': return { r: 0.067, g: 0.094, b: 0.149 }; // base-100 dark
      case 'b2': return { r: 0.122, g: 0.161, b: 0.216 }; // base-200
      case 'b3': return { r: 0.220, g: 0.255, b: 0.318 }; // base-300
      case 'bc': return { r: 0.9, g: 0.9, b: 0.9 }; // base-content light
      default: return { r: 0.235, g: 0.506, b: 0.957 }; // fallback to primary
    }
  };

  const settings = () => editorStore.settings;
  const viewport = () => viewportStore;

  // Set canvas when it becomes available
  createEffect(() => {
    const canvas = canvasRef();
    if (canvas) {
      console.log('[ViewportCanvas] Canvas ready, setting on render context');
      renderContext.setCanvas(canvas);
    }
  });

  // Initialize scene when renderer is ready
  createEffect(() => {
    const renderer = renderContext.renderer();
    
    if (renderer && !renderContext.isLoading()) {
      console.log('[ViewportCanvas] Setting up scene with renderer:', renderer.getRendererName());
      setupScene(renderer);
    }
  });

  const setupScene = async (renderer) => {
    try {
      // Create scene
      const scene = renderer.createScene();
      
      // Set background color based on theme
      updateSceneColors(renderer);
      
      // Create unified camera with identical positioning and behavior
      const rendererName = renderer.getRendererName();
      const cameraOptions = {
        position: { x: 0, y: 5, z: 10 },  // Standardized position
        fov: 75,  // Standardized field of view
        near: 0.1,
        far: 1000
      };
      
      let camera;
      if (rendererName === 'Babylon.js') {
        // Use universal camera for exact position matching
        camera = renderer.createCamera('universal', {
          ...cameraOptions,
          target: { x: 0, y: 0, z: 0 }
        });
      } else if (rendererName === 'Torus') {
        // Torus custom camera
        camera = renderer.createCamera('perspective', {
          ...cameraOptions,
          lookAt: { x: 0, y: 0, z: 0 }
        });
      } else {
        // Perspective camera with orbit controls (matches arcRotate behavior)
        camera = renderer.createCamera('perspective', {
          ...cameraOptions,
          lookAt: { x: 0, y: 0, z: 0 }
        });
      }
      
      renderer.setActiveCamera(camera);
      
      // Create renderer-appropriate lighting
      if (rendererName === 'Babylon.js') {
        renderer.createLight('hemispheric', {
          direction: { x: 0, y: 1, z: 0 },
          intensity: 0.7,
          color: getDaisyUIColor('bc')
        });
      } else {
        // For Three.js, Torus, and other renderers, use ambient light
        renderer.createLight('ambient', {
          intensity: 0.4,
          color: getDaisyUIColor('bc')
        });
      }
      
      renderer.createLight('directional', {
        direction: { x: 0.5, y: -1, z: 0.5 },
        position: { x: 5, y: 10, z: 5 },
        intensity: 0.8,
        color: getDaisyUIColor('p')
      });
      
      // Create a unified demo scene that works with all renderers
      createUnifiedDemoScene(renderer);
      
      // Start render loop with animations
      const startTime = Date.now();
      renderer.startRenderLoop(() => {
        // Update stats if enabled
        if (statsRef) {
          statsRef.begin();
        }
        
        // Disabled spinning animations - objects should stay still for gizmo interaction
        
        // Render frame happens automatically in renderer
        
        if (statsRef) {
          statsRef.end();
        }
      });
      
      console.log('[ViewportCanvas] Scene setup complete');
    } catch (error) {
      console.error('[ViewportCanvas] Scene setup failed:', error);
    }
  };

  const createUnifiedDemoScene = (renderer) => {
    const rendererName = renderer.getRendererName();
    console.log(`[ViewportCanvas] Creating unified demo scene for ${rendererName}`);
    
    // Create objects with renderer-agnostic parameters
    const redBox = renderer.createPrimitive('box', {
      position: { x: -2, y: 1, z: 0 },
      width: 1,
      height: 1,
      depth: 1,
      // Enable smooth shading for Torus renderer
      ...(rendererName === 'Torus' ? { 
        smooth: true,
        color: { r: 1, g: 0.2, b: 0.2 },
        castShadow: true 
      } : {}),
      ...(rendererName === 'Three.js' ? { 
        color: { r: 1, g: 0.2, b: 0.2 },
        castShadow: true 
      } : {})
    });
    
    const greenSphere = renderer.createPrimitive('sphere', {
      position: { x: 0, y: 1, z: 0 },
      // Use appropriate radius/diameter for each renderer
      ...(rendererName === 'Babylon.js' ? { diameter: 1 } : { radius: 0.5 }),
      // Higher quality sphere for Torus renderer
      ...(rendererName === 'Torus' ? { 
        widthSegments: 48, 
        heightSegments: 32,
        color: { r: 0.2, g: 1, b: 0.2 },
        castShadow: true 
      } : {}),
      ...(rendererName === 'Three.js' ? { 
        color: { r: 0.2, g: 1, b: 0.2 },
        castShadow: true 
      } : {})
    });
    
    const blueCylinder = renderer.createPrimitive('cylinder', {
      position: { x: 2, y: 1, z: 0 },
      height: 1,
      ...(rendererName === 'Babylon.js' ? { diameter: 1 } : { radiusTop: 0.5, radiusBottom: 0.5 }),
      ...(rendererName === 'Three.js' || rendererName === 'Torus' ? { 
        color: { r: 0.2, g: 0.2, b: 1 },
        castShadow: true 
      } : {})
    });
    
    // Add our signature torus shape (only for Torus renderer)
    if (rendererName === 'Torus') {
      const yellowTorus = renderer.createPrimitive('torus', {
        position: { x: -4, y: 1, z: 0 },  // Moved to left side for easier testing
        majorRadius: 0.8,
        minorRadius: 0.25,
        majorSegments: 32,
        minorSegments: 20,
        color: { r: 1, g: 1, b: 0.2 }
      });
    }
    
    // Grid serves as ground reference - no separate ground plane needed
    
    // Handle materials for Babylon.js (Three.js already has colors applied)
    if (rendererName === 'Babylon.js' && renderer.createMaterial && renderer.applyMaterial) {
      const redMaterial = renderer.createMaterial('standard', {
        diffuseColor: { r: 1, g: 0.2, b: 0.2 }
      });
      
      const greenMaterial = renderer.createMaterial('standard', {
        diffuseColor: { r: 0.2, g: 1, b: 0.2 }
      });
      
      const blueMaterial = renderer.createMaterial('standard', {
        diffuseColor: { r: 0.2, g: 0.2, b: 1 }
      });
      
      // Apply materials to objects
      renderer.applyMaterial(redBox, redMaterial);
      renderer.applyMaterial(greenSphere, greenMaterial);
      renderer.applyMaterial(blueCylinder, blueMaterial);
    }
    
    // Create grid helper for both renderers
    const grid = renderer.createGrid({
      size: 10,
      divisions: 10,
      position: { x: 0, y: 0, z: 0 }
    });
    
    console.log(`[ViewportCanvas] Unified demo scene created for ${rendererName} with grid`);
  };

  const updateSceneColors = (renderer) => {
    const backgroundSetting = settings().viewport.backgroundColor;
    
    if (backgroundSetting === 'theme' || !backgroundSetting) {
      // Use DaisyUI base-100 color for background
      const bgColor = getDaisyUIColor('b1');
      renderer.setSceneBackground({ ...bgColor, a: 1.0 });
      console.log('[ViewportCanvas] Using DaisyUI theme background color');
    } else {
      // Parse custom color
      const rgb = parseColorToRgb(backgroundSetting);
      if (rgb) {
        renderer.setSceneBackground({ ...rgb, a: 1.0 });
      }
      console.log('[ViewportCanvas] Using custom background color:', backgroundSetting);
    }
  };

  // Watch for settings changes
  createEffect(() => {
    const renderer = renderContext.renderer();
    if (renderer) {
      updateSceneColors(renderer);
    }
  });

  // Watch for theme changes (only when using theme colors)
  createEffect(() => {
    const renderer = renderContext.renderer();
    const backgroundSetting = settings().viewport.backgroundColor;
    
    if (renderer && (backgroundSetting === 'theme' || !backgroundSetting)) {
      // Watch for theme changes on the document element
      const themeObserver = new MutationObserver((mutations) => {
        mutations.forEach((mutation) => {
          if (mutation.type === 'attributes' && mutation.attributeName === 'data-theme') {
            // Small delay to ensure CSS variables are updated
            setTimeout(() => {
              console.log('[ViewportCanvas] Theme changed, updating colors');
              updateSceneColors(renderer);
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

  // Stats setup
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
    >
      <canvas
        ref={el => {
          setCanvasRef(el);
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
      
      <Show when={renderContext.isLoading()}>
        <div class="absolute inset-0 flex items-center justify-center bg-base-100/80 backdrop-blur-sm">
          <div class="flex flex-col items-center gap-2">
            <span class="loading loading-spinner loading-lg text-primary"></span>
            <span class="text-sm text-base-content">Switching renderer...</span>
          </div>
        </div>
      </Show>
      
      <Show when={renderContext.error()}>
        <div class="absolute top-4 right-4 alert alert-error shadow-lg max-w-sm">
          <span class="text-sm">⚠️ {renderContext.error()}</span>
        </div>
      </Show>
    </div>
  );
}

function ViewportCanvas(props) {
  return <ViewportCanvasInner {...props} />;
}

export default ViewportCanvas;