var gamepad = {
    gamepadIndex: null,
    pressedButtons: [],
    axes: [],
    isConnected: false,
    
    buttonMap: {},
    
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
        console.log("Gamepad connected at index " + this.gamepadIndex);
    },
    
    disconnectGamepad: function(e) {
        if (e.gamepad.index === this.gamepadIndex) {
            this.isConnected = false;
            this.gamepadIndex = null;
            console.log("Gamepad disconnected from index " + this.gamepadIndex);
        }
    },
    
    updateGamepadState: function() {
        if (this.isConnected && this.gamepadIndex !== null) {
            const gamepad = navigator.getGamepads()[this.gamepadIndex];
            if (gamepad) {
                this.handleButtons(gamepad.buttons);
                this.handleAxes(gamepad.axes);
            }
        }
        requestAnimationFrame(() => this.updateGamepadState());
    },
    
    handleButtons: function(buttons) {
        buttons.forEach((button, index) => {
            if (button.pressed) {
                if (!this.pressedButtons.includes(index)) {
                    this.pressedButtons.push(index);
                    if (this.buttonMap[index]) this.buttonMap[index]("down");
                }
            } else {
                const buttonIndex = this.pressedButtons.indexOf(index);
                if (buttonIndex > -1) {
                    this.pressedButtons.splice(buttonIndex, 1);
                    if (this.buttonMap[index]) this.buttonMap[index]("up");
                }
            }
        });
    },
    
    handleAxes: function(axes) {
        const threshold = 0.2; // Dead zone threshold

        const directions = ['left', 'right', 'up', 'down'];
        directions.forEach(direction => input.removeDirection(direction)); // Reset directions

        if (Math.abs(axes[0]) > threshold || Math.abs(axes[1]) > threshold) {
            // Left stick movement (axes[0] = left/right, axes[1] = up/down)
            const directionX = axes[0] > threshold ? 'right' : (axes[0] < -threshold ? 'left' : null);
            const directionY = axes[1] > threshold ? 'down' : (axes[1] < -threshold ? 'up' : null);

            if (directionX) {
                input.addDirection(directionX);
            }
            if (directionY) {
                input.addDirection(directionY);
            }
        }
    },

    handleAButton: function(state) {

    },

    handleBButton: function(state) {
        if (state === "down") {
            input.rightClick({ preventDefault: () => {}, button: 2 });
        }
    },

    handleXButton: function(state) {
        // Handle X button press/release
    },

    handleYButton: function(state) {
        // Handle Y button press/release
    },

    handleLeftBumper: function(state) {
        input.handleControlStateChange({ key: 'Shift' }, state === "down");
    },

    handleRightBumper: function(state) {
        // Handle right bumper press/release
    },

    handleLeftTrigger: function(state) {
        input.handleControlStateChange({ key: 'Control' }, state === "down");
    },

    handleRightTrigger: function(state) {
        input.handleControlStateChange({ key: 'Alt' }, state === "down");
    },

    handleSelectButton: function(state) {
        // Handle select button press/release
    },

    handleStartButton: function(state) {
        // Handle start button press/release
    },

    handleLeftStickButton: function(state) {
        // Handle left stick button press/release
    },

    handleRightStickButton: function(state) {
        // Handle right stick button press/release
    },

    handleDPadUp: function(state) {
        if (state === "down") {
            input.addDirection('up');
        } else {
            input.removeDirection('up');
        }
    },

    handleDPadDown: function(state) {
        if (state === "down") {
            input.addDirection('down');
        } else {
            input.removeDirection('down');
        }
    },

    handleDPadLeft: function(state) {
        if (state === "down") {
            input.addDirection('left');
        } else {
            input.removeDirection('left');
        }
    },

    handleDPadRight: function(state) {
        if (state === "down") {
            input.addDirection('right');
        } else {
            input.removeDirection('right');
        }
    }
};
