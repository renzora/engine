import { createSignal, createEffect, onMount, onCleanup } from 'solid-js';
import { editorStore } from '@/layout/stores/EditorStore';
import Stats from 'stats.js';

export const PhaserViewport = (props) => {
  const [canvasRef, setCanvasRef] = createSignal(null);
  const [initialized, setInitialized] = createSignal(false);
  const [phaserGame, setPhaserGame] = createSignal(null);
  const [frameCount, setFrameCount] = createSignal(0);
  const [statsRef, setStatsRef] = createSignal(null);

  // Initialize Phaser renderer
  const initializeRenderer = async () => {
    try {
      const canvas = canvasRef();
      if (!canvas) return;

      console.log('🎮 Initializing Phaser renderer...');
      
      // Import Phaser
      const Phaser = await import('phaser');
      
      // Create main scene with animated content
      class DemoScene extends Phaser.Scene {
        constructor() {
          super({ key: 'DemoScene' });
          this.cubes = [];
          this.particles = [];
          this.time = 0;
        }
        
        selectObject(obj, name) {
          // Deselect all other objects
          this.cubes.forEach(c => {
            c.isSelected = false;
            if (c.objectType === '2d') {
              c.setStrokeStyle(2, 0x818cf8);
            } else if (c.objectType === '3d') {
              c.lineStyle(2, 0x818cf8);
            }
          });
          
          // Select this object
          obj.isSelected = true;
          if (obj.objectType === '2d') {
            obj.setStrokeStyle(4, 0xfbbf24);
          } else if (obj.objectType === '3d') {
            obj.lineStyle(4, 0xfbbf24);
          }
          console.log(`🎮 Selected: ${name}`);
        }
        
        createCube3D(x, y, size, index) {
          // Create a container for the 3D cube
          const container = this.add.container(x, y);
          
          // Create graphics object for drawing the cube faces
          const graphics = this.add.graphics();
          container.add(graphics);
          
          // Store properties
          container.cubeSize = size;
          container.cubeIndex = index;
          container.rotationX = 0;
          container.rotationY = 0;
          container.isSelected = false;
          container.objectType = '3d';
          container.graphics = graphics;
          
          // Make interactive
          const hitArea = new Phaser.Geom.Rectangle(-size/2, -size/2, size, size);
          container.setInteractive(hitArea, Phaser.Geom.Rectangle.Contains);
          
          // Add click handler
          container.on('pointerdown', () => {
            this.selectObject(container, `3D Cube ${index}`);
          });
          
          // Draw the 3D cube
          this.drawCube3D(container);
          
          return container;
        }
        
        drawCube3D(container) {
          const graphics = container.graphics;
          const size = container.cubeSize;
          const rotX = container.rotationX;
          const rotY = container.rotationY;
          
          graphics.clear();
          
          // Calculate 3D projection (simple isometric)
          const cos = Math.cos(rotY);
          const sin = Math.sin(rotY);
          const depth = size * 0.5;
          
          // Cube faces with different shades
          const frontColor = 0x4f46e5;
          const rightColor = 0x3730a3;
          const topColor = 0x6366f1;
          
          // Set line style based on selection
          if (container.isSelected) {
            graphics.lineStyle(4, 0xfbbf24);
          } else {
            graphics.lineStyle(2, 0x818cf8);
          }
          
          // Draw front face
          graphics.fillStyle(frontColor);
          graphics.fillRect(-size/2, -size/2, size, size);
          graphics.strokeRect(-size/2, -size/2, size, size);
          
          // Draw right face (isometric projection)
          graphics.fillStyle(rightColor);
          graphics.beginPath();
          graphics.moveTo(size/2, -size/2);
          graphics.lineTo(size/2 + depth * cos, -size/2 - depth * sin);
          graphics.lineTo(size/2 + depth * cos, size/2 - depth * sin);
          graphics.lineTo(size/2, size/2);
          graphics.closePath();
          graphics.fillPath();
          graphics.strokePath();
          
          // Draw top face
          graphics.fillStyle(topColor);
          graphics.beginPath();
          graphics.moveTo(-size/2, -size/2);
          graphics.lineTo(-size/2 + depth * cos, -size/2 - depth * sin);
          graphics.lineTo(size/2 + depth * cos, -size/2 - depth * sin);
          graphics.lineTo(size/2, -size/2);
          graphics.closePath();
          graphics.fillPath();
          graphics.strokePath();
        }
        
        create() {
          console.log('🎮 Phaser: Creating scene objects...');
          
          // Create simple 2D rectangles first
          for (let i = 0; i < 3; i++) {
            const rect = this.add.rectangle(
              150 + i * 120, 
              200, 
              80, 
              80, 
              0x4f46e5
            );
            rect.setStrokeStyle(2, 0x818cf8);
            rect.setInteractive();
            rect.isSelected = false;
            rect.objectType = '2d';
            rect.objectIndex = i;
            
            // Ensure visibility
            rect.setVisible(true);
            rect.setAlpha(1.0);
            rect.setDepth(1);
            
            // Add click handler for selection
            rect.on('pointerdown', () => {
              this.selectObject(rect, `2D Rectangle ${i}`);
            });
            
            this.cubes.push(rect);
            console.log(`🎮 Created 2D rectangle ${i} at`, rect.x, rect.y);
            
            // Add debug text near each rectangle
            this.add.text(rect.x - 30, rect.y - 50, `R${i}`, {
              fontSize: '12px',
              fill: '#ffffff'
            });
          }
          
          // Create simple graphics-based cubes
          for (let i = 0; i < 2; i++) {
            const graphics = this.add.graphics();
            graphics.x = 500 + i * 120;
            graphics.y = 200;
            graphics.isSelected = false;
            graphics.objectType = '3d';
            graphics.objectIndex = i;
            
            // Draw a simple 3D-looking cube using correct API
            graphics.clear();
            
            // Front face
            graphics.fillStyle(0x6366f1);
            graphics.fillRect(-40, -40, 80, 80);
            
            // Stroke for front face
            graphics.lineStyle(2, 0x818cf8);
            graphics.strokeRect(-40, -40, 80, 80);
            
            // Right face (isometric)
            graphics.fillStyle(0x3730a3);
            graphics.beginPath();
            graphics.moveTo(40, -40);
            graphics.lineTo(60, -60);
            graphics.lineTo(60, 20);
            graphics.lineTo(40, 40);
            graphics.closePath();
            graphics.fillPath();
            graphics.strokePath();
            
            // Top face
            graphics.fillStyle(0x8b5cf6);
            graphics.beginPath();
            graphics.moveTo(-40, -40);
            graphics.lineTo(-20, -60);
            graphics.lineTo(60, -60);
            graphics.lineTo(40, -40);
            graphics.closePath();
            graphics.fillPath();
            graphics.strokePath();
            
            // Ensure visibility
            graphics.setVisible(true);
            graphics.setAlpha(1.0);
            graphics.setDepth(2);
            
            graphics.setInteractive(new Phaser.Geom.Rectangle(-40, -40, 80, 80), Phaser.Geom.Rectangle.Contains);
            
            graphics.on('pointerdown', () => {
              this.selectObject(graphics, `3D Cube ${i}`);
            });
            
            this.cubes.push(graphics);
            console.log(`🎮 Created 3D cube ${i} at`, graphics.x, graphics.y);
            
            // Add debug text near each cube
            this.add.text(graphics.x - 30, graphics.y - 70, `C${i}`, {
              fontSize: '12px',
              fill: '#ffffff'
            });
          }
          
          // Create floating particles
          for (let i = 0; i < 15; i++) {
            const particle = this.add.circle(
              Math.random() * this.cameras.main.width,
              Math.random() * this.cameras.main.height,
              3,
              0xffffff,
              0.6
            );
            particle.vx = (Math.random() - 0.5) * 2;
            particle.vy = (Math.random() - 0.5) * 2;
            this.particles.push(particle);
          }
          
          // Reset camera to ensure objects are visible
          this.cameras.main.setZoom(1);
          this.cameras.main.centerOn(400, 250);
          
          // Add text
          this.add.text(20, 20, 'Phaser: 2D Rectangles (left) + Pseudo-3D Cubes (right)', {
            fontSize: '14px',
            fill: '#ffffff',
            fontFamily: 'monospace'
          });
          
          this.add.text(20, 40, 'Click objects to select • Drag to pan • Scroll to zoom', {
            fontSize: '12px',
            fill: '#a5b4fc',
            fontFamily: 'monospace'
          });
          
          console.log('🎮 Camera centered at:', this.cameras.main.midPoint);
          console.log('🎮 Camera zoom:', this.cameras.main.zoom);
          console.log('🎮 Scene size:', this.cameras.main.width, 'x', this.cameras.main.height);
          
          // Add camera controls (pan)
          this.cameras.main.setLerp(0.1);
          this.input.on('pointermove', (pointer) => {
            if (pointer.isDown) {
              this.cameras.main.scrollX -= (pointer.x - pointer.prevPosition.x) / this.cameras.main.zoom;
              this.cameras.main.scrollY -= (pointer.y - pointer.prevPosition.y) / this.cameras.main.zoom;
            }
          });
          
          // Zoom controls
          this.input.on('wheel', (pointer, gameObjects, deltaX, deltaY) => {
            if (deltaY > 0) {
              this.cameras.main.zoom = Math.max(0.5, this.cameras.main.zoom - 0.1);
            } else {
              this.cameras.main.zoom = Math.min(3.0, this.cameras.main.zoom + 0.1);
            }
          });
        }
        
        update() {
          this.time += 0.016;
          
          // Animate objects
          this.cubes.forEach((obj, index) => {
            if (obj.objectType === '2d') {
              // 2D rectangle animation
              const baseY = 200;
              obj.y = baseY + Math.sin(this.time * 2 + index) * 30;
              obj.rotation += 0.02;
              
              // Color animation
              const hue = (this.time + index) % (Math.PI * 2);
              const r = Math.floor((0.3 + 0.3 * Math.sin(hue)) * 255);
              const g = Math.floor((0.3 + 0.3 * Math.sin(hue + 2)) * 255);
              const b = Math.floor((0.7 + 0.3 * Math.sin(hue + 4)) * 255);
              obj.fillColor = (r << 16) | (g << 8) | b;
            } else if (obj.objectType === '3d') {
              // 3D cube animation - simple bounce
              const baseY = 200;
              obj.y = baseY + Math.sin(this.time * 1.5 + index) * 40;
              obj.rotation += 0.01;
            }
          });
          
          // Animate particles
          this.particles.forEach(particle => {
            particle.x += particle.vx;
            particle.y += particle.vy;
            
            // Wrap around screen
            if (particle.x < 0) particle.x = this.cameras.main.width;
            if (particle.x > this.cameras.main.width) particle.x = 0;
            if (particle.y < 0) particle.y = this.cameras.main.height;
            if (particle.y > this.cameras.main.height) particle.y = 0;
            
            // Pulse alpha
            particle.alpha = 0.3 + 0.3 * Math.sin(this.time * 4 + particle.x * 0.01);
          });
          
          setFrameCount(prev => prev + 1);
        }
      }
      
      // Create Phaser game instance
      const game = new Phaser.Game({
        type: Phaser.WEBGL,
        canvas: canvas,
        width: canvas.clientWidth,
        height: canvas.clientHeight,
        backgroundColor: '#1a1a26',
        scene: DemoScene,
        scale: {
          mode: Phaser.Scale.RESIZE,
          width: canvas.clientWidth,
          height: canvas.clientHeight,
          autoCenter: Phaser.Scale.NO_CENTER
        },
        render: {
          antialias: true,
          pixelArt: false
        }
      });
      
      // Handle resize
      const handleResize = () => {
        game.scale.resize(canvas.clientWidth, canvas.clientHeight);
      };
      
      window.addEventListener('resize', handleResize);
      
      
      // Store globally for scene data extraction
      window.globalPhaserGame = () => game;
      
      setPhaserGame(game);
      setInitialized(true);
      
      onCleanup(() => {
        window.removeEventListener('resize', handleResize);
        game.destroy(true);
      });

    } catch (error) {
      console.error('🎮 Failed to initialize Phaser:', error);
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
    const game = phaserGame();
    if (game) {
      game.destroy(true);
    }
  });

  return (
    <div style={props.style} class="relative w-full h-full">
      <canvas
        ref={setCanvasRef}
        style={{
          width: '100%',
          height: '100%',
          display: 'block',
          margin: '0',
          padding: '0',
          border: 'none',
          outline: 'none'
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
        🎮 Phaser {initialized() ? `(Frame ${frameCount()})` : '(Loading...)'}
      </div>
    </div>
  );
};