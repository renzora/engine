var gamepad = {
    gamepadIndex: null,
    buttons: [],
    axes: [],
    isConnected: false,
    name: '',
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
        const buttonNames = this.getButtonNames();

        buttons.forEach((button, index) => {
            const pressure = button.value; // Pressure value ranges from 0 to 1
            this.buttonPressures[index] = pressure;

            const buttonName = buttonNames[index];

            if (button.pressed) {
                if (!this.buttons.includes(buttonName)) {
                    this.buttons.push(buttonName);
                    this.emitButtonEvent(index, "down", pressure);
                } else {
                    // Update the pressure continuously while the button is pressed
                    this.emitButtonEvent(index, "down", pressure);
                }
            } else {
                const buttonIndex = this.buttons.indexOf(buttonName);
                if (buttonIndex > -1) {
                    this.buttons.splice(buttonIndex, 1);
                    this.emitButtonEvent(index, "up", pressure);
                }
            }
        });
    },

    emitButtonEvent: function(buttonIndex, state, pressure) {
        const buttonNames = this.getButtonNames();
    
        if (typeof buttonIndex === 'number' && buttonNames[buttonIndex] !== undefined) {
            const buttonName = buttonNames[buttonIndex];
            const globalEventName = `gamepad${buttonName}${state === 'down' ? 'Pressed' : 'Released'}`;
    
            const eventDetail = { state, pressure, buttonName }; // Add buttonName to event detail
    
            // Emit global event
            const globalEvent = new CustomEvent(globalEventName, { detail: eventDetail });
            window.dispatchEvent(globalEvent);
    
            // Dynamically call the function on the active modal
            const activeModalId = modal.getActiveModal();
            const dynamicButtonName = buttonName + 'Button';
            if (activeModalId && window[activeModalId] && typeof window[activeModalId][dynamicButtonName] === 'function') {
                window[activeModalId][dynamicButtonName](pressure, state);
            }
        }
    },

    getButtonNames: function() {
        return [
            'a', 'b', 'x', 'y',
            'l1', 'r1',
            'l2', 'r2',
            'select', 'start',
            'leftStick', 'rightStick',
            'up', 'down', 'left', 'right'
        ];
    },

    vibrate: function(duration, strongMagnitude = 1.0, weakMagnitude = 1.0) {
        if (this.isConnected && this.gamepad && this.gamepad.vibrationActuator) {
            this.gamepad.vibrationActuator.playEffect("dual-rumble", {
                duration: duration,
                startDelay: 0,
                strongMagnitude: strongMagnitude,
                weakMagnitude: weakMagnitude
            }).catch(err => console.log('Vibration error: ', err));
        } else {
            console.log("Vibration not supported on this gamepad.");
        }
    }
};