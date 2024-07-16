var gamepad = {
    gamepadIndex: null,
    pressedButtons: [],
    axes: [],
    isConnected: false,

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

    handleAxes: function(axes) {
        const threshold = 0.2; // Dead zone threshold

        // Calculate axis pressures
        const leftStickX = Math.abs(axes[0]);
        const leftStickY = Math.abs(axes[1]);
        const rightStickX = Math.abs(axes[2]);
        const rightStickY = Math.abs(axes[3]);

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

            console.log("Left Stick X Pressure: " + this.axesPressures.leftStickX);
            console.log("Left Stick Y Pressure: " + this.axesPressures.leftStickY);
        } else {
            // Reset the directions for left stick if below threshold
            this.axesPressures.leftStickX = 0;
            this.axesPressures.leftStickY = 0;
        }

        if (rightStickX > threshold || rightStickY > threshold) {
            this.axesPressures.rightStickX = rightStickX;
            this.axesPressures.rightStickY = rightStickY;

            console.log("Right Stick X Pressure: " + this.axesPressures.rightStickX);
            console.log("Right Stick Y Pressure: " + this.axesPressures.rightStickY);
        } else {
            // Reset the pressures for right stick if below threshold
            this.axesPressures.rightStickX = 0;
            this.axesPressures.rightStickY = 0;
        }

        // Update the sprite's directions based on combined states
        input.updateSpriteDirections();
    },

    handleAButton: function(state, pressure) {
        console.log("A Button " + state + " with pressure " + pressure);
    },

    handleBButton: function(state, pressure) {
        if (state === "down") {
            input.rightClick({ preventDefault: () => {}, button: 2 });
        }
        console.log("B Button " + state + " with pressure " + pressure);
    },

    handleXButton: function(state, pressure) {
        console.log("X Button " + state + " with pressure " + pressure);
    },

    handleYButton: function(state, pressure) {
        console.log("Y Button " + state + " with pressure " + pressure);
    },

    handleLeftBumper: function(state, pressure) {
        input.handleControlStateChange({ key: 'Shift' }, state === "down");
        console.log("Left Bumper " + state + " with pressure " + pressure);
    },

    handleRightBumper: function(state, pressure) {
        console.log("Right Bumper " + state + " with pressure " + pressure);
    },

    handleLeftTrigger: function(state, pressure) {
        input.handleControlStateChange({ key: 'Control' }, state === "down");
        console.log("Left Trigger " + state + " with pressure " + pressure);
    },

    handleRightTrigger: function(state, pressure) {
        input.handleControlStateChange({ key: 'Alt' }, state === "down");
        console.log("Right Trigger " + state + " with pressure " + pressure);
    },

    handleSelectButton: function(state, pressure) {
        console.log("Select Button " + state + " with pressure " + pressure);
    },

    handleStartButton: function(state, pressure) {
        console.log("Start Button " + state + " with pressure " + pressure);
    },

    handleLeftStickButton: function(state, pressure) {
        console.log("Left Stick Button " + state + " with pressure " + pressure);
    },

    handleRightStickButton: function(state, pressure) {
        console.log("Right Stick Button " + state + " with pressure " + pressure);
    },

    handleDPadUp: function(state, pressure) {
        gamepad.directions.up = (state === "down");
        input.updateSpriteDirections();
        console.log("D-Pad Up " + state + " with pressure " + pressure);
    },

    handleDPadDown: function(state, pressure) {
        gamepad.directions.down = (state === "down");
        input.updateSpriteDirections();
        console.log("D-Pad Down " + state + " with pressure " + pressure);
    },

    handleDPadLeft: function(state, pressure) {
        gamepad.directions.left = (state === "down");
        input.updateSpriteDirections();
        console.log("D-Pad Left " + state + " with pressure " + pressure);
    },

    handleDPadRight: function(state, pressure) {
        gamepad.directions.right = (state === "down");
        input.updateSpriteDirections();
        console.log("D-Pad Right " + state + " with pressure " + pressure);
    }
};