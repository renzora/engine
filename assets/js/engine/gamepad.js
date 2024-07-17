var gamepad = {
    gamepadIndex: null,
    pressedButtons: [],
    axes: [],
    isConnected: false,
    name: '',
    buttonMap: {},
    buttonPressures: {},
    axesPressures: {},

    init: function() {
        window.addEventListener("gamepadconnected", (e) => this.connectGamepad(e));
        window.addEventListener("gamepaddisconnected", (e) => this.disconnectGamepad(e));
        this.updateGamepadState();

        // Initialize button map
        this.buttonMap = {
            0: this.handleAButton.bind(this),
            1: this.handleBButton.bind(this),
            2: this.handleXButton.bind(this),
            3: this.handleYButton.bind(this),
            4: this.handleLeftBumper.bind(this),
            5: this.handleRightBumper.bind(this),
            6: this.handleLeftTrigger.bind(this),
            7: this.handleRightTrigger.bind(this),
            8: this.handleSelectButton.bind(this),
            9: this.handleStartButton.bind(this),
            10: this.handleLeftStickButton.bind(this),
            11: this.handleRightStickButton.bind(this),
            12: this.handleDPadUp.bind(this),
            13: this.handleDPadDown.bind(this),
            14: this.handleDPadLeft.bind(this),
            15: this.handleDPadRight.bind(this)
        };
    },

    connectGamepad: function(e) {
        this.gamepadIndex = e.gamepad.index;
        this.isConnected = true;
        this.name = this.getGamepadName(e.gamepad); // Store the gamepad name
        game.updateInputMethod('gamepad', this.name);
        console.log("Gamepad connected at index " + this.gamepadIndex);
    },

    disconnectGamepad: function(e) {
        if (e.gamepad.index === this.gamepadIndex) {
            this.isConnected = false;
            this.gamepadIndex = null;
            game.updateInputMethod('keyboard'); // Revert to keyboard input method when gamepad is disconnected
            console.log("Gamepad disconnected from index " + this.gamepadIndex);
        }
    },

    updateGamepadState: function() {
        if (this.isConnected && this.gamepadIndex !== null) {
            const gamepad = navigator.getGamepads()[this.gamepadIndex];
            if (gamepad) {
                // Only update input method if there is active input from the gamepad
                if (this.hasActiveInput(gamepad)) {
                    game.updateInputMethod('gamepad', this.name);
                }
                this.handleButtons(gamepad.buttons);
                this.handleLeftAxes(gamepad.axes);
                this.handleRightAxes(gamepad.axes);
            }
        }
        requestAnimationFrame(() => this.updateGamepadState());
    },

    hasActiveInput: function(gamepad) {
        // Check for any button presses or significant axis movements
        const threshold = 0.2;
        const buttonsPressed = gamepad.buttons.some(button => button.pressed);
        const axesMoved = gamepad.axes.some(axis => Math.abs(axis) > threshold);
        return buttonsPressed || axesMoved;
    },

    getGamepadName: function(gamepad) {
        console.log(gamepad);
        const { id } = gamepad;
        if (id.includes('054c') && id.includes('0ce6')) {
            return 'PS5';
        } else if (id.toLowerCase().includes('xbox')) {
            return 'Xbox';
        } else if (id.toLowerCase().includes('nintendo') || id.toLowerCase().includes('switch')) {
            return 'Switch';
        } else if (id.toLowerCase().includes('logitech')) {
            return 'Logitech';
        } else if (id.toLowerCase().includes('steelseries')) {
            return 'SteelSeries';
        } else {
            return 'Generic';
        }
    },

    handleButtons: function(buttons) {
        buttons.forEach((button, index) => {
            const pressure = button.value; // Pressure value ranges from 0 to 1
            this.buttonPressures[index] = pressure;

            if (button.pressed) {
                if (!this.pressedButtons.includes(index)) {
                    this.pressedButtons.push(index);
                    if (this.buttonMap[index]) this.buttonMap[index]("down", pressure);
                } else {
                    // Update the pressure continuously while the button is pressed
                    if (this.buttonMap[index]) this.buttonMap[index]("down", pressure);
                }
            } else {
                const buttonIndex = this.pressedButtons.indexOf(index);
                if (buttonIndex > -1) {
                    this.pressedButtons.splice(buttonIndex, 1);
                    if (this.buttonMap[index]) this.buttonMap[index]("up", pressure);
                }
            }
        });
    },

    handleLeftAxes: function(axes) {
        const threshold = 0.2; // Dead zone threshold
    
        // Calculate axis pressures
        const leftStickX = Math.abs(axes[0]);
        const leftStickY = Math.abs(axes[1]);
    
        // Reset gamepad directions
        gamepad.directions = { left: false, right: false, up: false, down: false };
    
        if (leftStickX > threshold || leftStickY > threshold) {
            this.axesPressures.leftStickX = leftStickX;
            this.axesPressures.leftStickY = leftStickY;
    
            // Left stick movement (axes[0] = left/right, axes[1] = up/down)
            gamepad.directions.right = axes[0] > threshold;
            gamepad.directions.left = axes[0] < -threshold;
            gamepad.directions.down = axes[1] > threshold;
            gamepad.directions.up = axes[1] < -threshold;
    
            // Update the sprite's directions based on combined states
            input.updateSpriteDirections();
        } else {
            // Reset the directions for left stick if below threshold
            this.axesPressures.leftStickX = 0;
            this.axesPressures.leftStickY = 0;
        }
    
        // Update the sprite's directions based on combined states
        input.updateSpriteDirections();
    },

    handleRightAxes: function(axes) {
        const threshold = 0.2; // Dead zone threshold
    
        // Calculate axis pressures
        const rightStickX = Math.abs(axes[2]);
        const rightStickY = Math.abs(axes[3]);
    
        if (rightStickX > threshold || rightStickY > threshold) {
            this.axesPressures.rightStickX = rightStickX;
            this.axesPressures.rightStickY = rightStickY;
    
            // If the aim tool is active, update its position
            if (game.mainSprite && game.mainSprite.targetAim) {
                const aimSpeed = 10; // Adjust aim speed as necessary
                const newTargetX = game.mainSprite.targetX + axes[2] * aimSpeed;
                const newTargetY = game.mainSprite.targetY + axes[3] * aimSpeed;
    
                // Calculate distance from the main sprite
                const deltaX = newTargetX - (game.mainSprite.x + game.mainSprite.width / 2);
                const deltaY = newTargetY - (game.mainSprite.y + game.mainSprite.height / 2);
                const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
    
                // If within maxRange, update targetX and targetY
                if (distance <= game.mainSprite.maxRange) {
                    game.mainSprite.targetX = newTargetX;
                    game.mainSprite.targetY = newTargetY;
                } else {
                    // Otherwise, set target to maxRange in the same direction
                    const angle = Math.atan2(deltaY, deltaX);
                    game.mainSprite.targetX = game.mainSprite.x + game.mainSprite.width / 2 + Math.cos(angle) * game.mainSprite.maxRange;
                    game.mainSprite.targetY = game.mainSprite.y + game.mainSprite.height / 2 + Math.sin(angle) * game.mainSprite.maxRange;
                }
    
                // Clamp the target position within the canvas bounds
                game.mainSprite.targetX = Math.max(0, Math.min(game.mainSprite.targetX, game.worldWidth));
                game.mainSprite.targetY = Math.max(0, Math.min(game.mainSprite.targetY, game.worldHeight));
            }
        } else {
            // Reset the pressures for right stick if below threshold
            this.axesPressures.rightStickX = 0;
            this.axesPressures.rightStickY = 0;
        }
    },    

    handleAButton: function(state, pressure) {

    },

    handleBButton: function(state, pressure) {
        if (state === "down") {
            input.rightClick({ preventDefault: () => {}, button: 2 });
        }

    },

    handleXButton: function(state, pressure) {

    },

    handleYButton: function(state, pressure) {

    },

    handleLeftBumper: function(state, pressure) {
        input.handleControlStateChange({ key: 'Shift' }, state === "down");

    },

    handleRightBumper: function(state, pressure) {
 
    },

    handleLeftTrigger: function(state, pressure) {
        // Check the pressure value
        if (pressure >= 0.5) {
            // If the pressure is halfway down or more, activate the aim tool
            if (!game.mainSprite.targetAim) {
                game.mainSprite.targetAim = true;
                game.mainSprite.targetX = game.mainSprite.x + game.mainSprite.width / 2;
                game.mainSprite.targetY = game.mainSprite.y + game.mainSprite.height / 2;
            }
        } else {
            // If the pressure is below halfway, deactivate the aim tool
            if (game.mainSprite.targetAim) {
                game.mainSprite.targetAim = false;
            }
        }
    },

    handleRightTrigger: function(state, pressure) {
        input.handleControlStateChange({ key: 'Alt' }, state === "down");

    },

    handleSelectButton: function(state, pressure) {

    },

    handleStartButton: function(state, pressure) {

    },

    handleLeftStickButton: function(state, pressure) {

    },

    handleRightStickButton: function(state, pressure) {

    },

    handleDPadUp: function(state, pressure) {
        gamepad.directions.up = (state === "down");
        input.updateSpriteDirections();

    },

    handleDPadDown: function(state, pressure) {
        gamepad.directions.down = (state === "down");
        input.updateSpriteDirections();

    },

    handleDPadLeft: function(state, pressure) {
        gamepad.directions.left = (state === "down");
        input.updateSpriteDirections();

    },

    handleDPadRight: function(state, pressure) {
        gamepad.directions.right = (state === "down");
        input.updateSpriteDirections();

    }
};
