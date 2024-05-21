document.addEventListener('DOMContentLoaded', (e) => { input.loaded(e) });

var input = {
    pressedDirections: [],
    keys: {
        'ArrowUp': "up",
        'ArrowLeft': "left",
        'ArrowRight': "right",
        'ArrowDown': "down",
    },
    selectionStart: null,
    selectionEnd: null,
    isSelecting: false,
    selectedTiles: [],
    tempSelectedTiles: [],
    isShiftPressed: false,
    isCtrlPressed: false,
    isAltPressed: false,
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
        network.init();
    },

    keyDown: function(e) {
        if(e.altKey) {
            if(e.key === 'c') { ui.modal('mishell/index.php', 'mishell_window')}
        } else if (e.key === 'Tab') {
            e.preventDefault(); // Prevent the default browser behavior
            modal.load('editMode', 'edit_mode_window')
        } else {
            const dir = this.keys[e.code];
            if (dir) {
                sprite.addDirection(dir);
            }
        }
        
        // Track Shift, Ctrl, and Alt keys
        if (e.key === 'Shift') {
            this.isShiftPressed = true;
        } else if (e.key === 'Control') {
            this.isCtrlPressed = true;
        } else if (e.key === 'Alt') {
            this.isAltPressed = true;
        }
    },
    
    keyUp: function(e) {
        if(e.keyCode === 27) { // ESC key
            let maxZIndex = -Infinity;
            let maxZIndexElement = null;
            let attributeName = null;
        
            document.querySelectorAll('[data-window]').forEach(function(element) {
                let zIndex = parseInt(window.getComputedStyle(element).zIndex, 10);
                if(zIndex > maxZIndex) {
                    maxZIndex = zIndex;
                    maxZIndexElement = element;
                    attributeName = element.getAttribute('data-window');
                }
            });
        
            if(maxZIndexElement) {
                ui.closeModal(attributeName);
            }
        } else {
            const dir = this.keys[e.code];
            if(dir) {
                sprite.removeDirection(dir);
            }
        }
        
        // Track Shift, Ctrl, and Alt keys
        if (e.key === 'Shift') {
            this.isShiftPressed = false;
        } else if (e.key === 'Control') {
            this.isCtrlPressed = false;
        } else if (e.key === 'Alt') {
            this.isAltPressed = false;
        }
    },

    mouseDown: function(e) {
        if (e.button === 0 && editor.currentMode === Modes.SELECT) {
            const rect = game.canvas.getBoundingClientRect();
            const x = (e.clientX - rect.left) / game.zoomLevel + camera.cameraX;
            const y = (e.clientY - rect.top) / game.zoomLevel + camera.cameraY;
            const tileX = Math.floor(x / 16) * 16;
            const tileY = Math.floor(y / 16) * 16;

            if (this.isShiftPressed) {
                this.selectionEnd = { x: tileX, y: tileY };
                this.updateSelectedTiles();
            } else if (this.isCtrlPressed) {
                this.selectionEnd = { x: tileX, y: tileY };
                this.updateSelectedTiles(true);
            } else {
                this.selectionStart = { x: tileX, y: tileY };
                this.selectionEnd = { x: tileX, y: tileY };
                this.isSelecting = true;
                
                // Handle Alt key for appending to the selection
                if (this.isAltPressed) {
                    this.tempSelectedTiles = [...this.selectedTiles];
                } else {
                    this.tempSelectedTiles = [];
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
        if (this.isSelecting && editor.currentMode === Modes.SELECT) {
            const rect = game.canvas.getBoundingClientRect();
            const x = (e.clientX - rect.left) / game.zoomLevel + camera.cameraX;
            const y = (e.clientY - rect.top) / game.zoomLevel + camera.cameraY;
            const tileX = Math.floor(x / 16) * 16;
            const tileY = Math.floor(y / 16) * 16;

            if (this.isCtrlPressed) {
                this.selectionEnd = { x: tileX, y: tileY };
                this.updateSelectedTiles(true);
            } else {
                this.selectionEnd = { x: tileX, y: tileY };
                this.updateSelectedTiles();
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
            const endX = Math.max(this.selectionStart.x, this.selectionEnd.x);
            const endY = Math.max(this.selectionStart.y, this.selectionEnd.y);

            for (let x = startX; x <= endX; x += 16) {
                for (let y = startY; y <= endY; y += 16) {
                    newSelection.push({ x: x, y: y });
                }
            }
        }

        if (this.isAltPressed) {
            this.selectedTiles = [...this.tempSelectedTiles, ...newSelection];
        } else {
            this.selectedTiles = newSelection;
        }

        console.log("Selected tiles:", this.selectedTiles);
    },
    
    mouseUp: function(e) {
        if (editor.isEditMode && this.isSelecting) {
            this.isSelecting = false;
            this.updateSelectedTiles(this.isCtrlPressed);
        }
        this.isDragging = false;
        document.body.classList.remove('move-cursor');
    },

    mouseWheelScroll: function(e) {
        if(editor.isEditMode) {
            const zoomStep = 1;
            game.zoomLevel += (e.deltaY > 0) ? -zoomStep : zoomStep;
            game.zoomLevel = Math.max(3, Math.min(10, game.zoomLevel));
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
