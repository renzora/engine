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

        this.findNearestTarget = this.findNearestTarget.bind(this);
    },

    connectGamepad: function(e) {
        this.gamepadIndex = e.gamepad.index;
        this.isConnected = true;
        this.name = this.getGamepadName(e.gamepad); // Store the gamepad name
        game.updateInputMethod('gamepad', this.name);
        console.log("Gamepad connected at index " + this.gamepadIndex);

        // Emit custom event for gamepad connection
        const event = new CustomEvent('gamepadConnected');
        window.dispatchEvent(event);
    },

    disconnectGamepad: function(e) {
        if (e.gamepad.index === this.gamepadIndex) {
            this.isConnected = false;
            this.gamepadIndex = null;
            game.updateInputMethod('keyboard'); // Revert to keyboard input method when gamepad is disconnected
            console.log("Gamepad disconnected from index " + this.gamepadIndex);

            // Emit custom event for gamepad disconnection
            const event = new CustomEvent('gamepadDisconnected');
            window.dispatchEvent(event);
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
    
                // Update the aim tool position if L2 is held down
                if (game.mainSprite && game.mainSprite.targetAim && this.leftStickMoved) {
                    this.updateAimToolPosition();
                    this.leftStickMoved = false;
                }
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
                    this.emitButtonEvent(index, "down", pressure);
                } else {
                    // Update the pressure continuously while the button is pressed
                    if (this.buttonMap[index]) this.buttonMap[index]("down", pressure);
                    this.emitButtonEvent(index, "down", pressure);
                }
            } else {
                const buttonIndex = this.pressedButtons.indexOf(index);
                if (buttonIndex > -1) {
                    this.pressedButtons.splice(buttonIndex, 1);
                    if (this.buttonMap[index]) this.buttonMap[index]("up", pressure);
                    this.emitButtonEvent(index, "up", pressure);
                }
            }
        });
    },

    emitButtonEvent: function(buttonIndex, state, pressure) {
        const buttonNames = [
            'A', 'B', 'X', 'Y',
            'LeftBumper', 'RightBumper',
            'LeftTrigger', 'RightTrigger',
            'Select', 'Start',
            'LeftStick', 'RightStick',
            'DPadUp', 'DPadDown', 'DPadLeft', 'DPadRight'
        ];

        const eventName = `gamepad${buttonNames[buttonIndex]}${state === 'down' ? 'Pressed' : 'Released'}`;
        const event = new CustomEvent(eventName, {
            detail: { state, pressure }
        });
        window.dispatchEvent(event);
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
    
                // Update sprite direction based on aim tool position
                const angle = Math.atan2(deltaY, deltaX);
                if (angle >= -Math.PI / 8 && angle < Math.PI / 8) {
                    game.mainSprite.direction = 'E';
                } else if (angle >= Math.PI / 8 && angle < 3 * Math.PI / 8) {
                    game.mainSprite.direction = 'SE';
                } else if (angle >= 3 * Math.PI / 8 && angle < 5 * Math.PI / 8) {
                    game.mainSprite.direction = 'S';
                } else if (angle >= 5 * Math.PI / 8 && angle < 7 * Math.PI / 8) {
                    game.mainSprite.direction = 'SW';
                } else if (angle >= 7 * Math.PI / 8 || angle < -7 * Math.PI / 8) {
                    game.mainSprite.direction = 'W';
                } else if (angle >= -7 * Math.PI / 8 && angle < -5 * Math.PI / 8) {
                    game.mainSprite.direction = 'NW';
                } else if (angle >= -5 * Math.PI / 8 && angle < -3 * Math.PI / 8) {
                    game.mainSprite.direction = 'N';
                } else if (angle >= -3 * Math.PI / 8 && angle < -Math.PI / 8) {
                    game.mainSprite.direction = 'NE';
                }
    
                // If within maxRange, update targetX and targetY
                if (distance <= game.mainSprite.maxRange) {
                    game.mainSprite.targetX = newTargetX;
                    game.mainSprite.targetY = newTargetY;
                } else {
                    // Otherwise, set target to maxRange in the same direction
                    const maxRangeX = game.mainSprite.x + game.mainSprite.width / 2 + Math.cos(angle) * game.mainSprite.maxRange;
                    const maxRangeY = game.mainSprite.y + game.mainSprite.height / 2 + Math.sin(angle) * game.mainSprite.maxRange;
                    game.mainSprite.targetX = Math.max(0, Math.min(maxRangeX, game.worldWidth));
                    game.mainSprite.targetY = Math.max(0, Math.min(maxRangeY, game.worldHeight));
                }
            }
        } else {
            // Reset the pressures for right stick if below threshold
            this.axesPressures.rightStickX = 0;
            this.axesPressures.rightStickY = 0;
        }

        this.emitButtonEvent('RightStickMoved', { deltaX: rightStickX, deltaY: rightStickY });
        
    },    

    handleAButton: function(state, pressure) {
        this.emitButtonEvent(0, state, pressure);
    },

    handleBButton: function(state, pressure) {
        if (state === "down") {
            input.rightClick({ preventDefault: () => {}, button: 2 });
        }
        this.emitButtonEvent(1, state, pressure);
    },

    handleXButton: function(state, pressure) {
        this.emitButtonEvent(2, state, pressure);
    },

    handleYButton: function(state, pressure) {
        this.emitButtonEvent(3, state, pressure);
    },

    handleLeftBumper: function(state, pressure) {
        const gamepad = navigator.getGamepads()[this.gamepadIndex];
        const l2Button = gamepad.buttons[6]; // Assuming L2 is at index 6
    
        if (state === "down" && l2Button.value >= 0.5) {

            const player = game.mainSprite;
            const nearestTarget = this.findNearestTarget(player.targetX, player.targetY, player.maxRange);
    
            if (nearestTarget) {
                const targetCenterX = nearestTarget.x + (nearestTarget.width ? nearestTarget.width / 2 : 0);
                const targetCenterY = nearestTarget.y + (nearestTarget.height ? nearestTarget.height / 2 : 0);
                if (this.isWithinMaxRange(nearestTarget, player)) {
                    player.targetX = targetCenterX;
                    player.targetY = targetCenterY;
                    player.targetAim = true;
                } else {
                    // If the nearest target is not within max range, set the aim tool position in the same direction
                    const playerCenterX = player.x + player.width / 2;
                    const playerCenterY = player.y + player.height / 2;
                    const deltaX = targetCenterX - playerCenterX;
                    const deltaY = targetCenterY - playerCenterY;
                    const angle = Math.atan2(deltaY, deltaX);
                    player.targetX = playerCenterX + Math.cos(angle) * player.maxRange;
                    player.targetY = playerCenterY + Math.sin(angle) * player.maxRange;
                    player.targetAim = true;
                }
            } else {
                player.targetAim = false;
            }
        } else if (state === "up") {
            game.mainSprite.targetAim = false;
        }
        this.emitButtonEvent(4, state, pressure);
    },           

    handleRightBumper: function(state, pressure) {
        this.emitButtonEvent(5, state, pressure);
    },

    handleLeftTrigger: function(state, pressure) {
        // Check the pressure value
        if (pressure >= 0.5) {
            // If the pressure is halfway down or more, activate the aim tool
            if (!game.mainSprite.targetAim) {
                game.mainSprite.targetAim = true;
                // Initialize the target position slightly ahead of the sprite's direction
                this.updateAimToolPosition();
            }
        } else {
            // If the pressure is below halfway, deactivate the aim tool
            if (game.mainSprite.targetAim) {
                game.mainSprite.targetAim = false;
            }
        }
        this.emitButtonEvent(6, state, pressure);
    },

    isWithinMaxRange: function(target, player) {
        const targetCenterX = target.x + (target.width ? target.width / 2 : 0);
        const targetCenterY = target.y + (target.height ? target.height / 2 : 0);
        const playerCenterX = player.x + player.width / 2;
        const playerCenterY = player.y + player.height / 2;
        const deltaX = targetCenterX - playerCenterX;
        const deltaY = targetCenterY - playerCenterY;
        const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
        return distance <= player.maxRange;
    },    

    updateAimToolPosition: function() {
        const sprite = game.mainSprite;
        const aheadDistance = 30; // Distance ahead of the sprite to set the aim tool
        const directionOffsets = {
            'N': { x: 0, y: -aheadDistance },
            'S': { x: 0, y: aheadDistance },
            'E': { x: aheadDistance, y: 0 },
            'W': { x: -aheadDistance, y: 0 },
            'NE': { x: aheadDistance / Math.sqrt(2), y: -aheadDistance / Math.sqrt(2) },
            'NW': { x: -aheadDistance / Math.sqrt(2), y: -aheadDistance / Math.sqrt(2) },
            'SE': { x: aheadDistance / Math.sqrt(2), y: aheadDistance / Math.sqrt(2) },
            'SW': { x: -aheadDistance / Math.sqrt(2), y: aheadDistance / Math.sqrt(2) }
        };
    
        const offset = directionOffsets[sprite.direction] || { x: 0, y: 0 };
    
        sprite.targetX = sprite.x + sprite.width / 2 + offset.x;
        sprite.targetY = sprite.y + sprite.height / 2 + offset.y;
    
        // Clamp the target position within the canvas bounds
        sprite.targetX = Math.max(0, Math.min(sprite.targetX, game.worldWidth));
        sprite.targetY = Math.max(0, Math.min(sprite.targetY, game.worldHeight));
    },    

    // Add the new method here
    findNearestTarget: function(centerX, centerY, maxRadius) {
        let nearestTarget = null;
        let nearestDistance = Infinity;
    
        // Check sprites (enemies)
        for (let id in game.sprites) {
            const sprite = game.sprites[id];
            if (sprite.isEnemy) {
                const spriteCenterX = sprite.x + sprite.width / 2;
                const spriteCenterY = sprite.y + sprite.height / 2;
                const distance = Math.sqrt(
                    (centerX - spriteCenterX) ** 2 +
                    (centerY - spriteCenterY) ** 2
                );
                if (distance < nearestDistance && distance <= maxRadius) {
                    nearestDistance = distance;
                    nearestTarget = sprite;
                }
            }
        }
    
        // Check objects
        if (game.roomData && game.roomData.items) {
            game.roomData.items.forEach(item => {
                const itemData = assets.load('objectData')[item.id];
                if (itemData) {
                    const itemCenterX = item.x[0] * 16 + 8; // Center X coordinate
                    const itemCenterY = item.y[0] * 16 + 8; // Center Y coordinate
                    const distance = Math.sqrt(
                        (centerX - itemCenterX) ** 2 +
                        (centerY - itemCenterY) ** 2
                    );
                    if (distance < nearestDistance && distance <= maxRadius) {
                        nearestDistance = distance;
                        nearestTarget = { ...item, x: itemCenterX, y: itemCenterY }; // Include center coordinates
                    }
                }
            });
        }
    
        return nearestTarget;
    },      

    handleRightTrigger: function(state, pressure) {
        input.handleControlStateChange({ key: 'Alt' }, state === "down");
        this.emitButtonEvent(7, state, pressure);
    },

    handleSelectButton: function(state, pressure) {
        this.emitButtonEvent(8, state, pressure);
    },

    handleStartButton: function(state, pressure) {
        this.emitButtonEvent(9, state, pressure);
    },

    handleLeftStickButton: function(state, pressure) {
        this.emitButtonEvent(10, state, pressure);
    },

    handleRightStickButton: function(state, pressure) {
        this.emitButtonEvent(11, state, pressure);
    },

    handleDPadUp: function(state, pressure) {
        gamepad.directions.up = (state === "down");
        input.updateSpriteDirections();
        this.emitButtonEvent(12, state, pressure);
    },

    handleDPadDown: function(state, pressure) {
        gamepad.directions.down = (state === "down");
        input.updateSpriteDirections();
        this.emitButtonEvent(13, state, pressure);
    },

    handleDPadLeft: function(state, pressure) {
        gamepad.directions.left = (state === "down");
        input.updateSpriteDirections();
        this.emitButtonEvent(14, state, pressure);
    },

    handleDPadRight: function(state, pressure) {
        gamepad.directions.right = (state === "down");
        input.updateSpriteDirections();
        this.emitButtonEvent(15, state, pressure);
    }
};
