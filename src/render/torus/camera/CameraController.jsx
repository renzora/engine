/**
 * Camera Controller - Handles orbit, pan, and zoom controls for Torus renderer
 */
export class CameraController {
  constructor(canvas, sceneManager) {
    this.canvas = canvas;
    this.sceneManager = sceneManager;
    
    // Get initial camera state from scene manager
    const initialPos = sceneManager.cameraPosition || [0, 5, 10];
    const initialTarget = sceneManager.cameraTarget || [0, 0, 0];
    
    // Calculate initial spherical coordinates from current position
    const dx = initialPos[0] - initialTarget[0];
    const dy = initialPos[1] - initialTarget[1];
    const dz = initialPos[2] - initialTarget[2];
    
    this.distance = Math.sqrt(dx*dx + dy*dy + dz*dz);
    this.azimuth = Math.atan2(dz, dx);
    this.elevation = Math.asin(dy / this.distance);
    this.target = [...initialTarget];
    this.position = [...initialPos];
    
    // Control state
    this.isMouseDown = false;
    this.mouseButton = -1;
    this.lastMouseX = 0;
    this.lastMouseY = 0;
    
    // Control sensitivity
    this.orbitSpeed = 0.005;
    this.panSpeed = 0.003;  // Reduced from 0.01 to 0.003
    this.zoomSpeed = 0.1;
    this.minDistance = 2;
    this.maxDistance = 50;
    
    // Don't setup event listeners here - will be handled by renderer
    this.updateCameraPosition();
  }
  
  setupEventListeners() {
    // Mouse controls
    this.canvas.addEventListener('mousedown', (e) => this.onMouseDown(e));
    this.canvas.addEventListener('mousemove', (e) => this.onMouseMove(e));
    this.canvas.addEventListener('mouseup', (e) => this.onMouseUp(e));
    this.canvas.addEventListener('wheel', (e) => this.onWheel(e));
    
    // Prevent context menu
    this.canvas.addEventListener('contextmenu', (e) => e.preventDefault());
    
    // Make canvas focusable for keyboard events
    this.canvas.setAttribute('tabindex', '0');
  }
  
  onMouseDown(event) {
    this.isMouseDown = true;
    this.mouseButton = event.button;
    this.lastMouseX = event.clientX;
    this.lastMouseY = event.clientY;
    
    // Focus canvas for keyboard events
    this.canvas.focus();
    
    event.preventDefault();
  }
  
  onMouseMove(event) {
    if (!this.isMouseDown) return;
    
    const deltaX = event.clientX - this.lastMouseX;
    const deltaY = event.clientY - this.lastMouseY;
    
    if (this.mouseButton === 0) {
      // Left mouse button - orbit
      this.orbit(deltaX, deltaY);
    } else if (this.mouseButton === 2 || (this.mouseButton === 0 && event.shiftKey)) {
      // Right mouse button or Shift+Left - pan
      this.pan(deltaX, deltaY);
    }
    
    this.lastMouseX = event.clientX;
    this.lastMouseY = event.clientY;
    
    event.preventDefault();
  }
  
  onMouseUp(event) {
    this.isMouseDown = false;
    this.mouseButton = -1;
    event.preventDefault();
  }
  
  onWheel(event) {
    const delta = event.deltaY > 0 ? 1 : -1;
    this.zoom(delta);
    event.preventDefault();
  }
  
  orbit(deltaX, deltaY) {
    // Pinch behavior: drag left makes scene rotate left (camera goes right)
    this.azimuth += deltaX * this.orbitSpeed;
    // Drag up makes scene rotate up (camera goes down)  
    this.elevation += deltaY * this.orbitSpeed;
    
    // Clamp elevation to prevent flipping
    this.elevation = Math.max(-Math.PI/2 + 0.1, Math.min(Math.PI/2 - 0.1, this.elevation));
    
    this.updateCameraPosition();
  }
  
  pan(deltaX, deltaY) {
    // Get camera vectors for proper world-space panning
    const right = [
      -Math.sin(this.azimuth),
      0,
      Math.cos(this.azimuth)
    ];
    
    const up = [
      -Math.sin(this.elevation) * Math.cos(this.azimuth),
      Math.cos(this.elevation),
      -Math.sin(this.elevation) * Math.sin(this.azimuth)
    ];
    
    // Pan factor scales with distance for consistent feel
    const panFactor = this.distance * this.panSpeed;
    
    // Correct pan behavior: drag right moves scene right
    this.target[0] += (right[0] * deltaX + up[0] * deltaY) * panFactor;
    this.target[1] += (right[1] * deltaX + up[1] * deltaY) * panFactor;
    this.target[2] += (right[2] * deltaX + up[2] * deltaY) * panFactor;
    
    this.updateCameraPosition();
  }
  
  zoom(delta) {
    this.distance += delta * this.zoomSpeed * this.distance;
    this.distance = Math.max(this.minDistance, Math.min(this.maxDistance, this.distance));
    
    this.updateCameraPosition();
  }
  
  updateCameraPosition() {
    // Spherical to Cartesian conversion
    const cosAzimuth = Math.cos(this.azimuth);
    const sinAzimuth = Math.sin(this.azimuth);
    const cosElevation = Math.cos(this.elevation);
    const sinElevation = Math.sin(this.elevation);
    
    // Calculate position relative to target
    this.position[0] = this.target[0] + this.distance * cosElevation * cosAzimuth;
    this.position[1] = this.target[1] + this.distance * sinElevation;
    this.position[2] = this.target[2] + this.distance * cosElevation * sinAzimuth;
    
    // Update scene manager's camera (ensure we're not modifying object references)
    this.sceneManager.cameraPosition = [this.position[0], this.position[1], this.position[2]];
    this.sceneManager.cameraTarget = [this.target[0], this.target[1], this.target[2]];
    this.sceneManager.updateViewMatrix();
  }
  
  // Public methods for programmatic control
  setTarget(x, y, z) {
    this.target = [x, y, z];
    this.updateCameraPosition();
  }
  
  setDistance(distance) {
    this.distance = Math.max(this.minDistance, Math.min(this.maxDistance, distance));
    this.updateCameraPosition();
  }
  
  reset() {
    this.distance = 10;
    this.azimuth = 0;
    this.elevation = 0.3;
    this.target = [0, 0, 0];
    this.updateCameraPosition();
  }
  
  dispose() {
    // Event listeners are now handled by the renderer
    // Nothing to clean up here since we don't add listeners directly
  }
}