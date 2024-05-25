document.addEventListener('DOMContentLoaded', (e) => { input.loaded(e) });

var input = {
    pressedDirections: [],
    keys: {
        'ArrowUp': "up",
        'ArrowLeft': "left",
        'ArrowRight': "right",
        'ArrowDown': "down",
    },
    isShiftPressed: false,
    isCtrlPressed: false,
    isAltPressed: false,
    isDragging: false,

    init: function() {
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
    },

    loaded: function(e) {
        this.init();
        editor.history = [];
        editor.redoStack = [];
        network.init();
    },

    keyDown: function(e) {
        // Check if the target is an input or textarea element
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            e.preventDefault(); // Prevent default behavior for non-input elements
        }
    
        if (e.ctrlKey) {
            if (e.shiftKey && e.key === 'Z') {
                editor.redo();
            } else if (e.key === 'z') {
                editor.undo();
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
            const dir = this.keys[e.code];
            if (dir) {
                game.sprites[0].addDirection(dir); // Control the main sprite
            }
        }
    
        if (e.key === 'Shift') {
            this.isShiftPressed = true;
        } else if (e.key === 'Control') {
            this.isCtrlPressed = true;
        } else if (e.key === 'Alt') {
            this.isAltPressed = true;
        }
    },
    
    keyUp: function(e) {
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
                modal.closeModal(attributeName);
            }
        } else {
            const dir = this.keys[e.code];
            if (dir) {
                game.sprites[0].removeDirection(dir); // Control the main sprite
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
        const isEventOnCanvas = e.target === game.canvas || game.canvas.contains(e.target);

        if (e.button === 0 && editor.currentMode === Modes.SELECT && isEventOnCanvas) {
            const rect = game.canvas.getBoundingClientRect();
            const x = (e.clientX - rect.left) / game.zoomLevel + camera.cameraX;
            const y = (e.clientY - rect.top) / game.zoomLevel + camera.cameraY;
            const tileX = Math.floor(x / 16) * 16;
            const tileY = Math.floor(y / 16) * 16;

            if (this.isShiftPressed) {
                editor.selectionEnd = { x: tileX, y: tileY };
                editor.updateSelectedTiles();
            } else if (this.isCtrlPressed) {
                editor.selectionEnd = { x: tileX, y: tileY };
                editor.updateSelectedTiles(true);
            } else {
                editor.selectionStart = { x: tileX, y: tileY };
                editor.selectionEnd = { x: tileX, y: tileY };
                editor.isSelecting = true;

                if (this.isAltPressed) {
                    editor.tempSelectedTiles = [...editor.selectedTiles];
                } else {
                    editor.tempSelectedTiles = [];
                }
            }
        }

        if ((e.button === 1) || (editor.isEditMode && editor.currentMode === Modes.NAVIGATE)) {
            this.isDragging = true;
            this.startX = e.clientX;
            this.startY = e.clientY;
            document.body.classList.add('move-cursor');
        }
    },

    mouseMove: function(e) {
        if (editor.isSelecting && editor.currentMode === Modes.SELECT) {
            const rect = game.canvas.getBoundingClientRect();
            const x = (e.clientX - rect.left) / game.zoomLevel + camera.cameraX;
            const y = (e.clientY - rect.top) / game.zoomLevel + camera.cameraY;
            const tileX = Math.floor(x / 16) * 16;
            const tileY = Math.floor(y / 16) * 16;

            if (this.isCtrlPressed) {
                editor.selectionEnd = { x: tileX, y: tileY };
                editor.updateSelectedTiles(true);
            } else {
                editor.selectionEnd = { x: tileX, y: tileY };
                editor.updateSelectedTiles();
            }
        }

        if (editor.isEditMode && this.isDragging) {
            const dx = (this.startX - e.clientX) / game.zoomLevel;
            const dy = (this.startY - e.clientY) / game.zoomLevel;

            camera.cameraX = Math.max(0, Math.min(game.worldWidth - window.innerWidth / game.zoomLevel, camera.cameraX + dx));
            camera.cameraY = Math.max(0, Math.min(game.worldHeight - window.innerHeight / game.zoomLevel, camera.cameraY + dy));

            this.startX = e.clientX;
            this.startY = e.clientY;
        }
    },

    mouseUp: function(e) {
        if (editor.isEditMode && editor.isSelecting) {
            editor.isSelecting = false;
            editor.updateSelectedTiles(this.isCtrlPressed);

            const latestSelection = [...editor.selectedTiles];
            const previousSelection = editor.history[editor.history.length - 1];

            if (JSON.stringify(latestSelection) !== JSON.stringify(previousSelection)) {
                editor.history.push(latestSelection);
                editor.redoStack = [];
            }
        }
        this.isDragging = false;
        document.body.classList.remove('move-cursor');
    },

    mouseWheelScroll: function(e) {
        const isEventOnCanvas = e.target === game.canvas || game.canvas.contains(e.target);

        if (isEventOnCanvas) {
            e.preventDefault(); // Prevent default scroll behavior for all cases

            if (this.isAltPressed) {
                const panSpeed = 10;
                camera.cameraX += e.deltaY > 0 ? panSpeed : -panSpeed;
                camera.cameraX = Math.max(0, Math.min(camera.cameraX, game.worldWidth - window.innerWidth / game.zoomLevel));
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
                camera.cameraX = cursorX - (cursorX - camera.cameraX) * zoomFactor;
                camera.cameraY = cursorY - (cursorY - camera.cameraY) * zoomFactor;

                // Ensure the camera doesn't go outside the world bounds
                const scaledWindowWidth = window.innerWidth / game.zoomLevel;
                const scaledWindowHeight = window.innerHeight / game.zoomLevel;
                camera.cameraX = Math.max(0, Math.min(camera.cameraX, game.worldWidth - scaledWindowWidth));
                camera.cameraY = Math.max(0, Math.min(camera.cameraY, game.worldHeight - scaledWindowHeight));
            } else {
                const panSpeed = 10;
                camera.cameraY += e.deltaY > 0 ? panSpeed : -panSpeed;
                camera.cameraY = Math.max(0, Math.min(camera.cameraY, game.worldHeight - window.innerHeight / game.zoomLevel));
            }
        }
    },

    leftClick: function(e) {
        console.log("left button clicked");
        if(e.target.matches('[data-close], [data-esc]')) {
            console.log("data close clicked");
            var parent = modal.closest(e.target);
            modal.close(parent);
        }
    },

    rightClick: function(e, x, y) {
        e.preventDefault();
    },

    doubleClick: function(e, x, y) {

    }
};
