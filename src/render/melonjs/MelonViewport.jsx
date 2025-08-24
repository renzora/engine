import { createSignal, createEffect, onMount, onCleanup } from 'solid-js';
import { editorStore } from '@/layout/stores/EditorStore';
import Stats from 'stats.js';

export const MelonViewport = (props) => {
  const [canvasRef, setCanvasRef] = createSignal(null);
  const [initialized, setInitialized] = createSignal(false);
  const [melonGame, setMelonGame] = createSignal(null);
  const [frameCount, setFrameCount] = createSignal(0);
  const [statsRef, setStatsRef] = createSignal(null);

  // Initialize MelonJS renderer
  const initializeRenderer = async () => {
    try {
      const canvas = canvasRef();
      if (!canvas) return;

      console.log('🍉 Initializing MelonJS renderer...');
      
      // Import MelonJS
      const me = await import('melonjs');
      
      // Initialize MelonJS engine first
      await me.boot();
      
      // Initialize MelonJS video system
      if (!me.video.init(canvas.clientWidth, canvas.clientHeight, {
        parent: canvas.parentElement,
        canvas: canvas,
        renderer: me.video.AUTO,
        preferWebGL1: false,
        scale: 'auto',
        scaleMethod: 'fill-min'
      })) {
        throw new Error('MelonJS video initialization failed');
      }
      
      // Create a demo scene with animated entities
      class DemoScene extends me.Stage {
        onResetEvent() {
          // Add background
          this.backgroundColor = '#1a1a26';
          
          // Create animated cubes
          for (let i = 0; i < 3; i++) {
            const cube = new AnimatedCube(200 + i * 150, 200, i);
            me.game.world.addChild(cube, 1);
          }
          
          // Create floating particles
          for (let i = 0; i < 15; i++) {
            const particle = new FloatingParticle(
              Math.random() * me.game.viewport.width,
              Math.random() * me.game.viewport.height
            );
            me.game.world.addChild(particle, 0);
          }
        }
        
        update(dt) {
          super.update(dt);
          setFrameCount(prev => prev + 1);
        }
      }
      
      // Animated cube entity
      class AnimatedCube extends me.Entity {
        constructor(x, y, index) {
          super(x, y, {
            width: 80,
            height: 80
          });
          this.index = index;
          this.time = 0;
          this.alwaysUpdate = true;
        }
        
        update(dt) {
          super.update(dt);
          this.time += dt / 1000;
          
          // Bounce animation
          this.pos.y = 200 + Math.sin(this.time * 2 + this.index) * 50;
          this.angle += 0.02;
          
          return true;
        }
        
        draw(renderer) {
          const color = me.pool.pull('me.Color');
          
          // Animated color
          const hue = (this.time + this.index) % (Math.PI * 2);
          color.setHSV(hue, 0.7, 0.8);
          
          renderer.setColor(color);
          renderer.fillRect(this.pos.x, this.pos.y, this.width, this.height);
          
          // Draw border
          renderer.setColor('#818cf8');
          renderer.strokeRect(this.pos.x, this.pos.y, this.width, this.height);
          
          me.pool.push(color);
        }
      }
      
      // Floating particle entity
      class FloatingParticle extends me.Entity {
        constructor(x, y) {
          super(x, y, {
            width: 6,
            height: 6
          });
          this.vx = (Math.random() - 0.5) * 2;
          this.vy = (Math.random() - 0.5) * 2;
          this.time = 0;
          this.alwaysUpdate = true;
        }
        
        update(dt) {
          super.update(dt);
          this.time += dt / 1000;
          
          this.pos.x += this.vx;
          this.pos.y += this.vy;
          
          // Wrap around screen
          if (this.pos.x < 0) this.pos.x = me.game.viewport.width;
          if (this.pos.x > me.game.viewport.width) this.pos.x = 0;
          if (this.pos.y < 0) this.pos.y = me.game.viewport.height;
          if (this.pos.y > me.game.viewport.height) this.pos.y = 0;
          
          return true;
        }
        
        draw(renderer) {
          const alpha = 0.3 + 0.3 * Math.sin(this.time * 4 + this.pos.x * 0.01);
          renderer.setGlobalAlpha(alpha);
          renderer.setColor('#ffffff');
          renderer.fillEllipse(this.pos.x, this.pos.y, this.width, this.height);
          renderer.setGlobalAlpha(1.0);
        }
      }
      
      // Set and initialize the demo scene
      me.state.set(me.state.PLAY, new DemoScene());
      me.state.change(me.state.PLAY);
      
      
      // Handle resize
      const handleResize = () => {
        me.video.updateDisplaySize(canvas.clientWidth, canvas.clientHeight);
        me.game.viewport.resize(canvas.clientWidth, canvas.clientHeight);
      };
      
      window.addEventListener('resize', handleResize);
      
      // Store globally for scene data extraction
      window.globalMelonGame = () => me;
      
      setMelonGame(me);
      setInitialized(true);
      
      onCleanup(() => {
        window.removeEventListener('resize', handleResize);
        me.video.renderer.flush();
      });

    } catch (error) {
      console.error('🍉 Failed to initialize MelonJS:', error);
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
    const game = melonGame();
    if (game && game.video) {
      game.video.renderer.flush();
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
        🍉 MelonJS {initialized() ? `(Frame ${frameCount()})` : '(Loading...)'}
      </div>
    </div>
  );
};