/**
 * InputAPI - Comprehensive input handling for keyboard, mouse, touch, and gamepad
 * Priority: HIGH - Essential for interactive applications
 */
export class InputAPI {
  constructor(scene, babylonObject) {
    this.scene = scene;
    this.babylonObject = babylonObject;
    this._keys = new Set();
    this._mouseButtons = new Set();
    this._mousePosition = { x: 0, y: 0 };
    this._gamepadState = new Map();
    this._touchState = new Map();
    
    this._initializeInputListeners();
  }

  // === INPUT SYSTEM INITIALIZATION ===
  
  _initializeInputListeners() {
    if (!this.scene?.getEngine) return;
    
    const engine = this.scene.getEngine();
    const canvas = engine.getRenderingCanvas();
    
    if (!canvas) return;

    // Keyboard listeners
    canvas.addEventListener('keydown', (e) => {
      this._keys.add(e.code);
    });
    
    canvas.addEventListener('keyup', (e) => {
      this._keys.delete(e.code);
    });

    // Mouse listeners
    canvas.addEventListener('mousedown', (e) => {
      this._mouseButtons.add(e.button);
    });
    
    canvas.addEventListener('mouseup', (e) => {
      this._mouseButtons.delete(e.button);
    });
    
    canvas.addEventListener('mousemove', (e) => {
      const rect = canvas.getBoundingClientRect();
      this._mousePosition.x = e.clientX - rect.left;
      this._mousePosition.y = e.clientY - rect.top;
    });

    // Touch listeners
    canvas.addEventListener('touchstart', (e) => {
      for (let touch of e.changedTouches) {
        this._touchState.set(touch.identifier, {
          x: touch.clientX,
          y: touch.clientY,
          active: true
        });
      }
    });
    
    canvas.addEventListener('touchend', (e) => {
      for (let touch of e.changedTouches) {
        this._touchState.delete(touch.identifier);
      }
    });
    
    canvas.addEventListener('touchmove', (e) => {
      for (let touch of e.changedTouches) {
        if (this._touchState.has(touch.identifier)) {
          this._touchState.set(touch.identifier, {
            x: touch.clientX,
            y: touch.clientY,
            active: true
          });
        }
      }
    });

    // Gamepad update loop
    this._updateGamepads();
  }

  // === KEYBOARD INPUT ===
  
  isKeyPressed(key) {
    // Support both key codes and key names
    const keyCode = this._getKeyCode(key);
    return this._keys.has(keyCode);
  }

  isKeyDown(key) {
    return this.isKeyPressed(key);
  }

  isAnyKeyPressed() {
    return this._keys.size > 0;
  }

  getPressedKeys() {
    return Array.from(this._keys);
  }

  // Key combination checks
  isKeyComboPressed(keys) {
    if (!Array.isArray(keys)) return false;
    return keys.every(key => this.isKeyPressed(key));
  }

  // Common key shortcuts
  isCtrlPressed() {
    return this.isKeyPressed('ControlLeft') || this.isKeyPressed('ControlRight');
  }

  isShiftPressed() {
    return this.isKeyPressed('ShiftLeft') || this.isKeyPressed('ShiftRight');
  }

  isAltPressed() {
    return this.isKeyPressed('AltLeft') || this.isKeyPressed('AltRight');
  }

  // === MOUSE INPUT ===
  
  isMouseButtonPressed(button = 0) {
    return this._mouseButtons.has(button);
  }

  isLeftMouseButtonPressed() {
    return this.isMouseButtonPressed(0);
  }

  isRightMouseButtonPressed() {
    return this.isMouseButtonPressed(2);
  }

  isMiddleMouseButtonPressed() {
    return this.isMouseButtonPressed(1);
  }

  getMousePosition() {
    return [this._mousePosition.x, this._mousePosition.y];
  }

  getMouseX() {
    return this._mousePosition.x;
  }

  getMouseY() {
    return this._mousePosition.y;
  }

  // Normalized mouse coordinates (-1 to 1)
  getMouseNormalized() {
    const canvas = this.scene?.getEngine()?.getRenderingCanvas();
    if (!canvas) return [0, 0];
    
    const x = (this._mousePosition.x / canvas.width) * 2 - 1;
    const y = -((this._mousePosition.y / canvas.height) * 2 - 1);
    return [x, y];
  }

  // === TOUCH INPUT ===
  
  getTouchCount() {
    return this._touchState.size;
  }

  getTouches() {
    const touches = [];
    for (let [id, touch] of this._touchState) {
      touches.push({
        id: id,
        x: touch.x,
        y: touch.y,
        active: touch.active
      });
    }
    return touches;
  }

  getTouch(index = 0) {
    const touches = this.getTouches();
    return touches[index] || null;
  }

  isTouching() {
    return this._touchState.size > 0;
  }

  // Multi-touch gestures (basic implementation)
  getPinchDistance() {
    const touches = this.getTouches();
    if (touches.length < 2) return 0;
    
    const touch1 = touches[0];
    const touch2 = touches[1];
    
    const dx = touch2.x - touch1.x;
    const dy = touch2.y - touch1.y;
    return Math.sqrt(dx * dx + dy * dy);
  }

