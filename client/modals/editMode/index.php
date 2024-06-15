<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='edit_mode_window' class='window window_bg position-fixed top-2 justify-center flex' style='width: 600px; height: 47px; background: #3a445b; border-radius: 0;'>

    <!-- Handle that spans the whole left side -->
    <div data-part='handle' class='window_title rounded-none' style='width: 20px; background-image: radial-gradient(#e5e5e58a 1px, transparent 0) !important; border-radius: 0;'>
    </div>

    <!-- Rest of the content -->
    <div class='relative flex-grow'>
      <div class='container text-light window_body p-2 mx-2'>
        <button type="button" id="items_button" class="mode-button shadow appearance-none border rounded py-2 px-3 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #276b49; border: 1px rgba(0,0,0,0.5) solid;">Inventory</button>
        <button type="button" id="select_button" class="mode-button shadow appearance-none border rounded py-2 px-3 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #276b49; border: 1px rgba(0,0,0,0.5) solid;">Select</button>
        <button type="button" id="move_button" class="mode-button shadow appearance-none border rounded py-2 px-3 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #276b49; border: 1px rgba(0,0,0,0.5) solid;">Move</button>
        <button type="button" id="pickup_button" class="mode-button shadow appearance-none border rounded py-2 px-3 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #276b49; border: 1px rgba(0,0,0,0.5) solid;">Pick Up</button>
        <button type="button" id="drop_button" class="mode-button shadow appearance-none border rounded py-2 px-3 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #276b49; border: 1px rgba(0,0,0,0.5) solid;">Drop</button>
        <button type="button" id="navigate_button" class="mode-button shadow appearance-none border rounded py-2 px-3 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #276b49; border: 1px rgba(0,0,0,0.5) solid;">Pan</button>
        <button type="button" class="mode-button shadow appearance-none border rounded py-2 px-3 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #276b49; border: 1px rgba(0,0,0,0.5) solid;" onclick="modal.load('renadmin/tileset.php','renadmin_tileset_manager')">Manager</button>
      </div>
    </div>

    <!-- Close button on the right -->
    <button class="icon close_dark hint--right ml-auto" aria-label="Close (ESC)" data-close></button>
  </div>

  <style>
    body.move-cursor {
      cursor: move !important;
    }
  </style>

  <script>
