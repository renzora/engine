import { createSignal, createEffect, onMount, onCleanup } from 'solid-js';
import { editorStore } from '@/layout/stores/EditorStore';
import Stats from 'stats.js';

export const PixiViewport = (props) => {
  const [canvasRef, setCanvasRef] = createSignal(null);
  const [initialized, setInitialized] = createSignal(false);
  const [pixiApp, setPixiApp] = createSignal(null);
  const [frameCount, setFrameCount] = createSignal(0);
  const [statsRef, setStatsRef] = createSignal(null);

  // Initialize PixiJS renderer
  const initializeRenderer = async () => {
    try {
      const canvas = canvasRef();
      if (!canvas) return;

      console.log('🎨 Initializing PixiJS renderer...');
      
      // Import PixiJS
      const PIXI = await import('pixi.js');
      
      // Create PixiJS application
      const app = new PIXI.Application();
      await app.init({
        canvas: canvas,
        width: canvas.clientWidth,
        height: canvas.clientHeight,
        backgroundColor: 0x1a1a26,
        antialias: true,
        resolution: window.devicePixelRatio,
        autoDensity: true
      });
      
      // Create container for 3D-like scene
      const sceneContainer = new PIXI.Container();
      app.stage.addChild(sceneContainer);
      
      // Create animated sprites (cube representations)
      const cubes = [];
      for (let i = 0; i < 3; i++) {
        const graphics = new PIXI.Graphics();
        graphics.x = 200 + i * 150;
        graphics.y = 200;
        graphics.interactive = true;
        graphics.buttonMode = true;
        graphics.isSelected = false;
        graphics.cubeIndex = i;
        
        // Add click handler for selection
        graphics.on('pointerdown', () => {
          // Deselect all other cubes
          cubes.forEach(cube => {
            cube.isSelected = false;
          });
          
          // Select this cube
          graphics.isSelected = true;
          console.log(`🎨 Selected PixiJS cube ${i}`);
        });
        
        cubes.push(graphics);
        sceneContainer.addChild(graphics);
      }
      
      // Helper function to convert RGB to hex
      const rgb2hex = (rgb) => {
        const r = Math.floor(rgb[0] * 255);
        const g = Math.floor(rgb[1] * 255);
        const b = Math.floor(rgb[2] * 255);
        return (r << 16) | (g << 8) | b;
      };

      // Draw a cube-like shape with gradients
      const drawCube = (g, size, hue, isSelected) => {
        g.clear();
        
        const x = 0;
        const y = 0;
        
        // Selection outline
        if (isSelected) {
          g.lineStyle(4, 0xfbbf24, 1);
        } else {
          g.lineStyle(2, 0x818cf8, 1);
        }
        
        // Main face (front)
        g.beginFill(rgb2hex([
          0.8 + 0.2 * Math.sin(hue),
          0.4 + 0.4 * Math.sin(hue + 2),
          0.4 + 0.4 * Math.sin(hue + 4)
        ]));
        g.drawRect(x - size/2, y - size/2, size, size);
        g.endFill();
        
        // Right face (side)
        g.beginFill(rgb2hex([
          0.6 + 0.2 * Math.sin(hue),
          0.3 + 0.3 * Math.sin(hue + 2),
          0.3 + 0.3 * Math.sin(hue + 4)
        ]));
        g.moveTo(x + size/2, y - size/2);
        g.lineTo(x + size/2 + size/4, y - size/2 - size/4);
        g.lineTo(x + size/2 + size/4, y + size/2 - size/4);
        g.lineTo(x + size/2, y + size/2);
        g.closePath();
        g.endFill();
        
        // Top face
        g.beginFill(rgb2hex([
          0.9 + 0.1 * Math.sin(hue),
          0.5 + 0.3 * Math.sin(hue + 2),
          0.5 + 0.3 * Math.sin(hue + 4)
        ]));
        g.moveTo(x - size/2, y - size/2);
        g.lineTo(x - size/2 + size/4, y - size/2 - size/4);
        g.lineTo(x + size/2 + size/4, y - size/2 - size/4);
        g.lineTo(x + size/2, y - size/2);
        g.closePath();
        g.endFill();
      };
      
      sceneContainer.addChild(graphics);
      
      // Add floating particles
      const particles = [];
      for (let i = 0; i < 20; i++) {
        const particle = new PIXI.Graphics();
        particle.beginFill(0xffffff, 0.6);
        particle.drawCircle(0, 0, 2);
        particle.endFill();
        particle.x = Math.random() * app.canvas.width;
        particle.y = Math.random() * app.canvas.height;
        particle.vx = (Math.random() - 0.5) * 2;
        particle.vy = (Math.random() - 0.5) * 2;
        particles.push(particle);
        sceneContainer.addChild(particle);
      }
      
      // Animation variables
      let time = 0;
      let cubeSize = 80;
      
      // Animation loop
      const animate = () => {
        time += 0.016; // ~60fps
        
        // Animate cubes
        cubes.forEach((cube, index) => {
          const bounceY = Math.sin(time * 2 + index) * 50;
          cube.y = 200 + bounceY;
          cube.rotation += 0.02;
          
          const hue = time * 2 + index;
          drawCube(cube, 80, hue, cube.isSelected);
        });
        
        // Animate particles
        particles.forEach(particle => {
          particle.x += particle.vx;
          particle.y += particle.vy;
          
          // Wrap around screen
          if (particle.x < 0) particle.x = app.canvas.width;
          if (particle.x > app.canvas.width) particle.x = 0;
          if (particle.y < 0) particle.y = app.canvas.height;
          if (particle.y > app.canvas.height) particle.y = 0;
          
          // Pulse alpha
          particle.alpha = 0.3 + 0.3 * Math.sin(time * 4 + particle.x * 0.01);
        });
        
        setFrameCount(prev => prev + 1);
      };
      
      // Add camera pan controls
      let isDragging = false;
      let lastPointer = { x: 0, y: 0 };
      
      app.stage.interactive = true;
      app.stage.hitArea = app.screen;
      
      app.stage.on('pointerdown', (event) => {
        isDragging = true;
        lastPointer = { x: event.global.x, y: event.global.y };
      });
      
      app.stage.on('pointermove', (event) => {
        if (isDragging) {
          const dx = event.global.x - lastPointer.x;
          const dy = event.global.y - lastPointer.y;
          
          sceneContainer.x += dx;
          sceneContainer.y += dy;
          
          lastPointer = { x: event.global.x, y: event.global.y };
        }
      });
      
      app.stage.on('pointerup', () => {
        isDragging = false;
      });
      
      app.stage.on('pointerupoutside', () => {
        isDragging = false;
      });
      
      // Zoom controls
      app.stage.on('wheel', (event) => {
        const scaleFactor = event.deltaY > 0 ? 0.9 : 1.1;
        sceneContainer.scale.x = Math.max(0.5, Math.min(3.0, sceneContainer.scale.x * scaleFactor));
        sceneContainer.scale.y = Math.max(0.5, Math.min(3.0, sceneContainer.scale.y * scaleFactor));
      });
      
      // Start ticker
      app.ticker.add(animate);
      
      // Handle resize
      const handleResize = () => {
        app.renderer.resize(canvas.clientWidth, canvas.clientHeight);
      };
      
      window.addEventListener('resize', handleResize);
      
      // Store globally for scene data extraction
      window.globalPixiApp = () => app;
      
      setPixiApp(app);
      setInitialized(true);
      
      onCleanup(() => {
        window.removeEventListener('resize', handleResize);
        app.ticker.remove(animate);
      });

    } catch (error) {
      console.error('🎨 Failed to initialize PixiJS:', error);
    }
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
    const app = pixiApp();
    if (app) {
      app.destroy(true);
    }
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
        🎨 PixiJS {initialized() ? `(Frame ${frameCount()})` : '(Loading...)'}
      </div>
    </div>
  );
};