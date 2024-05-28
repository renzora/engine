var input = {
    pressedDirections: [],
    keys: {
        'ArrowUp': "up",
        'ArrowLeft': "left",
        'ArrowRight': "right",
        'ArrowDown': "down",
        'w': "up",
        'a': "left",
        's': "down",
        'd': "right"
    },
    isShiftPressed: false,
    isCtrlPressed: false,
    isAltPressed: false,
    isDragging: false,
    cursorVisibilityTimeout: null,
    cursorHideDelay: 10000, // milliseconds after which to hide the cursor
    deadZone: 0.1, // Dead zone for joystick movement
    joystickSensitivity: 1.0, // Sensitivity for joystick movement
    lastActivityTime: 0, // Track the last time of activity
    cursorVisible: true, // Track cursor visibility

    // Initialization and event handlers
    init: function() {
        this.gamepadConnected = false;
        this.handleKeyDown = this.handleKeyDown.bind(this);
        this.handleKeyUp = this.handleKeyUp.bind(this);
        document.addEventListener("keydown", (e) => this.keyDown(e));
        document.addEventListener("keyup", (e) => this.keyUp(e));
        document.addEventListener('mousedown', (e) => this.mouseDown(e));
        document.addEventListener('mousemove', (e) => this.mouseMove(e));
        document.addEventListener('mouseup', (e) => this.mouseUp(e));
        document.addEventListener('wheel', (e) => this.mouseWheelScroll(e), { passive: false });
        document.addEventListener('click', (e) => this.leftClick(e));
        document.addEventListener('dblclick', (e) => this.doubleClick(e));
        document.addEventListener('contextmenu', (e) => this.rightClick(e));
        window.addEventListener('resize', (e) => game.resizeCanvas(e));
        window.addEventListener('gamepadconnected', (e) => this.connectGamepad(e));
        window.addEventListener('gamepaddisconnected', (e) => this.disconnectGamepad(e));
        
        // Ensure custom cursor element exists
        if (!document.getElementById('customCursor')) {
            const cursorElement = document.createElement('div');
            cursorElement.id = 'customCursor';
            document.body.appendChild(cursorElement);
        }
    },

    loaded: function(e) {
        this.init();
        network.init();
    },

    connectGamepad: function(e) {
        this.gamepadConnected = true;
        const gamepadName = this.simplifyGamepadName(e.gamepad.id);
        console.log('Gamepad connected:', gamepadName);
        ui.notif(`${gamepadName} connected`, "bottom-center");
        this.handleGamepadInput();
    },

    disconnectGamepad: function(e) {
        this.gamepadConnected = false;
        console.log('Gamepad disconnected:', e.gamepad);
    },

    simplifyGamepadName: function(gamepadId) {
        if (gamepadId.includes('054c') && gamepadId.includes('0ce6')) {
            return 'PS5 controller';
        } else if (gamepadId.toLowerCase().includes('xbox')) {
            return 'Xbox controller';
        } else {
            return 'Gamepad'; // Default fallback name
        }
    },

    keyUp: function(e) {
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            e.preventDefault(); // Prevent default action for keyUp
            this.handleKeyUp(e);
        }
    },

    keyDown: function(e) {
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            if (e.key === 'Tab') {
                e.preventDefault();
            }
            this.handleKeyDown(e);
        }
    },

    handleKeyDown: function(e) {
        if (e.altKey && e.key === 'c') {
            if (e.key === 'c') {
                modal.load('mishell/index.php', 'mishell_window');
            }
        } else if (e.key === 'Tab') {
            e.preventDefault();
            modal.load('editMode', 'edit_mode_window');
        } else {
            const dir = this.keys[e.key];
            if (dir) {
                const mainSprite = game.sprites['main'];
                if (mainSprite) {
                    mainSprite.addDirection(dir); // Control the main sprite
                    actions.handleWalkOnTile(mainSprite.x, mainSprite.y);
                } else {
                    console.error('Main sprite not found.');
                }
            }
        }

        if (e.key === 'Shift') {
            this.isShiftPressed = true;
        } else if (e.key === 'Control') {
            this.isCtrlPressed = true;
        } else if (e.key === 'Alt') {
            this.isAltPressed = true;
        }

        if (e.key === 'F') {
            const mainSprite = game.sprites['main'];
            if (mainSprite) {
                mainSprite.targetAim = !mainSprite.targetAim; // Toggle target aiming mode
                if (mainSprite.targetAim) {
                    console.log('Target aim activated');
                } else {
                    console.log('Target aim deactivated');
                }
            } else {
                console.error('Main sprite not found.');
            }
        }

        if (e.key === 'E') {
            const mainSprite = game.sprites['main'];
            const targetX = mainSprite.x + mainSprite.width / 2;
            const targetY = mainSprite.y + mainSprite.height / 2;
            actions.handleObjectInteraction(targetX, targetY);
        }
    },

    handleKeyUp: function(e) {
        if (e.keyCode === 27) { // ESC key
            let maxZIndex = -Infinity;
            let maxZIndexElement = null;
            let attributeName = null;

            document.querySelectorAll('[data-window]').forEach(function(element) {
                let zIndex = parseInt(window.getComputedStyle(element).zIndex, 10);
                if (zIndex > maxZIndex) {
                    maxZIndex = zIndex;
                    maxZIndexElement = element;
                    attributeName = element.getAttribute('data-window');
                }
            });

            if (maxZIndexElement) {
                modal.close(attributeName, true); // Indicate that the close request came from the Esc key
            }
        } else {
            const dir = this.keys[e.key];
            if (dir) {
                const mainSprite = game.sprites['main'];
                if (mainSprite) {
                    mainSprite.removeDirection(dir); // Control the main sprite
                } else {
                    console.error('Main sprite not found.');
                }
            }
        }

        if (e.key === 'Shift') {
            this.isShiftPressed = false;
        } else if (e.key === 'Control') {
            this.isCtrlPressed = false;
        } else if (e.key === 'Alt') {
            this.isAltPressed = false;
        }
    },

    mouseDown: function(e) {
        if (e.button === 1) {
            this.isDragging = true;
            this.startX = e.clientX;
            this.startY = e.clientY;
            document.body.classList.add('move-cursor');
        }
    },

    mouseMove: function(e) {
        if (this.isDragging) {
            const dx = (this.startX - e.clientX) / game.zoomLevel;
            const dy = (this.startY - e.clientY) / game.zoomLevel;

            game.cameraX = Math.max(0, Math.min(game.worldWidth - window.innerWidth / game.zoomLevel, game.cameraX + dx));
            game.cameraY = Math.max(0, Math.min(game.worldHeight - window.innerHeight / game.zoomLevel, game.cameraY + dy));

            this.startX = e.clientX;
            this.startY = e.clientY;
        }

        // Update mouse coordinates for target aiming
        const mainSprite = game.sprites['main'];
        if (mainSprite && mainSprite.targetAim) {
            const rect = game.canvas.getBoundingClientRect();
            mainSprite.targetX = (e.clientX - rect.left) / game.zoomLevel + game.cameraX;
            mainSprite.targetY = (e.clientY - rect.top) / game.zoomLevel + game.cameraY;
        }
    },

    mouseUp: function(e) {
        this.isDragging = false;
        document.body.classList.remove('move-cursor');
    },

    mouseWheelScroll: function(e) {
        const isEventOnCanvas = e.target === game.canvas || game.canvas.contains(e.target);

        if (isEventOnCanvas) {
            e.preventDefault(); // Prevent default scroll behavior for all cases

            if (e.altKey) {
                const panSpeed = 10;
                game.cameraX += e.deltaY > 0 ? panSpeed : -panSpeed;
                game.cameraX = Math.max(0, Math.min(game.cameraX, game.worldWidth - window.innerWidth / game.zoomLevel));
            } else if (e.ctrlKey) {
                const zoomStep = 1;
                const rect = game.canvas.getBoundingClientRect();
                const cursorX = (e.clientX - rect.left) / game.zoomLevel;
                const cursorY = (e.clientY - rect.top) / game.zoomLevel;

                const prevZoomLevel = game.zoomLevel;
                game.zoomLevel += (e.deltaY > 0) ? -zoomStep : zoomStep;
                game.zoomLevel = Math.max(3, Math.min(10, game.zoomLevel));

                const zoomFactor = game.zoomLevel / prevZoomLevel;

                // Adjust camera position to keep the cursor focused
                game.cameraX = cursorX - (cursorX - game.cameraX) * zoomFactor;
                game.cameraY = cursorY - (cursorY - game.cameraY) * zoomFactor;

                // Ensure the camera doesn't go outside the world bounds
                const scaledWindowWidth = window.innerWidth / game.zoomLevel;
                const scaledWindowHeight = window.innerHeight / game.zoomLevel;
                game.cameraX = Math.max(0, Math.min(game.cameraX, game.worldWidth - scaledWindowWidth));
                game.cameraY = Math.max(0, Math.min(game.cameraY, game.worldHeight - scaledWindowHeight));
            } else {
                const panSpeed = 10;
                game.cameraY += e.deltaY > 0 ? panSpeed : -panSpeed;
                game.cameraY = Math.max(0, Math.min(game.cameraY, game.worldHeight - window.innerHeight / game.zoomLevel));
            }
        }
    },

    leftClick: function(e) {
        console.log("left button clicked");

        // Calculate the click position relative to the game world
        const rect = game.canvas.getBoundingClientRect();
        const clickX = (e.clientX - rect.left) / game.zoomLevel + game.cameraX;
        const clickY = (e.clientY - rect.top) / game.zoomLevel + game.cameraY;

        if (e.target.matches('[data-close], [data-esc]')) {
            console.log("data close clicked");
            var parent = modal.closest(e.target);
            modal.close(parent, false); // Indicate that the close request did not come from the Esc key
        } else {
            actions.handleObjectInteraction(clickX, clickY);
        }
    },

    rightClick: function(e) {
        e.preventDefault();
    },

    doubleClick: function(e) {},

    // Gamepad integration functions
    handleGamepadInput: function() {
        const gamepads = navigator.getGamepads ? navigator.getGamepads() : [];
        let anyActiveInput = false;
        for (let gamepad of gamepads) {
            if (!gamepad) continue;

            const leftStickX = gamepad.axes[0]; // Left stick horizontal axis
            const leftStickY = gamepad.axes[1]; // Left stick vertical axis
            this.handleLeftStickInput(leftStickX, leftStickY);

            // Handling right stick movement
            const rightStickX = gamepad.axes[2]; // Right stick horizontal axis
            const rightStickY = gamepad.axes[3]; // Right stick vertical axis
            this.handleRightStickInput(rightStickX, rightStickY);

            // Define actions for each button
            const buttonActions = {
                0: () => this.action1(), // Face button (Bottom)
                1: () => this.action2(), // Face button (Right)
                2: () => this.action3(), // Face button (Left)
                3: () => this.action4(), // Face button (Top)
                4: () => this.leftBumper(), // Left bumper
                5: () => this.rightBumper(), // Right bumper
                6: () => this.leftTrigger(), // Left trigger
                7: () => this.rightTrigger(), // Right trigger
                8: () => this.select(), // Select/Back
                9: () => this.start(), // Start
                10: () => this.leftStickPress(),
                11: () => this.rightStickPress(), // Right stick press
                12: () => this.dPadUp(), // D-Pad Up
                13: () => this.dPadDown(), // D-Pad Down
                14: () => this.dPadLeft(), // D-Pad Left
                15: () => this.dPadRight(), // D-Pad Right
                17: () => this.touchpadButton() // Touchpad button (PS4/PS5)
            };

            // Track pressed buttons to prevent repeated actions on hold
            this.pressedButtons = this.pressedButtons || {};

            // Handle button input for actions
            gamepad.buttons.forEach((button, index) => {
                if (button.pressed) {
                    if (!this.pressedButtons[index] && buttonActions[index]) {
                        buttonActions[index](); // Execute action if button is newly pressed
                    }
                    this.pressedButtons[index] = true; // Mark as pressed
                } else {
                    this.pressedButtons[index] = false; // Reset when button is released
                }
            });

            if (!anyActiveInput) {
                // If no active input is detected, consider resetting the cursor visibility timer
                this.resetCursorVisibilityTimer();
            }
        }

        // Continue to poll for gamepad input
        if (this.gamepadConnected) {
            requestAnimationFrame(() => this.handleGamepadInput());
        }
    },

    handleLeftStickInput: function(x, y) {
        const mainSprite = game.sprites['main'];
        if (!mainSprite) return;
    
        let joystickActive = false;
        const directions = {};
    
        const adjustedX = x * this.joystickSensitivity; // Adjust sensitivity
        const adjustedY = y * this.joystickSensitivity; // Adjust sensitivity
    
        if (Math.abs(adjustedX) > this.deadZone || Math.abs(adjustedY) > this.deadZone) {
            this.hideCustomCursor();
            clearTimeout(this.cursorVisibilityTimeout); // Stop the hide timeout
    
            // Determine direction based on joystick input
            if (adjustedY < -this.deadZone) directions['up'] = true;
            if (adjustedY > this.deadZone) directions['down'] = true;
            if (adjustedX < -this.deadZone) directions['left'] = true;
            if (adjustedX > this.deadZone) directions['right'] = true;
    
            joystickActive = true;
        }
    
        // Update sprite directions and position only if joystick is active
        if (joystickActive) {
            //console.log('Joystick Active, Directions:', directions); // Debugging line
            mainSprite.setDirections(directions);
    
            // Calculate movement based on joystick input
            const deltaX = adjustedX * mainSprite.speed * game.deltaTime / 1000;
            const deltaY = adjustedY * mainSprite.speed * game.deltaTime / 1000;
    
            const newX = mainSprite.x + deltaX;
            const newY = mainSprite.y + deltaY;
    
            // Ensure the new position is within game boundaries and update sprite position
            if (!game.collision(newX, newY, mainSprite)) {
                mainSprite.x = newX;
                mainSprite.y = newY;
            }
    
            actions.handleWalkOnTile(mainSprite.x, mainSprite.y);
        } else {
            mainSprite.clearJoystickDirections();
        }
    },

    handleRightStickInput: function(x, y) {
        const mainSprite = game.sprites['main'];
        if (!mainSprite) return;
    
        const smoothingFactor = 0.1; // Adjust this value to control the smoothing
        const sensitivity = 10; // Reduced sensitivity for more precise control
    
        if (Math.abs(x) > this.deadZone || Math.abs(y) > this.deadZone) {
            if (!this.cursorVisible) {
                this.showCustomCursor();
            }
    
            if (mainSprite.targetAim) {
                // Target aiming mode
                const adjustedX = x * sensitivity;
                const adjustedY = y * sensitivity;
    
                // Apply smoothing to the target position
                mainSprite.targetX = this.lerp(mainSprite.targetX, mainSprite.targetX + adjustedX, smoothingFactor);
                mainSprite.targetY = this.lerp(mainSprite.targetY, mainSprite.targetY + adjustedY, smoothingFactor);
    
                // Ensure the target stays within the bounds of the game world
                mainSprite.targetX = Math.max(0, Math.min(game.worldWidth, mainSprite.targetX));
                mainSprite.targetY = Math.max(0, Math.min(game.worldHeight, mainSprite.targetY));
    
                // Update the target position for aiming
                game.handleAimAttack();
            } else {
                // Cursor movement mode
                const adjustedX = x * sensitivity;
                const adjustedY = y * sensitivity;
    
                // Update cursor position
                this.updateCursorPosition(adjustedX, adjustedY);
    
                // Indicate new input has been detected
                this.newInputDetected = true; // Ensure this flag is used to manage state changes
            }
        } else {
            // If the stick is effectively idle, consider allowing the cursor to hide
            if (this.cursorVisible && !this.newInputDetected) {
                // Only reset the visibility timer if new significant input hasn't been detected
                this.resetCursorVisibilityTimer();
            }
        }
    },    
    
    lerp: function(start, end, t) {
        return start * (1 - t) + end * t;
    },

    handleJoystickInput: function(direction) {
        if (!game.isMovementEnabled) return;
        const mainSprite = game.sprites['main'];
        if (mainSprite) {
            mainSprite.addDirection(direction);
            actions.handleWalkOnTile(mainSprite.x, mainSprite.y);
        }
    },

    showCustomCursor: function() {
        console.log("showCustomCursor called");
        const cursorElement = document.getElementById('customCursor');
        if (cursorElement) {
            cursorElement.style.display = 'block';
        }
        this.cursorVisible = true; // Ensure visibility state is updated
    },

    hideCustomCursor: function() {
        console.log("hideCustomCursor called");
        const cursorElement = document.getElementById('customCursor');
        if (cursorElement) {
            cursorElement.style.display = 'none';
        }
        this.cursorVisible = false; // Ensure visibility state is updated
    },

    resetCursorVisibilityTimer: function() {
        if (this.cursorVisible) {
            clearTimeout(this.cursorVisibilityTimeout);
            this.cursorVisibilityTimeout = setTimeout(() => {
                this.hideCustomCursor();
            }, this.cursorHideDelay);
            console.log("resetCursorVisibilityTimer called");
        }
    },    

    updateCursorPosition: function(x, y) {
        console.log(`updateCursorPosition called with x: ${x}, y: ${y}`);
        if (!this.customCursorPosition) {
            this.customCursorPosition = { x: window.innerWidth / 2, y: window.innerHeight / 2 };
        }
    
        this.customCursorPosition.x += x;
        this.customCursorPosition.y += y;
    
        // Ensure the cursor stays within the bounds of the window
        this.customCursorPosition.x = Math.max(0, Math.min(window.innerWidth, this.customCursorPosition.x));
        this.customCursorPosition.y = Math.max(0, Math.min(window.innerHeight, this.customCursorPosition.y));
    
        this.moveCustomCursor(this.customCursorPosition.x, this.customCursorPosition.y);
    
        // Show the cursor when moved
        if (!this.cursorVisible) {
            this.showCustomCursor();
        }
    
        // Reset the visibility timer
        this.resetCursorVisibilityTimer();
    },    

    moveCustomCursor: function(x, y) {
        const cursorElement = document.getElementById('customCursor');
        if (cursorElement) {
            cursorElement.style.left = `${x}px`;
            cursorElement.style.top = `${y}px`;
        }
    },

    vibrateController: function(duration, strength) {
        const gamepads = navigator.getGamepads ? navigator.getGamepads() : [];
        for (let gamepad of gamepads) {
            if (gamepad && gamepad.vibrationActuator && gamepad.vibrationActuator.type === "dual-rumble") {
                gamepad.vibrationActuator.playEffect("dual-rumble", {
                    startDelay: 0,
                    duration: duration,
                    weakMagnitude: strength,
                    strongMagnitude: strength
                }).catch(e => console.error("Vibration not supported on this device or browser.", e));
            }
        }
    },

    action1: function() {
        // Check if the custom cursor position is defined
        this.vibrateController(500, 0.5);
        if (this.customCursorPosition) {
            // Simulate a left click at the custom cursor's current position
            this.leftClick(null, this.customCursorPosition.x, this.customCursorPosition.y);
        } else {
            // Fallback or error handling if the custom cursor position isn't available
            console.log('Custom cursor position not set or available.');
        }
    },
    action2: function() { modal.closeAll() },
    action3: function() { pie_menu_window.showPieMenu(); },
    action4: function() { console.log('Triangle triggered'); },
    leftBumper: function() { console.log('L1 triggered'); },
    rightBumper: function() { console.log('R1 triggered'); },
    leftTrigger: function() {
        console.log('L2 triggered');
        game.sprites['main'].targetAim = true;
    },
    rightTrigger: function() {
        console.log('R2 triggered');
    },
    select: function() { console.log('select triggered'); },
    start: function() { modal.load('settings/index.php', 'settings_window') },
    leftStickPress: function() { console.log('left stick press triggered'); },
    rightStickPress: function() {
        if (this.customCursorPosition) {
            // Simulate a left click at the custom cursor's current position
            ui.contextMenu(this.customCursorPosition.x, this.customCursorPosition.y, null);
        } else {
            // Fallback or error handling if the custom cursor position isn't available
            console.log('Custom cursor position not set or available.');
        }
     },
    dPadUp: function() { console.log('D pad up triggered'); },
    dPadDown: function() { console.log('D pad down triggered'); },
    dPadLeft: function() { console.log('D pad left triggered'); },
    dPadRight: function() { console.log('D pad right triggered'); },
    touchpadButton: function() { console.log('touchpad button triggered'); },

    removeDirection: function(...dirs) {
        this.pressedDirections = this.pressedDirections.filter(dir => !dirs.includes(dir));
    }
};

document.addEventListener('DOMContentLoaded', (e) => { input.loaded(e) });
