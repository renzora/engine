import { createSignal, createEffect, onMount, onCleanup } from 'solid-js';
import { editorStore } from '@/layout/stores/EditorStore';
import { useCameraController } from './CameraController';
import PlayCanvasRenderer from './PlayCanvasRenderer';
import Stats from 'stats.js';

export const PlayCanvasViewport = (props) => {
  const [canvasRef, setCanvasRef] = createSignal(null);
  const [initialized, setInitialized] = createSignal(false);
  const [renderer, setRenderer] = createSignal(null);
  const [frameCount, setFrameCount] = createSignal(0);
  const [statsRef, setStatsRef] = createSignal(null);

  // Mouse event handlers
  const handleMouseDown = (event) => {
    const app = pcApp();
    if (!app) return;
    const camera = app.root.findByName('camera');
    if (!camera) return;

    event.preventDefault();
    lastMouseX = event.clientX;
    lastMouseY = event.clientY;
    mouseDownPos = { x: event.clientX, y: event.clientY };
    isCameraDragging = false;

    if (event.button === 0) isLeftMouseDown = true;
    if (event.button === 1) isMiddleMouseDown = true;
    if (event.button === 2) isRightMouseDown = true;
  };

  const handleMouseMove = (event) => {
    const app = pcApp();
    if (!app) return;
    const camera = app.root.findByName('camera');
    if (!camera) return;

    const deltaX = event.clientX - lastMouseX;
    const deltaY = event.clientY - lastMouseY;

    if ((isLeftMouseDown || isMiddleMouseDown || isRightMouseDown) && mouseDownPos) {
      const totalDelta = Math.abs(event.clientX - mouseDownPos.x) + Math.abs(event.clientY - mouseDownPos.y);
      if (totalDelta > 5) {
        isCameraDragging = true;
      }
    }

    const precisionMultiplier = keysPressed.has('Control') ? 0.2 : 1.0;
    const cameraSettings = () => viewportStore.camera;
    const mouseSensitivity = () => cameraSettings()?.mouseSensitivity || 0.002;
    const rotationSpeed = () => mouseSensitivity();
    const panSpeed = 0.01;

    const getForwardDirection = (camera) => {
      const forward = new pc.Vec3();
      camera.getWorldTransform().getZ(forward);
      forward.scale(-1); // PlayCanvas uses negative Z as forward
      return forward;
    };
    
    const getRightDirection = (camera) => {
      const right = new pc.Vec3();
      camera.getWorldTransform().getX(right);
      return right;
    };
    
    const getUpDirection = (camera) => {
      const up = new pc.Vec3();
      camera.getWorldTransform().getY(up);
      return up;
    };

    if (isLeftMouseDown) {
      // Forward/backward movement (like Babylon.js left mouse)
      const forward = getForwardDirection(camera);
      const horizontalForward = new pc.Vec3(forward.x, 0, forward.z).normalize();
      const moveDirection = horizontalForward.clone().scale(-deltaY * 0.025 * precisionMultiplier);
      const newPos = camera.getPosition().clone().add(moveDirection);
      camera.setPosition(newPos);
    } else if (isRightMouseDown) {
      // Look around (like Babylon.js right mouse)
      const angles = camera.getEulerAngles();
      const newAngles = new pc.Vec3(
        angles.x + deltaY * rotationSpeed() * 180 / Math.PI * precisionMultiplier,
        angles.y + deltaX * rotationSpeed() * 180 / Math.PI * precisionMultiplier,
        angles.z
      );
      // Clamp pitch
      newAngles.x = Math.max(-90, Math.min(90, newAngles.x));
      camera.setEulerAngles(newAngles);
    } else if (isMiddleMouseDown) {
      // Pan (like Babylon.js middle mouse)
      const right = getRightDirection(camera);
      const up = getUpDirection(camera);
      const panVector = right.clone().scale(-deltaX * panSpeed * precisionMultiplier)
        .add(up.clone().scale(deltaY * panSpeed * precisionMultiplier));
      const newPos = camera.getPosition().clone().add(panVector);
      camera.setPosition(newPos);
    }

    lastMouseX = event.clientX;
    lastMouseY = event.clientY;
  };

  const handleMouseUp = (event) => {
    if (event.button === 0) isLeftMouseDown = false;
    if (event.button === 1) isMiddleMouseDown = false;
    if (event.button === 2) isRightMouseDown = false;
  };

  const handleWheel = (event) => {
    const app = pcApp();
    if (!app) return;
    const camera = app.root.findByName('camera');
    if (!camera) return;

    event.preventDefault();
    const delta = event.deltaY * -0.01;
    const cameraSettings = () => viewportStore.camera;
    const zoomSpeed = 0.3;
    
    const getForwardDirection = (camera) => {
      const forward = new pc.Vec3();
      camera.getWorldTransform().getZ(forward);
      forward.scale(-1);
      return forward;
    };
    
    const forward = getForwardDirection(camera);
    const precisionMultiplier = keysPressed.has('Control') ? 0.2 : 1.0;
    
    let wheelSpeedMultiplier = 1.0;
    if (delta > 0) {
      wheelSpeedMultiplier = 0.6;
    } else {
      wheelSpeedMultiplier = 1.5;
    }
    
    const moveVector = forward.clone().scale(delta * zoomSpeed * wheelSpeedMultiplier * precisionMultiplier);
    const newPos = camera.getPosition().clone().add(moveVector);
    camera.setPosition(newPos);
  };

  // Keyboard event listeners
  const handleKeyDown = (event) => {
    const key = event.key.toLowerCase();
    keysPressed.add(key);
    
    // Update old keys object for compatibility
    switch(event.code) {
      case 'KeyW': keys.w = true; break;
      case 'KeyA': keys.a = true; break;
      case 'KeyS': keys.s = true; break;
      case 'KeyD': keys.d = true; break;
      case 'KeyQ': keys.q = true; break;
      case 'KeyE': keys.e = true; break;
    }
  };

  const handleKeyUp = (event) => {
    const key = event.key.toLowerCase();
    keysPressed.delete(key);
    
    // Update old keys object for compatibility
    switch(event.code) {
      case 'KeyW': keys.w = false; break;
      case 'KeyA': keys.a = false; break;
      case 'KeyS': keys.s = false; break;
      case 'KeyD': keys.d = false; break;
      case 'KeyQ': keys.q = false; break;
      case 'KeyE': keys.e = false; break;
    }
  };

  // Initialize PlayCanvas renderer
  const initializeRenderer = async () => {
    try {
      const canvas = canvasRef();
      if (!canvas) return;

      console.log('🎯 Initializing PlayCanvas renderer...');
      
      // Import PlayCanvas
      const pc = await import('playcanvas');
      
      // Create PlayCanvas application
      const app = new pc.Application(canvas, {
        mouse: new pc.Mouse(canvas),
        keyboard: new pc.Keyboard(window),
        touch: new pc.TouchDevice(canvas),
        antiAlias: true,
        alpha: true
      });
      
      // Set canvas size
      app.setCanvasFillMode(pc.FILLMODE_FILL_WINDOW);
      app.setCanvasResolution(pc.RESOLUTION_AUTO);
      
      // Create camera entity
      const camera = new pc.Entity('camera');
      camera.addComponent('camera', {
        clearColor: new pc.Color(0.1, 0.1, 0.15),
        farClip: 100,
        nearClip: 0.1,
        fov: 60
      });
      camera.setPosition(0, 2, 8);
      camera.lookAt(0, 0, 0);
      app.root.addChild(camera);
      
      // Create light entity
      const light = new pc.Entity('light');
      light.addComponent('light', {
        type: pc.LIGHTTYPE_DIRECTIONAL,
        color: new pc.Color(1, 1, 1),
        intensity: 1,
        castShadows: true,
        shadowBias: 0.05,
        normalOffsetBias: 0.1,
        shadowDistance: 20,
        shadowResolution: 4096,
        shadowType: pc.SHADOW_PCF5,
        vsmBlurSize: 11,
        cookieIntensity: 1
      });
      light.setEulerAngles(45, 135, 0);
      app.root.addChild(light);
      
      // Create ambient light
      const ambientLight = new pc.Entity('ambient');
      ambientLight.addComponent('light', {
        type: pc.LIGHTTYPE_AMBIENT,
        color: new pc.Color(0.4, 0.4, 0.4),
        intensity: 0.3
      });
      app.root.addChild(ambientLight);
      
      // Create cube entity
      const cube = new pc.Entity('cube');
      cube.addComponent('render', {
        type: 'box',
        castShadows: true,
        receiveShadows: true
      });
      
      // Create material
      const material = new pc.StandardMaterial();
      material.diffuse = new pc.Color(1, 0.4, 0.4);
      material.shininess = 60;
      material.useMetalness = true;
      material.metalness = 0.3;
      material.update();
      
      cube.render.material = material;
      cube.setPosition(0, -1.5, 0); // Position cube on the floor (ground is at y=-2, cube is 1 unit tall)
      app.root.addChild(cube);
      
      // Make cube selectable
      cube.isSelectable = true;
      
      // Create ground plane
      const ground = new pc.Entity('ground');
      ground.addComponent('render', {
        type: 'plane',
        receiveShadows: true
      });
      
      const groundMaterial = new pc.StandardMaterial();
      groundMaterial.diffuse = new pc.Color(0.5, 0.5, 0.5);
      groundMaterial.update();
      
      ground.render.material = groundMaterial;
      ground.setPosition(0, -2, 0);
      ground.setLocalScale(10, 1, 10);
      app.root.addChild(ground);
      
      // Create transform gizmo
      const createGizmo = (entity) => {
        const gizmoEntity = new pc.Entity('gizmo');
        
        // Translation handles (X, Y, Z arrows)
        const createArrow = (color, direction, name) => {
          const arrow = new pc.Entity(name);
          arrow.addComponent('render', {
            type: 'cylinder',
            castShadows: false,
            receiveShadows: false
          });
          
          const arrowMaterial = new pc.StandardMaterial();
          arrowMaterial.diffuse = color;
          arrowMaterial.emissive = color;
          arrowMaterial.emissiveIntensity = 0.3;
          arrowMaterial.update();
          
          arrow.render.material = arrowMaterial;
          arrow.setLocalScale(0.05, 1, 0.05);
          arrow.setLocalPosition(direction.x, direction.y, direction.z);
          
          if (direction.x !== 0) arrow.setLocalEulerAngles(0, 0, 90);
          if (direction.z !== 0) arrow.setLocalEulerAngles(90, 0, 0);
          
          arrow.gizmoAxis = name.toLowerCase();
          arrow.isGizmoHandle = true;
          
          return arrow;
        };
        
        // Create XYZ arrows
        const xArrow = createArrow(new pc.Color(1, 0, 0), new pc.Vec3(1, 0, 0), 'X');
        const yArrow = createArrow(new pc.Color(0, 1, 0), new pc.Vec3(0, 1, 0), 'Y');
        const zArrow = createArrow(new pc.Color(0, 0, 1), new pc.Vec3(0, 0, 1), 'Z');
        
        gizmoEntity.addChild(xArrow);
        gizmoEntity.addChild(yArrow);
        gizmoEntity.addChild(zArrow);
        
        // Position gizmo at entity
        gizmoEntity.setPosition(entity.getPosition());
        
        app.root.addChild(gizmoEntity);
        return gizmoEntity;
      };
      
      // Mouse picking and gizmo interaction
      let isDragging = false;
      let dragAxis = null;
      let lastMousePos = new pc.Vec2();
      let dragStartPos = new pc.Vec3();
      
      // Convert screen coordinates to normalized device coordinates
      const screenToNDC = (x, y) => {
        const gfxCanvas = app.graphicsDevice.canvas;
        return {
          x: (x / gfxCanvas.clientWidth) * 2 - 1,
          y: -((y / gfxCanvas.clientHeight) * 2 - 1)
        };
      };
      
      // Simple AABB intersection test
      const testAABB = (ray, position, size) => {
        const min = new pc.Vec3(position.x - size, position.y - size, position.z - size);
        const max = new pc.Vec3(position.x + size, position.y + size, position.z + size);
        
        const invDir = new pc.Vec3(1 / ray.direction.x, 1 / ray.direction.y, 1 / ray.direction.z);
        
        const t1 = (min.x - ray.origin.x) * invDir.x;
        const t2 = (max.x - ray.origin.x) * invDir.x;
        const t3 = (min.y - ray.origin.y) * invDir.y;
        const t4 = (max.y - ray.origin.y) * invDir.y;
        const t5 = (min.z - ray.origin.z) * invDir.z;
        const t6 = (max.z - ray.origin.z) * invDir.z;
        
        const tmin = Math.max(Math.max(Math.min(t1, t2), Math.min(t3, t4)), Math.min(t5, t6));
        const tmax = Math.min(Math.min(Math.max(t1, t2), Math.max(t3, t4)), Math.max(t5, t6));
        
        if (tmax < 0 || tmin > tmax) return false;
        return tmin > 0 ? tmin : tmax;
      };
      
      app.mouse.on('mousedown', (event) => {
        if (event.button === pc.MOUSEBUTTON_LEFT) {
          const camera = app.root.findByName('camera');
          const ndc = screenToNDC(event.x, event.y);
          
          // Create ray from camera through mouse position
          const ray = camera.camera.screenToWorld(event.x, event.y, 1);
          const rayDirection = new pc.Vec3().sub2(ray, camera.getPosition()).normalize();
          
          const cameraRay = {
            origin: camera.getPosition(),
            direction: rayDirection
          };
          
          let hitEntity = null;
          let closestDistance = Infinity;
          
          // Check cube first
          if (cube.isSelectable) {
            const distance = testAABB(cameraRay, cube.getPosition(), 0.5);
            if (distance !== false && distance < closestDistance) {
              hitEntity = cube;
              closestDistance = distance;
            }
          }
          
          // Check gizmo handles if we have a selected entity
          if (selectedEntity() && gizmo()) {
            const gizmoChildren = gizmo().children;
            for (let child of gizmoChildren) {
              if (child.isGizmoHandle) {
                const worldPos = new pc.Vec3().add2(gizmo().getPosition(), child.getLocalPosition());
                const distance = testAABB(cameraRay, worldPos, 0.1);
                if (distance !== false && distance < closestDistance) {
                  hitEntity = child;
                  closestDistance = distance;
                }
              }
            }
          }
          
          if (hitEntity) {
            if (hitEntity.isGizmoHandle) {
              // Start gizmo drag
              isDragging = true;
              dragAxis = hitEntity.gizmoAxis;
              lastMousePos.set(event.x, event.y);
              const selectedEnt = selectedEntity();
              if (selectedEnt) {
                dragStartPos.copy(selectedEnt.getPosition());
              }
              console.log('🎯 Started dragging:', dragAxis, 'axis');
            } else if (hitEntity.isSelectable) {
              // Select entity
              setSelectedEntity(hitEntity);
              
              // Remove old gizmo
              const oldGizmo = gizmo();
              if (oldGizmo) {
                oldGizmo.destroy();
              }
              
              // Create new gizmo
              const newGizmo = createGizmo(hitEntity);
              setGizmo(newGizmo);
              
              console.log('🎯 Selected:', hitEntity.name);
            }
          } else {
            // Deselect
            setSelectedEntity(null);
            const oldGizmo = gizmo();
            if (oldGizmo) {
              oldGizmo.destroy();
              setGizmo(null);
            }
            console.log('🎯 Deselected');
          }
        }
      });
      
      app.mouse.on('mousemove', (event) => {
        if (isDragging && dragAxis && selectedEntity()) {
          const deltaX = event.x - lastMousePos.x;
          const deltaY = event.y - lastMousePos.y;
          
          const entity = selectedEntity();
          const currentPos = entity.getPosition();
          
          // Convert screen space movement to world space
          const sensitivity = 0.01;
          
          switch (dragAxis) {
            case 'x':
              entity.setPosition(currentPos.x + deltaX * sensitivity, currentPos.y, currentPos.z);
              break;
            case 'y':
              entity.setPosition(currentPos.x, currentPos.y - deltaY * sensitivity, currentPos.z);
              break;
            case 'z':
              entity.setPosition(currentPos.x, currentPos.y, currentPos.z + deltaY * sensitivity);
              break;
          }
          
          // Update gizmo position
          const currentGizmo = gizmo();
          if (currentGizmo) {
            currentGizmo.setPosition(entity.getPosition());
          }
          
          lastMousePos.set(event.x, event.y);
        }
      });
      
      app.mouse.on('mouseup', (event) => {
        if (event.button === pc.MOUSEBUTTON_LEFT) {
          isDragging = false;
          dragAxis = null;
        }
      });
      
      // Camera helper functions (inside initializeRenderer scope)
      const cameraSettings = () => viewportStore.camera;
      const cameraSpeed = () => cameraSettings()?.speed || 5;
      const mouseSensitivity = () => cameraSettings()?.mouseSensitivity || 0.002;
      const rotationSpeed = () => mouseSensitivity();
      const panSpeed = 0.01;
      const zoomSpeed = 0.3;
      const moveSpeed = () => 0.35 * (cameraSpeed() / 5);
      
      const getForwardDirection = (camera) => {
        const forward = new pc.Vec3();
        camera.getWorldTransform().getZ(forward);
        forward.scale(-1); // PlayCanvas uses negative Z as forward
        return forward;
      };
      
      const getRightDirection = (camera) => {
        const right = new pc.Vec3();
        camera.getWorldTransform().getX(right);
        return right;
      };
      
      const getUpDirection = (camera) => {
        const up = new pc.Vec3();
        camera.getWorldTransform().getY(up);
        return up;
      };

      // Add mouse event listeners
      const appCanvas = app.graphicsDevice.canvas;
      appCanvas.addEventListener('contextmenu', (e) => e.preventDefault());
      appCanvas.addEventListener('mousedown', handleMouseDown);
      appCanvas.addEventListener('mousemove', handleMouseMove);
      appCanvas.addEventListener('mouseup', handleMouseUp);
      appCanvas.addEventListener('wheel', handleWheel, { passive: false });
      
      // Add keyboard event listeners
      window.addEventListener('keydown', handleKeyDown);
      window.addEventListener('keyup', handleKeyUp);
      
      // WASD keyboard movement (Babylon.js style)
      const handleKeyboardMovement = () => {
        const camera = app.root.findByName('camera');
        if (!camera) return;
        
        const speed = moveSpeed();
        const speedMultiplier = keysPressed.has('shift') ? 2.0 : 1.0;
        const finalSpeed = speed * speedMultiplier;
        
        const forward = getForwardDirection(camera);
        const right = getRightDirection(camera);
        const up = new pc.Vec3(0, 1, 0); // World up
        
        let movement = new pc.Vec3(0, 0, 0);
        
        if (keys.w) movement.add(forward);
        if (keys.s) movement.sub(forward);
        if (keys.d) movement.add(right);
        if (keys.a) movement.sub(right);
        if (keys.e) movement.add(up);
        if (keys.q) movement.sub(up);
        
        if (movement.length() > 0) {
          movement.normalize().scale(finalSpeed);
          const newPos = camera.getPosition().clone().add(movement);
          camera.setPosition(newPos);
        }
      };

      // Update loop
      app.on('update', (dt) => {
        // Update stats
        if (statsRef()) {
          statsRef().begin();
        }
        
        // Update camera movement
        handleKeyboardMovement();
        
        setFrameCount(prev => prev + 1);
      });
      
      // Post-render stats update
      app.on('postrender', () => {
        if (statsRef()) {
          statsRef().end();
        }
      });
      
      // Start the application
      app.start();
      
      // Store globally for scene data extraction
      window.globalPlayCanvasApp = () => app;
      
      setPcApp(app);
      setInitialized(true);

    } catch (error) {
      console.error('🎯 Failed to initialize PlayCanvas:', error);
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
    const app = pcApp();
    if (app && app.graphicsDevice && app.graphicsDevice.canvas) {
      const appCanvas = app.graphicsDevice.canvas;
      appCanvas.removeEventListener('contextmenu', (e) => e.preventDefault());
      appCanvas.removeEventListener('mousedown', handleMouseDown);
      appCanvas.removeEventListener('mousemove', handleMouseMove);
      appCanvas.removeEventListener('mouseup', handleMouseUp);
      appCanvas.removeEventListener('wheel', handleWheel);
    }
    
    // Remove keyboard event listeners
    window.removeEventListener('keydown', handleKeyDown);
    window.removeEventListener('keyup', handleKeyUp);
    
    if (app) {
      app.destroy();
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
        🎯 PlayCanvas {initialized() ? `(Frame ${frameCount()})` : '(Loading...)'}
      </div>
    </div>
  );
};