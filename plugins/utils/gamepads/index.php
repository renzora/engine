<div class="window bg-gray-900 w-full max-w-md rounded shadow-lg">
    <div data-part="handle" class="window_title bg-gray-800 text-gray-100 p-4 rounded-t flex justify-between items-center">
        <span class="text-lg font-semibold">Controller Assignment</span>
        <button class="icon close_dark text-white text-xl" aria-label="Close (ESC)" data-close>&times;</button>
    </div>

    <div class="relative bg-gray-700 text-white p-4">
        <h3 class="text-lg font-medium mb-4 text-center">Select a Controller</h3>
        <div id="controllerMenu" class="flex flex-wrap justify-center gap-4 mb-6">
            <!-- Dynamic controller visuals will be populated here -->
        </div>
        <h3 class="text-lg font-medium mb-4 text-center">Assigned Slot</h3>
        <div id="gamepadSlots" class="flex justify-center gap-4">
            <!-- Gamepad slot rendered here -->
        </div>
        <p class="mt-6 text-sm text-gray-400 text-center">
            Use <strong class="text-gray-200">Left</strong> and <strong class="text-gray-200">Right</strong> to navigate, 
            and <strong class="text-gray-200">A</strong> to assign the controller.
        </p>
    </div>
</div>
</div>

<script>
window[id] = {
    id: id,
    currentIndex: 0, // Index of the currently highlighted controller in the filtered list
    throttleDuration: 200, // Throttle duration in milliseconds
    lastButtonPress: 0, // Timestamp of the last button press

    start: function() {
        this.renderControllerMenu();
        this.renderGamepadSlots();
        this.addClickListeners();
        this.addGamepadListeners(); // Add gamepad connection listeners
        this.showControllersOnLoad(); // Ensure controllers are displayed on load
    },

    getConnectedGamepads: function() {
        // Filter out null gamepads from navigator.getGamepads()
        return Array.from(navigator.getGamepads()).filter(gamepad => gamepad !== null);
    },

    renderControllerMenu: function() {
        const menuContainer = document.getElementById('controllerMenu');
        const gamepads = this.getConnectedGamepads();
        menuContainer.innerHTML = '';

        gamepads.forEach((gamepadObj, index) => {
            const isSelected = index === this.currentIndex ? 'bg-blue-600' : 'bg-gray-600';
            const controllerName = gamepad.getGamepadName(gamepadObj); // Use gamepad.getGamepadName for controller name

            const controllerVisual = `
                <div class="controller-visual border-2 p-4 rounded text-center text-white ${isSelected}"
                     id="controller-${index}" data-index="${index}">
                    <p class="font-bold">${controllerName}</p>
                </div>`;
            menuContainer.innerHTML += controllerVisual;
        });
    },

    renderGamepadSlots: function() {
        const slotsContainer = document.getElementById('gamepadSlots');
        const activeController = navigator.getGamepads()[gamepad.gamepadIndex];
        const controllerName = activeController ? gamepad.getGamepadName(activeController) : 'None';

        slotsContainer.innerHTML = `
            <div class="gamepad-slot border-2 border-gray-600 w-32 h-32 rounded bg-gray-800 text-center text-gray-400 p-2">
                <p class="font-semibold">Assigned Controller</p>
                <div class="slot-visual mt-4 text-gray-100">${controllerName}</div>
            </div>`;
    },

    addClickListeners: function() {
        const menuContainer = document.getElementById('controllerMenu');
        menuContainer.addEventListener('click', this.handleControllerClick.bind(this));
    },

    addGamepadListeners: function() {
        window.addEventListener('gamepadconnected', this.showControllersOnLoad.bind(this));
        window.addEventListener('gamepaddisconnected', this.showControllersOnLoad.bind(this));
    },

    handleControllerClick: function(event) {
        const target = event.target.closest('.controller-visual');
        if (target && target.dataset.index) {
            const newIndex = parseInt(target.dataset.index, 10);
            this.currentIndex = newIndex;
            this.renderControllerMenu();
            this.assignController();
        }
    },

    throttle: function(callback) {
        const currentTime = Date.now();
        if (currentTime - this.lastButtonPress < this.throttleDuration) {
            return false;
        }
        this.lastButtonPress = currentTime;
        callback();
        return true;
    },

    assignController: function() {
        const gamepads = this.getConnectedGamepads();
        const selectedGamepad = gamepads[this.currentIndex];
        if (selectedGamepad) {
            gamepad.gamepadIndex = selectedGamepad.index; // Update gamepad.gamepadIndex
            console.log(`Gamepad ${gamepad.gamepadIndex + 1} assigned as the active controller.`);
            this.renderControllerMenu();
            this.renderGamepadSlots();
        }
    },

    showControllersOnLoad: function() {
        const gamepads = this.getConnectedGamepads();
        if (gamepads.length > 0) {
            this.currentIndex = 0; // Default to the first controller
            this.renderControllerMenu();
            this.renderGamepadSlots();
        } else {
            document.getElementById('controllerMenu').innerHTML = '<p class="text-gray-400">No controllers connected.</p>';
            document.getElementById('gamepadSlots').innerHTML = '';
        }
    },

    leftButton: function() {
        this.throttle(() => {
            const gamepads = this.getConnectedGamepads();
            this.currentIndex = Math.max(0, this.currentIndex - 1);
            this.renderControllerMenu();
        });
    },

    rightButton: function() {
        this.throttle(() => {
            const gamepads = this.getConnectedGamepads();
            this.currentIndex = Math.min(gamepads.length - 1, this.currentIndex + 1);
            this.renderControllerMenu();
        });
    },

    aButton: function() {
        this.throttle(() => {
            this.assignController();
        });
    },

    unmount: function() {
        const menuContainer = document.getElementById('controllerMenu');
        menuContainer.removeEventListener('click', this.handleControllerClick.bind(this));
        window.removeEventListener('gamepadconnected', this.showControllersOnLoad.bind(this));
        window.removeEventListener('gamepaddisconnected', this.showControllersOnLoad.bind(this));
        console.log('Gamepads window unmounted.');
    }
};
</script>