  getTouchCenter() {
    const touches = this.getTouches();
    if (touches.length === 0) return [0, 0];
    
    let centerX = 0, centerY = 0;
    for (let touch of touches) {
      centerX += touch.x;
      centerY += touch.y;
    }
    
    return [centerX / touches.length, centerY / touches.length];
  }

  // === GAMEPAD INPUT ===
  
  _updateGamepads() {
    if (!navigator.getGamepads) return;
    
    const gamepads = navigator.getGamepads();
    for (let i = 0; i < gamepads.length; i++) {
      const gamepad = gamepads[i];
      if (gamepad) {
        this._gamepadState.set(i, {
          id: gamepad.id,
          buttons: gamepad.buttons.map(btn => ({
            pressed: btn.pressed,
            value: btn.value
          })),
          axes: Array.from(gamepad.axes),
          connected: gamepad.connected,
          timestamp: gamepad.timestamp
        });
      }
    }
    
    // Continue updating
    requestAnimationFrame(() => this._updateGamepads());
  }

  getGamepads() {
    return Array.from(this._gamepadState.values());
  }

  getGamepad(index = 0) {
    return this._gamepadState.get(index) || null;
  }

  isGamepadConnected(index = 0) {
    const gamepad = this.getGamepad(index);
    return gamepad?.connected || false;
  }

  isGamepadButtonPressed(buttonIndex, gamepadIndex = 0) {
    const gamepad = this.getGamepad(gamepadIndex);
    if (!gamepad || !gamepad.buttons[buttonIndex]) return false;
    
    return gamepad.buttons[buttonIndex].pressed;
  }

  getGamepadButtonValue(buttonIndex, gamepadIndex = 0) {
    const gamepad = this.getGamepad(gamepadIndex);
    if (!gamepad || !gamepad.buttons[buttonIndex]) return 0;
    
    return gamepad.buttons[buttonIndex].value;
  }

  // Analog stick input
  getLeftStick(gamepadIndex = 0, deadzone = 0.1) {
    const gamepad = this.getGamepad(gamepadIndex);
    if (!gamepad || gamepad.axes.length < 2) return [0, 0];
    
    const x = -gamepad.axes[0]; // Invert X-axis for correct left/right
    const y = gamepad.axes[1];
    
    // Apply deadzone
    return this.applyDeadzone(x, y, deadzone);
  }

  getRightStick(gamepadIndex = 0, deadzone = 0.1) {
    const gamepad = this.getGamepad(gamepadIndex);
    if (!gamepad || gamepad.axes.length < 4) return [0, 0];
    
    const x = -gamepad.axes[2]; // Invert X-axis for correct left/right
    const y = gamepad.axes[3];
    
    // Apply deadzone
    return this.applyDeadzone(x, y, deadzone);
  }

  getLeftStickX(gamepadIndex = 0) {
    return this.getLeftStick(gamepadIndex)[0];
  }

  getLeftStickY(gamepadIndex = 0) {
    return this.getLeftStick(gamepadIndex)[1];
  }

  getRightStickX(gamepadIndex = 0) {
    return this.getRightStick(gamepadIndex)[0];
  }

  getRightStickY(gamepadIndex = 0) {
    return this.getRightStick(gamepadIndex)[1];
  }

  // Trigger input
  getLeftTrigger(gamepadIndex = 0) {
    return this.getGamepadButtonValue(6, gamepadIndex);
  }

  getRightTrigger(gamepadIndex = 0) {
    return this.getGamepadButtonValue(7, gamepadIndex);
  }

  getGamepadTrigger(trigger, gamepadIndex = 0) {
    if (trigger === 'left' || trigger === 'L2') {
      return this.getLeftTrigger(gamepadIndex);
    } else if (trigger === 'right' || trigger === 'R2') {
      return this.getRightTrigger(gamepadIndex);
    }
    return 0;
  }

  // Common gamepad button mappings (Xbox controller standard)
  isGamepadButtonA(gamepadIndex = 0) {
    return this.isGamepadButtonPressed(0, gamepadIndex);
  }

  isGamepadButtonB(gamepadIndex = 0) {
    return this.isGamepadButtonPressed(1, gamepadIndex);
  }

  isGamepadButtonX(gamepadIndex = 0) {
    return this.isGamepadButtonPressed(2, gamepadIndex);
  }

  isGamepadButtonY(gamepadIndex = 0) {
    return this.isGamepadButtonPressed(3, gamepadIndex);
  }

  // === ADVANCED INPUT FEATURES ===
  
  // Deadzone handling for analog sticks
  applyDeadzone(x, y, deadzone = 0.1) {
    const magnitude = Math.sqrt(x * x + y * y);
    if (magnitude < deadzone) return [0, 0];
    
    const scale = (magnitude - deadzone) / (1 - deadzone);
    const normalizedX = x / magnitude;
    const normalizedY = y / magnitude;
    
    return [normalizedX * scale, normalizedY * scale];
  }

  getLeftStickWithDeadzone(deadzone = 0.1, gamepadIndex = 0) {
    const [x, y] = this.getLeftStick(gamepadIndex);
    return this.applyDeadzone(x, y, deadzone);
  }

