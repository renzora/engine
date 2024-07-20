var gamepad = {
    gamepadIndex: null,
    pressedButtons: [],
    axes: [],
    isConnected: false,
    name: '',
    buttonMap: {},
    buttonPressures: {},
    axesPressures: {},
    throttledEvents: {},

    init: function() {
        window.addEventListener("gamepadconnected", (e) => this.connectGamepad(e));
        window.addEventListener("gamepaddisconnected", (e) => this.disconnectGamepad(e));
        this.updateGamepadState();
    },

    throttle: function(func, delay) {
        return (...args) => {
            const now = Date.now();
            const lastCall = this.throttledEvents[func] || 0;
            if (now - lastCall >= delay) {
                this.throttledEvents[func] = now;
                func(...args);
            }
        };
    },

    connectGamepad: function(e) {
        this.gamepadIndex = e.gamepad.index;
        this.isConnected = true;
        this.name = this.getGamepadName(e.gamepad); // Store the gamepad name
        game.updateInputMethod('gamepad', this.name);
        modal.minimize('console_window');
        modal.front('ui_inventory_window');
        const event = new CustomEvent('gamepadConnected');
        window.dispatchEvent(event);
    },

    disconnectGamepad: function(e) {
        if (e.gamepad.index === this.gamepadIndex) {
            this.isConnected = false;
            this.gamepadIndex = null;
            game.updateInputMethod('keyboard'); // Revert to keyboard input method when gamepad is disconnected
            console.log("Gamepad disconnected from index " + this.gamepadIndex);
            modal.show('console_window');

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

                // Emit events for left and right axes
                const axesEvent = new CustomEvent('gamepadAxes', { detail: gamepad.axes });
                window.dispatchEvent(axesEvent);
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
                    this.emitButtonEvent(index, "down", pressure);
                } else {
                    // Update the pressure continuously while the button is pressed
                    this.emitButtonEvent(index, "down", pressure);
                }
            } else {
                const buttonIndex = this.pressedButtons.indexOf(index);
                if (buttonIndex > -1) {
                    this.pressedButtons.splice(buttonIndex, 1);
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

        if (typeof buttonIndex === 'number' && buttonNames[buttonIndex] !== undefined) {
            const buttonName = buttonNames[buttonIndex];
            const globalEventName = `gamepad${buttonName}${state === 'down' ? 'Pressed' : 'Released'}`;

            const eventDetail = { state, pressure };

            // Emit global event
            const globalEvent = new CustomEvent(globalEventName, { detail: eventDetail });
            window.dispatchEvent(globalEvent);

            // Dynamically call the function on the active modal
            const activeModalId = modal.getActiveModal();
            if (activeModalId && window[activeModalId] && typeof window[activeModalId][buttonName] === 'function') {
                window[activeModalId][buttonName](pressure, state);
            }
        }
    }
};