var edit_mode_window = {
    currentMode: null,
    history: [],
    redoStack: [],
    selectionStart: null,
    selectionEnd: null,
    isSelecting: false,
    selectedTiles: [],
    tempSelectedTiles: [],
    modeChangeHandlers: {},
    Modes: {
        ITEMS: 'items',
        SELECT: 'select',
        MOVE: 'move',
        PICKUP: 'pickup',
        DROP: 'drop',
        NAVIGATE: 'navigate'
    },
    modeButtons: {},

    start: function() {
        this.modeButtons = {
            items: document.getElementById('items_button'),
            select: document.getElementById('select_button'),
            move: document.getElementById('move_button'),
            pickup: document.getElementById('pickup_button'),
            drop: document.getElementById('drop_button'),
            navigate: document.getElementById('navigate_button')
        };

        modal.hide('ui_window');
        game.isEditMode = true;
        this.currentMode = null;
        this.changeMode(this.Modes.SELECT);

        Object.keys(this.modeButtons).forEach(mode => {
            var handler = () => this.changeMode(mode);
            this.modeChangeHandlers[mode] = handler;
            this.modeButtons[mode].addEventListener('click', handler.bind(this));
        });

        this.initEventListeners();
    },

    unmount: function() {
        modal.show('ui_window');
        game.zoomLevel = 4;
        game.isEditMode = false;
        this.currentMode = null;

        // Reset editor data
        this.selectedTiles = [];
        this.selectionStart = null;
        this.selectionEnd = null;
        this.history = [];
        this.redoStack = [];
        this.tempSelectedTiles = [];

        // Remove mode button event listeners
        Object.keys(this.modeButtons).forEach(mode => {
            var handler = this.modeChangeHandlers[mode];
            if (handler) {
                this.modeButtons[mode].removeEventListener('click', handler);
            }
        });

        // Clear mode buttons and handlers
        this.modeButtons = {};
        this.modeChangeHandlers = {};

        this.removeEventListeners();
    },

    changeMode: function(newMode) {
        // Reset styles for all buttons
        Object.values(this.modeButtons).forEach(button => {
            button.style.background = '#276b49';
            button.style.color = 'white'; // Reset text color if changed
        });

        // Highlight the active mode button
        if (this.modeButtons[newMode]) {
            this.modeButtons[newMode].style.background = 'white';
            this.modeButtons[newMode].style.color = '#276b49'; // Change text color if needed
        }

        // Handle mode-specific actions
        if (newMode === this.Modes.ITEMS) {
            modal.load('inventory');
        } else {
            modal.hide('inventory_window');
        }

        // Set the editor's current mode
        this.currentMode = newMode;
        console.log(`Current mode: ${newMode}`);
    },

    undo: function() {
        if (this.history.length > 0) {
            const lastState = this.history.pop();
            this.redoStack.push([...this.selectedTiles]);
            this.selectedTiles = lastState;
            console.log("Undo performed. Selected tiles:", this.selectedTiles);
        }
    },

    redo: function() {
        if (this.redoStack.length > 0) {
            const nextState = this.redoStack.pop();
            this.history.push([...this.selectedTiles]);
            this.selectedTiles = nextState;
            console.log("Redo performed. Selected tiles:", this.selectedTiles);
        }
    },

    updateSelectedTiles: function(isLineSelect = false) {
        let newSelection = [];

        if (isLineSelect) {
            const x1 = this.selectionStart.x;
            const y1 = this.selectionStart.y;
            const x2 = this.selectionEnd.x;
            const y2 = this.selectionEnd.y;

            const dx = Math.abs(x2 - x1);
            const dy = Math.abs(y2 - y1);
            const sx = (x1 < x2) ? 16 : -16;
            const sy = (y1 < y2) ? 16 : -16;
            let err = dx - dy;

            let x = x1;
            let y = y1;

            while (true) {
                newSelection.push({ x: x, y: y });

                if (x === x2 && y === y2) break;

                const e2 = 2 * err;

                if (e2 > -dy) {
                    err -= dy;
                    x += sx;
                }

                if (e2 < dx) {
                    err += dx;
                    y += sy;
                }
            }
        } else {
            const startX = Math.min(this.selectionStart.x, this.selectionEnd.x);
            const startY = Math.min(this.selectionStart.y, this.selectionEnd.y);
            const endX = Math.max(this.selectionEnd.x, this.selectionEnd.x);
            const endY = Math.max(this.selectionStart.y, this.selectionStart.y);

            for (let x = startX; x <= endX; x += 16) {
                for (let y = startY; y <= endY; y += 16) {
                    newSelection.push({ x: x, y: y });
                }
            }
        }

        if (input.isAltPressed) {
            this.selectedTiles = [...this.tempSelectedTiles, ...newSelection];
        } else {
            this.selectedTiles = newSelection;
        }

        console.log("Selected tiles:", this.selectedTiles);
    },

    initEventListeners: function() {
        document.addEventListener("keydown", this.keyDownHandler.bind(this));
        document.addEventListener("keyup", this.keyUpHandler.bind(this));
        document.addEventListener('mousedown', this.mouseDownHandler.bind(this));
        document.addEventListener('mousemove', this.mouseMoveHandler.bind(this));
        document.addEventListener('mouseup', this.mouseUpHandler.bind(this));
    },

    removeEventListeners: function() {
        document.removeEventListener("keydown", this.keyDownHandler.bind(this));
        document.removeEventListener("keyup", this.keyUpHandler.bind(this));
        document.removeEventListener('mousedown', this.mouseDownHandler.bind(this));
        document.removeEventListener('mousemove', this.mouseMoveHandler.bind(this));
        document.removeEventListener('mouseup', this.mouseUpHandler.bind(this));
    },

    keyDownHandler: function(e) {
        // Check if the target is an input or textarea element
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            e.preventDefault(); // Prevent default behavior for non-input elements
        }

        if (e.ctrlKey) {
            if (e.shiftKey && e.key === 'Z') {
                this.redo();
            } else if (e.key === 'z') {
                this.undo();
            }
            return; // Exit early to prevent other actions on Ctrl+Z or Ctrl+Shift+Z
        }

        if (e.altKey) {
            if (e.key === 'c') {
                ui.modal('mishell/index.php', 'mishell_window');
            }
        } else if (e.key === 'Tab') {
            e.preventDefault();
            modal.load('editMode', 'edit_mode_window');
        } else {
            const dir = input.keys[e.code];
            if (dir) {
                game.sprites[0].addDirection(dir); // Control the main sprite
            }
        }

        if (e.key === 'Shift') {
            input.isShiftPressed = true;
        } else if (e.key === 'Control') {
            input.isCtrlPressed = true;
        } else if (e.key === 'Alt') {
            input.isAltPressed = true;
        }
    },

    keyUpHandler: function(e) {
        // Check if the target is an input or textarea element
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            e.preventDefault(); // Prevent default behavior for non-input elements
        }

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
                modal.close(attributeName);
            }
        } else {
            const dir = input.keys[e.code];
            if (dir) {
                game.sprites[0].removeDirection(dir); // Control the main sprite
            }
        }

        if (e.key === 'Shift') {
            input.isShiftPressed = false;
        } else if (e.key === 'Control') {
            input.isCtrlPressed = false;
        } else if (e.key === 'Alt') {
            input.isAltPressed = false;
        }
    },

    mouseDownHandler: function(e) {
        const isEventOnCanvas = e.target === game.canvas || game.canvas.contains(e.target);

        if (e.button === 0 && this.currentMode === this.Modes.SELECT && isEventOnCanvas) {
            const rect = game.canvas.getBoundingClientRect();
            const x = (e.clientX - rect.left) / game.zoomLevel + game.cameraX;
            const y = (e.clientY - rect.top) / game.zoomLevel + game.cameraY;
            const tileX = Math.floor(x / 16) * 16;
            const tileY = Math.floor(y / 16) * 16;

            if (input.isShiftPressed) {
                this.selectionEnd = { x: tileX, y: tileY };
                this.updateSelectedTiles();
            } else if (input.isCtrlPressed) {
                this.selectionEnd = { x: tileX, y: tileY };
                this.updateSelectedTiles(true);
            } else {
                this.selectionStart = { x: tileX, y: tileY };
                this.selectionEnd = { x: tileX, y: tileY };
                this.isSelecting = true;

                if (input.isAltPressed) {
                    this.tempSelectedTiles = [...this.selectedTiles];
                } else {
                    this.tempSelectedTiles = [];
                }
            }
        }

        if ((e.button === 1) || (game.isEditMode && this.currentMode === this.Modes.NAVIGATE)) {
            input.isDragging = true;
            input.startX = e.clientX;
            input.startY = e.clientY;
            document.body.classList.add('move-cursor');
        }
    },

    mouseMoveHandler: function(e) {
        if (this.isSelecting && this.currentMode === this.Modes.SELECT) {
            const rect = game.canvas.getBoundingClientRect();
            const x = (e.clientX - rect.left) / game.zoomLevel + game.cameraX;
            const y = (e.clientY - rect.top) / game.zoomLevel + game.cameraY;
            const tileX = Math.floor(x / 16) * 16;
            const tileY = Math.floor(y / 16) * 16;

            if (input.isCtrlPressed) {
                this.selectionEnd = { x: tileX, y: tileY };
                this.updateSelectedTiles(true);
            } else {
                this.selectionEnd = { x: tileX, y: tileY };
                this.updateSelectedTiles();
            }
        }

        if (game.isEditMode && input.isDragging) {
            const dx = (input.startX - e.clientX) / game.zoomLevel;
            const dy = (input.startY - e.clientY) / game.zoomLevel;

            game.cameraX = Math.max(0, Math.min(game.worldWidth - window.innerWidth / game.zoomLevel, game.cameraX + dx));
            game.cameraY = Math.max(0, Math.min(game.worldHeight - window.innerHeight / game.zoomLevel, game.cameraY + dy));

            input.startX = e.clientX;
            input.startY = e.clientY;
        }
    },

    mouseUpHandler: function(e) {
        if (game.isEditMode && this.isSelecting) {
            this.isSelecting = false;
            this.updateSelectedTiles(input.isCtrlPressed);

            const latestSelection = [...this.selectedTiles];
            const previousSelection = this.history[this.history.length - 1];

            if (JSON.stringify(latestSelection) !== JSON.stringify(previousSelection)) {
                this.history.push(latestSelection);
                this.redoStack = [];
            }
        }
        input.isDragging = false;
        document.body.classList.remove('move-cursor');
    },
};

// Start the edit mode window when required
edit_mode_window.start();
  </script>
<?php
}
?>
