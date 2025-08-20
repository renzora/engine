/**
 * WebGL Context Manager
 * Handles WebGL initialization, state management, and canvas sizing
 */
export class WebGLContext {
  constructor(canvas, options = {}) {
    this.canvas = canvas;
    this.options = options;
    this.gl = null;
    this.handleResize = null;
  }

  async initialize() {
    // Initialize WebGL context
    this.gl = this.canvas.getContext('webgl2', {
      antialias: this.options.antialias !== false,
      alpha: this.options.alpha !== false,
      depth: true,
      stencil: false,
      powerPreference: this.options.powerPreference || 'high-performance'
    });

    if (!this.gl) {
      throw new Error('WebGL2 not supported');
    }

    console.log('[Torus WebGL] Context created');

    // Set up WebGL state
    this.setupWebGLState();
    
    // Set up resize handling
    this.handleResize = () => this.resizeCanvas();
    window.addEventListener('resize', this.handleResize);
    
    return this.gl;
  }

  setupWebGLState() {
    const gl = this.gl;
    
    // Fix canvas resolution to match display size
    this.resizeCanvas();
    
    // Enable depth testing
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LEQUAL);
    
    // Enable backface culling
    gl.enable(gl.CULL_FACE);
    gl.cullFace(gl.BACK);
    
    // Set default clear color
    gl.clearColor(0.1, 0.1, 0.1, 1.0);
    
    console.log('[Torus WebGL] State configured');
  }

  resizeCanvas() {
    const canvas = this.canvas;
    const gl = this.gl;
    
    // Get the display size (CSS pixels)
    const displayWidth = canvas.clientWidth;
    const displayHeight = canvas.clientHeight;
    
    // Get device pixel ratio for crisp rendering
    const devicePixelRatio = window.devicePixelRatio || 1;
    
    // Calculate actual canvas size
    const canvasWidth = Math.floor(displayWidth * devicePixelRatio);
    const canvasHeight = Math.floor(displayHeight * devicePixelRatio);
    
    // Resize canvas buffer if needed
    if (canvas.width !== canvasWidth || canvas.height !== canvasHeight) {
      canvas.width = canvasWidth;
      canvas.height = canvasHeight;
      
      // Update viewport
      if (gl) {
        gl.viewport(0, 0, canvasWidth, canvasHeight);
      }
      
      console.log(`[Torus WebGL] Canvas resized to ${canvasWidth}x${canvasHeight} (display: ${displayWidth}x${displayHeight}, ratio: ${devicePixelRatio})`);
      
      // Notify listeners
      this.onResize?.(canvasWidth, canvasHeight);
    }
  }

  resize(width, height) {
    this.resizeCanvas();
  }

  clear() {
    if (this.gl) {
      this.gl.clear(this.gl.COLOR_BUFFER_BIT | this.gl.DEPTH_BUFFER_BIT);
    }
  }

  setBackgroundColor(color) {
    if (this.gl) {
      this.gl.clearColor(color.r, color.g, color.b, color.a || 1.0);
    }
  }

  isReady() {
    return !!this.gl;
  }

  getContext() {
    return this.gl;
  }

  getCanvasSize() {
    return {
      width: this.canvas.width,
      height: this.canvas.height,
      displayWidth: this.canvas.clientWidth,
      displayHeight: this.canvas.clientHeight
    };
  }

  getCapabilities() {
    const gl = this.gl;
    if (!gl) return {};

    return {
      webgl: true,
      webgl2: gl instanceof WebGL2RenderingContext,
      webgpu: false,
      maxTextureSize: gl.getParameter(gl.MAX_TEXTURE_SIZE),
      maxLights: 16,
      supportsInstancing: true,
      supportsPhysics: false,
      supportsPostProcessing: true,
      custom: true
    };
  }

  async dispose() {
    // Remove resize listener
    if (this.handleResize) {
      window.removeEventListener('resize', this.handleResize);
      this.handleResize = null;
    }
    
    // WebGL context will be cleaned up by browser
    this.gl = null;
    
    console.log('[Torus WebGL] Disposed');
  }
}