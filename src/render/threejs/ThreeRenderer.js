// Three.js Renderer Implementation

export default class ThreeRenderer {
  constructor(config) {
    this.config = config;
    this.scene = null;
    this.camera = null;
    this.renderer = null;
    this.initialized = false;
  }

  async initialize(canvas) {
    try {
      const THREE = await import('three');
      
      // Create scene
      this.scene = new THREE.Scene();
      this.scene.background = new THREE.Color(0x1a1a26);
      
      // Create camera
      this.camera = new THREE.PerspectiveCamera(
        75, 
        canvas.clientWidth / canvas.clientHeight, 
        0.1, 
        1000
      );
      this.camera.position.set(0, 2, 5);
      
      // Create renderer
      this.renderer = new THREE.WebGLRenderer({ 
        canvas: canvas,
        antialias: true,
        alpha: true
      });
      this.renderer.setSize(canvas.clientWidth, canvas.clientHeight);
      this.renderer.setPixelRatio(window.devicePixelRatio);
      this.renderer.shadowMap.enabled = true;
      this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
      
      this.initialized = true;
      return true;
    } catch (error) {
      console.error('Three.js initialization failed:', error);
      return false;
    }
  }

  render() {
    if (this.initialized && this.scene && this.camera && this.renderer) {
      this.renderer.render(this.scene, this.camera);
      return true;
    }
    return false;
  }

  resize(width, height) {
    if (this.camera && this.renderer) {
      this.camera.aspect = width / height;
      this.camera.updateProjectionMatrix();
      this.renderer.setSize(width, height);
    }
  }

  dispose() {
    if (this.renderer) {
      this.renderer.dispose();
    }
  }

  getScene() {
    return this.scene;
  }

  getCamera() {
    return this.camera;
  }

  getRenderer() {
    return this.renderer;
  }
}