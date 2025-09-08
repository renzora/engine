/**
 * RuntimeRenderer - Minimal Babylon.js renderer for exported projects
 * Contains only essential rendering functionality
 */
export class RuntimeRenderer {
  constructor(canvas) {
    this.canvas = canvas;
    this.engine = null;
    this.scene = null;
    this.camera = null;
    this.isRunning = false;
  }

  /**
   * Initialize the renderer
   */
  async initialize() {
    try {
      // Initialize Babylon.js engine and scene
      
      // Import Babylon.js modules
      const { Engine } = await import('@babylonjs/core/Engines/engine.js');
      const { Scene } = await import('@babylonjs/core/scene.js');
      const { ArcRotateCamera } = await import('@babylonjs/core/Cameras/arcRotateCamera.js');
      const { Vector3 } = await import('@babylonjs/core/Maths/math.vector.js');
      const { HemisphericLight } = await import('@babylonjs/core/Lights/hemisphericLight.js');
      
      // Create engine
      this.engine = new Engine(this.canvas, true, {
        preserveDrawingBuffer: true,
        stencil: true,
        disableWebGL2Support: false
      });
      
      // Create scene
      this.scene = new Scene(this.engine);
      
      // Set up default camera
      this.camera = new ArcRotateCamera(
        'camera', 
        -Math.PI / 2, 
        Math.PI / 2.5, 
        10, 
        Vector3.Zero(), 
        this.scene
      );
      
      this.camera.attachControls(this.canvas, true);
      
      // Add default lighting
      const light = new HemisphericLight('light', new Vector3(0, 1, 0), this.scene);
      light.intensity = 0.7;
      
      // Handle window resize
      window.addEventListener('resize', () => {
        this.engine.resize();
      });
      
      // Renderer initialization complete
      
    } catch (error) {
      console.error('❌ RuntimeRenderer: Initialization failed:', error);
      throw error;
    }
  }

  /**
   * Start the render loop
   */
  start() {
    if (this.isRunning) return;
    
    // Start rendering loop
    this.isRunning = true;
    
    this.engine.runRenderLoop(() => {
      if (this.scene) {
        this.scene.render();
      }
    });
  }

  /**
   * Stop the render loop
   */
  stop() {
    if (!this.isRunning) return;
    
    // Stop rendering loop
    this.isRunning = false;
    this.engine.stopRenderLoop();
  }

  /**
   * Dispose of renderer resources
   */
  dispose() {
    // Clean up renderer resources
    
    this.stop();
    
    if (this.scene) {
      this.scene.dispose();
      this.scene = null;
    }
    
    if (this.engine) {
      this.engine.dispose();
      this.engine = null;
    }
    
    this.camera = null;
    this.canvas = null;
  }

  /**
   * Get renderer statistics
   */
  getStats() {
    if (!this.scene || !this.engine) {
      return { fps: 0, meshCount: 0, textureCount: 0 };
    }
    
    return {
      fps: this.engine.getFps(),
      meshCount: this.scene.meshes.length,
      textureCount: this.scene.textures.length,
      lightCount: this.scene.lights.length,
      isRunning: this.isRunning
    };
  }
}