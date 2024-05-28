var input = {
    pressedDirections: [],
    keys: {
        'ArrowUp': "up",
        'ArrowLeft': "left",
        'ArrowRight': "right",
        'ArrowDown': "down",
        'w': "up",
        'a': "left",
        's': "down",
        'd': "right"
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
        network.init();
    },

    keyUp: function(e) {
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            e.preventDefault(); // Prevent default action for keyUp
            this.handleKeyUp(e);
        }
    },

    keyDown: function(e) {
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            if (e.key === 'Tab') {
                e.preventDefault();
            }
            this.handleKeyDown(e);
        }
    },

    handleKeyDown: function(e) {
        if (e.altKey && e.key === 'c') {
            if (e.key === 'c') {
                modal.load('mishell/index.php', 'mishell_window');
            }
        } else if (e.key === 'Tab') {
            e.preventDefault();
            modal.load('editMode', 'edit_mode_window');
        } else {
            const dir = this.keys[e.key];
            if (dir) {
                const mainSprite = game.sprites['main'];
                if (mainSprite) {
                    mainSprite.addDirection(dir); // Control the main sprite
                } else {
                    console.error('Main sprite not found.');
                }
            }
        }

        if (e.key === 'Shift') {
            this.isShiftPressed = true;
        } else if (e.key === 'Control') {
            this.isCtrlPressed = true;
        } else if (e.key === 'Alt') {
            this.isAltPressed = true;
        }

        if (e.key === 'F') {
            const mainSprite = game.sprites['main'];
            if (mainSprite) {
                mainSprite.targetAim = !mainSprite.targetAim; // Toggle target aiming mode
                if (mainSprite.targetAim) {
                    console.log('Target aim activated');
                } else {
                    console.log('Target aim deactivated');
                }
            } else {
                console.error('Main sprite not found.');
            }
        }
    },

    handleKeyUp: function(e) {
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
            const dir = this.keys[e.key];
            if (dir) {
                const mainSprite = game.sprites['main'];
                if (mainSprite) {
                    mainSprite.removeDirection(dir); // Control the main sprite
                } else {
                    console.error('Main sprite not found.');
                }
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
        if (e.button === 1) {
            this.isDragging = true;
            this.startX = e.clientX;
            this.startY = e.clientY;
            document.body.classList.add('move-cursor');
        }
    },

    mouseMove: function(e) {
        if (this.isDragging) {
            const dx = (this.startX - e.clientX) / game.zoomLevel;
            const dy = (this.startY - e.clientY) / game.zoomLevel;

            game.cameraX = Math.max(0, Math.min(game.worldWidth - window.innerWidth / game.zoomLevel, game.cameraX + dx));
            game.cameraY = Math.max(0, Math.min(game.worldHeight - window.innerHeight / game.zoomLevel, game.cameraY + dy));

            this.startX = e.clientX;
            this.startY = e.clientY;
        }

        // Update mouse coordinates for target aiming
        const mainSprite = game.sprites['main'];
        if (mainSprite && mainSprite.targetAim) {
            const rect = game.canvas.getBoundingClientRect();
            mainSprite.targetX = (e.clientX - rect.left) / game.zoomLevel + game.cameraX;
            mainSprite.targetY = (e.clientY - rect.top) / game.zoomLevel + game.cameraY;
        }
    },

    mouseUp: function(e) {
        this.isDragging = false;
        document.body.classList.remove('move-cursor');
    },

    mouseWheelScroll: function(e) {
        const isEventOnCanvas = e.target === game.canvas || game.canvas.contains(e.target);

        if (isEventOnCanvas) {
            e.preventDefault(); // Prevent default scroll behavior for all cases

            if (e.altKey) {
                const panSpeed = 10;
                game.cameraX += e.deltaY > 0 ? panSpeed : -panSpeed;
                game.cameraX = Math.max(0, Math.min(game.cameraX, game.worldWidth - window.innerWidth / game.zoomLevel));
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
                game.cameraX = cursorX - (cursorX - game.cameraX) * zoomFactor;
                game.cameraY = cursorY - (cursorY - game.cameraY) * zoomFactor;

                // Ensure the camera doesn't go outside the world bounds
                const scaledWindowWidth = window.innerWidth / game.zoomLevel;
                const scaledWindowHeight = window.innerHeight / game.zoomLevel;
                game.cameraX = Math.max(0, Math.min(game.cameraX, game.worldWidth - scaledWindowWidth));
                game.cameraY = Math.max(0, Math.min(game.cameraY, game.worldHeight - scaledWindowHeight));
            } else {
                const panSpeed = 10;
                game.cameraY += e.deltaY > 0 ? panSpeed : -panSpeed;
                game.cameraY = Math.max(0, Math.min(game.cameraY, game.worldHeight - window.innerHeight / game.zoomLevel));
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

    rightClick: function(e) {
        e.preventDefault();
    },

    doubleClick: function(e) {}
};

document.addEventListener('DOMContentLoaded', (e) => { input.loaded(e) });
