import { createSignal, createEffect, onMount, onCleanup } from 'solid-js';
import { editorStore } from '@/layout/stores/EditorStore';
import Stats from 'stats.js';

export const ThreeViewport = (props) => {
  const [canvasRef, setCanvasRef] = createSignal(null);
  const [initialized, setInitialized] = createSignal(false);
  const [threeScene, setThreeScene] = createSignal(null);
  const [threeRenderer, setThreeRenderer] = createSignal(null);
  const [threeCamera, setThreeCamera] = createSignal(null);
  const [frameCount, setFrameCount] = createSignal(0);
  const [statsRef, setStatsRef] = createSignal(null);

  // Initialize Three.js renderer
  const initializeRenderer = async () => {
    try {
      const canvas = canvasRef();
      if (!canvas) return;

      console.log('🎮 Initializing Three.js renderer...');
      
      // Import Three.js
      const THREE = await import('three');
      
      // Create scene
      const scene = new THREE.Scene();
      scene.background = new THREE.Color(0x1a1a26);
      
      // Create camera
      const camera = new THREE.PerspectiveCamera(
        75, 
        canvas.clientWidth / canvas.clientHeight, 
        0.1, 
        1000
      );
      camera.position.set(0, 2, 5);
      
      // Create renderer
      const renderer = new THREE.WebGLRenderer({ 
        canvas: canvas,
        antialias: true,
        alpha: true
      });
      renderer.setSize(canvas.clientWidth, canvas.clientHeight);
      renderer.setPixelRatio(window.devicePixelRatio);
      renderer.shadowMap.enabled = true;
      renderer.shadowMap.type = THREE.PCFSoftShadowMap;
      
      // Add lighting
      const ambientLight = new THREE.AmbientLight(0x404040, 0.6);
      scene.add(ambientLight);
      
      const directionalLight = new THREE.DirectionalLight(0xffffff, 0.8);
      directionalLight.position.set(10, 10, 5);
      directionalLight.castShadow = true;
      directionalLight.shadow.mapSize.width = 2048;
      directionalLight.shadow.mapSize.height = 2048;
      scene.add(directionalLight);
      
      // Add default cube
      const geometry = new THREE.BoxGeometry(1, 1, 1);
      const material = new THREE.MeshPhongMaterial({ 
        color: 0xff6b6b,
        shininess: 100
      });
      const cube = new THREE.Mesh(geometry, material);
      cube.castShadow = true;
      cube.receiveShadow = true;
      scene.add(cube);
      
      // Add ground plane
      const planeGeometry = new THREE.PlaneGeometry(20, 20);
      const planeMaterial = new THREE.MeshPhongMaterial({ color: 0x808080 });
      const plane = new THREE.Mesh(planeGeometry, planeMaterial);
      plane.rotation.x = -Math.PI / 2;
      plane.position.y = -2;
      plane.receiveShadow = true;
      scene.add(plane);
      
      // Store globally for scene data extraction
      window.globalThreeScene = () => scene;
      window.globalThreeCamera = () => camera;
      window.globalThreeRenderer = () => renderer;
      
      
      // Animation loop
      const animate = () => {
        const frameId = requestAnimationFrame(animate);
        
        if (statsRef()) statsRef().begin();
        
        // Rotate cube
        cube.rotation.x += 0.01;
        cube.rotation.y += 0.01;
        
        // Bounce cube
        cube.position.y = Math.sin(Date.now() * 0.002) * 0.5;
        
        // Orbit camera
        const time = Date.now() * 0.001;
        camera.position.x = Math.cos(time * 0.5) * 5;
        camera.position.z = Math.sin(time * 0.5) * 5;
        camera.lookAt(0, 0, 0);
        
        renderer.render(scene, camera);
        setFrameCount(prev => prev + 1);
        
        if (statsRef()) statsRef().end();
        
        return frameId;
      };
      
      const animationId = animate();
      
      setThreeScene(scene);
      setThreeRenderer(renderer);
      setThreeCamera(camera);
      setInitialized(true);
      
      // Handle window resize
      const handleResize = () => {
        const newWidth = canvas.clientWidth;
        const newHeight = canvas.clientHeight;
        
        camera.aspect = newWidth / newHeight;
        camera.updateProjectionMatrix();
        renderer.setSize(newWidth, newHeight);
      };
      
      window.addEventListener('resize', handleResize);
      
      onCleanup(() => {
        cancelAnimationFrame(animationId);
        window.removeEventListener('resize', handleResize);
      });

    } catch (error) {
      console.error('🎮 Failed to initialize Three.js:', error);
    }
  };

  // Handle canvas mounting
  onMount(() => {
    const canvas = canvasRef();
    if (canvas) {
      initializeRenderer();
    }
  });

  // Cleanup on unmount
  onCleanup(() => {
    const renderer = threeRenderer();
    
    if (renderer) {
      renderer.dispose();
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
        🎮 Three.js {initialized() ? `(Frame ${frameCount()})` : '(Loading...)'}
      </div>
    </div>
  );
};