  getRightStickWithDeadzone(deadzone = 0.1, gamepadIndex = 0) {
    const [x, y] = this.getRightStick(gamepadIndex);
    return this.applyDeadzone(x, y, deadzone);
  }

  // Input events
  onKeyDown(callback) {
    if (!callback || !this.scene) return;
    
    const canvas = this.scene.getEngine()?.getRenderingCanvas();
    if (canvas) {
      canvas.addEventListener('keydown', callback);
    }
  }

  onKeyUp(callback) {
    if (!callback || !this.scene) return;
    
    const canvas = this.scene.getEngine()?.getRenderingCanvas();
    if (canvas) {
      canvas.addEventListener('keyup', callback);
    }
  }

  onMouseDown(callback) {
    if (!callback || !this.scene) return;
    
    const canvas = this.scene.getEngine()?.getRenderingCanvas();
    if (canvas) {
      canvas.addEventListener('mousedown', callback);
    }
  }

  onMouseUp(callback) {
    if (!callback || !this.scene) return;
    
    const canvas = this.scene.getEngine()?.getRenderingCanvas();
    if (canvas) {
      canvas.addEventListener('mouseup', callback);
    }
  }

  // === POINTER LOCK API ===
  
  requestPointerLock() {
    const canvas = this.scene?.getEngine()?.getRenderingCanvas();
    if (canvas && canvas.requestPointerLock) {
      canvas.requestPointerLock();
    }
  }

  exitPointerLock() {
    if (document.exitPointerLock) {
      document.exitPointerLock();
    }
  }

  isPointerLocked() {
    return document.pointerLockElement !== null;
  }

  // === VIRTUAL JOYSTICKS ===
  
  createVirtualJoystick() {
    console.warn('Virtual joystick creation requires UI implementation');
    return null;
  }

  // === HAPTIC FEEDBACK ===
  
  vibrateGamepad(gamepadIndex = 0, intensity = 1.0, duration = 200) {
    if (!navigator.getGamepads) return false;
    
    const gamepads = navigator.getGamepads();
    const gamepad = gamepads[gamepadIndex];
    
    if (gamepad && gamepad.vibrationActuator) {
      gamepad.vibrationActuator.playEffect('dual-rumble', {
        startDelay: 0,
        duration: duration,
        weakMagnitude: intensity * 0.5,
        strongMagnitude: intensity
      });
      return true;
    }
    
    return false;
  }

  // === UTILITY METHODS ===
  
  _getKeyCode(key) {
    // Convert common key names to key codes
    const keyMap = {
      'space': 'Space',
      'enter': 'Enter',
      'escape': 'Escape',
      'tab': 'Tab',
      'shift': 'ShiftLeft',
      'ctrl': 'ControlLeft',
      'alt': 'AltLeft',
      'up': 'ArrowUp',
      'down': 'ArrowDown',
      'left': 'ArrowLeft',
      'right': 'ArrowRight',
      'w': 'KeyW',
      'a': 'KeyA',
      's': 'KeyS',
      'd': 'KeyD'
    };
    
    return keyMap[key.toLowerCase()] || key;
  }

  // Input state snapshot for replay/recording
  getInputSnapshot() {
    return {
      keys: Array.from(this._keys),
      mouseButtons: Array.from(this._mouseButtons),
      mousePosition: { ...this._mousePosition },
      gamepads: this.getGamepads(),
      touches: this.getTouches(),
      timestamp: performance.now()
    };
  }

  // Cleanup
  dispose() {
    this._keys.clear();
    this._mouseButtons.clear();
    this._gamepadState.clear();
    this._touchState.clear();
  }
  
  // === SHORT NAME ALIASES ===
  
  pressedKeys() {
    return this.getPressedKeys();
  }
  
  mousePosition() {
    return this.getMousePosition();
  }
  
  mouseX() {
    return this.getMouseX();
  }
  
  mouseY() {
    return this.getMouseY();
  }
  
  mouseNormalized() {
    return this.getMouseNormalized();
  }
  
  touchCount() {
    return this.getTouchCount();
  }
  
  touches() {
    return this.getTouches();
  }
  
  touch(index = 0) {
    return this.getTouch(index);
  }
  
  pinchDistance() {
    return this.getPinchDistance();
  }
  
  touchCenter() {
    return this.getTouchCenter();
  }
  
  gamepads() {
    return this.getGamepads();
  }
  
  gamepad(index = 0) {
    return this.getGamepad(index);
  }
  
  leftStick(gamepadIndex = 0, deadzone = 0.1) {
    return this.getLeftStick(gamepadIndex, deadzone);
  }
  
  rightStick(gamepadIndex = 0, deadzone = 0.1) {
    return this.getRightStick(gamepadIndex, deadzone);
  }
  
  leftTrigger(gamepadIndex = 0) {
    return this.getLeftTrigger(gamepadIndex);
  }
  
  rightTrigger(gamepadIndex = 0) {
    return this.getRightTrigger(gamepadIndex);
  }
  
  inputSnapshot() {
    return this.getInputSnapshot();
  }
}