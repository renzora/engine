var gamepad = {
    gamepadIndex: null,
    buttons: [],
    axes: [],
    isConnected: false,
    name: '',
    buttonPressures: {},
    axesPressures: {},
    throttledEvents: {},
    buttonOverwrite: {
        leftButton: null,
        rightButton: null,
        aButton: null,
        bButton: null
    },
    playerLimit: 2, // Maximum number of players that can connect
    assignedControllers: {}, // Stores the mapping of player to controller

    spritesheetButtonMap: {
        ps5: ['up', 'left', 'down', 'right', 'leftstick', 'leftstickpressed', 'rightstick', 'rightstickpressed', 'x', 'square', 'circle', 'triangle', 'l1', 'l2', 'r1', 'r2'],
        xbox: ['up', 'left', 'down', 'right', 'leftstick', 'leftstickpressed', 'rightstick', 'rightstickpressed', 'a', 'x', 'b', 'y', 'lb', 'lt', 'rb', 'rt'],
        switch: ['up', 'left', 'down', 'right', 'leftstick', 'leftstickpressed', 'rightstick', 'rightstickpressed', 'b', 'y', 'a', 'x', 'l', 'zl', 'r', 'zr']
    },

    defaultPlatform: 'ps5',
    button_size: 16, // Original button size in the spritesheet
    display_size: 32, // Updated display size in pixels

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
        this.name = this.getGamepadName(e.gamepad);
        input.updateInputMethod('gamepad', this.name);
        const event = new CustomEvent('gamepadConnected');
        window.dispatchEvent(event);
    },

    disconnectGamepad: function(e) {
        if (e.gamepad.index === this.gamepadIndex) {
            this.isConnected = false;
            this.gamepadIndex = null;
            input.updateInputMethod('keyboard');
            console.log("Gamepad disconnected from index " + this.gamepadIndex);
            const event = new CustomEvent('gamepadDisconnected');
            window.dispatchEvent(event);
        }
    },

    assignController: function(player, gamepadIndex) {
        if (Object.keys(this.assignedControllers).length < this.playerLimit) {
            this.assignedControllers[player] = gamepadIndex;
        } else {
            console.log("Player limit reached. Cannot assign more controllers.");
        }
    },

    unassignController: function(player) {
        if (this.assignedControllers[player]) {
            delete this.assignedControllers[player];
        }
    },

    updateGamepadState: function() {
        if (this.isConnected && this.gamepadIndex !== null) {
            const gamepad = navigator.getGamepads()[this.gamepadIndex];
            if (gamepad) {
                if (this.hasActiveInput(gamepad)) {
                    input.updateInputMethod('gamepad', this.name);
                }
                this.handleButtons(gamepad.buttons);
                const axesEvent = new CustomEvent('gamepadAxes', { detail: gamepad.axes });
                window.dispatchEvent(axesEvent);
            }
        }
    },

    hasActiveInput: function(gamepad) {
        const threshold = 0.2;
        const buttonsPressed = gamepad.buttons.some(button => button.pressed);
        const axesMoved = gamepad.axes.some(axis => Math.abs(axis) > threshold);
        return buttonsPressed || axesMoved;
    },

    getGamepadName: function(gamepad) {
        console.log(gamepad);
        const vendorProductMapping = {
            '045e:02e0': 'Xbox 360',
            '045e:028e': 'Xbox One',
            '054c:0ce6': 'PS5',
            '054c:05c4': 'PS4',
            '057e:2009': 'Nintendo Switch Pro Controller',
            '046d:c216': 'Logitech F310',
            '1038:1412': 'SteelSeries Stratus Duo'
        };
    
        const id = gamepad.id;
    
        // Extract Vendor ID and Product ID from the id string
        const match = id.match(/Vendor:\s*([0-9a-fA-F]{4})\s*Product:\s*([0-9a-fA-F]{4})/);
        if (match) {
            const vendorProduct = `${match[1].toLowerCase()}:${match[2].toLowerCase()}`;
            if (vendorProductMapping[vendorProduct]) {
                return vendorProductMapping[vendorProduct];
            }
        }
    
        // Fallback to generic matching if no direct Vendor ID/Product ID match is found
        if (id.toLowerCase().includes('xbox')) {
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
            const pressure = button.value;
            this.buttonPressures[index] = pressure;

            const buttonName = buttonNames[index];

            if (button.pressed) {
                if (!this.buttons.includes(buttonName)) {
                    this.buttons.push(buttonName);
                    this.emitButtonEvent(index, "down", pressure);
                } else {
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
    
            const eventDetail = { state, pressure, buttonName };
            const globalEvent = new CustomEvent(globalEventName, { detail: eventDetail });
            window.dispatchEvent(globalEvent);
    
            const activeModalId = modal.getActiveModal();
            if (activeModalId && window[activeModalId]) {
                const dynamicButtonName = buttonName + 'Button';
                const dynamicReleasedName = buttonName + 'ButtonReleased';
    
                if (state === 'down' && typeof window[activeModalId][dynamicButtonName] === 'function') {
                    window[activeModalId][dynamicButtonName](pressure, state);
                } else if (state === 'up' && typeof window[activeModalId][dynamicReleasedName] === 'function') {
                    window[activeModalId][dynamicReleasedName](pressure, state);
                }
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
    },

    updateButtonImages: function() {
        const spritesheet = assets.use('gamepad_buttons');
        let platform = this.name || this.defaultPlatform;
    
        // Fallback to 'ps5' if platform is not in the map
        if (!this.spritesheetButtonMap[platform]) {
            platform = this.defaultPlatform;
        }
    
        const platformButtons = this.spritesheetButtonMap[platform];
        const platformRow = this.getPlatformRow(platform);
        const scaleFactor = this.display_size / this.button_size;
    
        if (!platformButtons) {
            console.error(`No buttons mapped for platform: ${platform}`);
            return;
        }
    
        platformButtons.forEach((buttonName, index) => {
            const buttonElements = document.querySelectorAll(`.gamepad_button_${buttonName}`);
            const x = index * this.button_size; // X position (column)
            const y = platformRow * this.button_size; // Y position (row)
    
            buttonElements.forEach(element => {
                element.style.width = `${this.display_size}px`;
                element.style.height = `${this.display_size}px`;
                element.style.backgroundImage = `url('${spritesheet.src}')`;
                element.style.backgroundPosition = `-${x * scaleFactor}px -${y * scaleFactor}px`;
                element.style.backgroundSize = `${this.button_size * 16 * scaleFactor}px ${this.button_size * 3 * scaleFactor}px`;
                element.style.backgroundRepeat = 'no-repeat';
                element.style.display = 'inline-block';
            });
        });
    },    

    clearButtonImages: function() {
        const buttonNames = this.spritesheetButtonMap.ps5;
        buttonNames.forEach(buttonName => {
            const buttonElements = document.querySelectorAll(`.gamepad_button_${buttonName}`);
            buttonElements.forEach(element => {
                element.style.backgroundImage = '';
            });
        });
    },

    getPlatformRow: function(platform) {
        const rowMap = { ps5: 0, xbox: 1, switch: 2 };
        return rowMap[platform] !== undefined ? rowMap[platform] : 0;
    }
};