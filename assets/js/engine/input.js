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
    isSpacePressed: false,
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

    keyUp: function(e) {
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            const timestamp = new Date().toISOString();
            console.log(`[${timestamp}] Key up: ${e.key}`);
            e.preventDefault(); // Prevent default action for keyUp
            this.handleKeyUp(e);
        }
    },

    keyDown: function(e) {
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            const timestamp = new Date().toISOString();
            console.log(`[${timestamp}] Key down: ${e.key}`);
            if (e.key === 'Tab') {
                e.preventDefault();
            }
            if (e.key === ' ') {
                e.preventDefault(); // Prevent default behavior for Space bar
            }
            this.handleKeyDown(e);
        }
    },

    handleKeyDown: function(e) {
        const mainSprite = game.sprites[game.playerid];

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
                if (mainSprite && game.allowControls) {
                    mainSprite.addDirection(dir); // Control the main sprite
                } else {
                    console.error('Main sprite not found.');
                }
                this.cancelPathfinding(mainSprite);
            }
        }

        if (e.key === ' ') {
            if (mainSprite && game.allowControls) {
                this.isSpacePressed = true;
                mainSprite.targetAim = true;
                this.cancelPathfinding(mainSprite);
            }
        } else if (e.key === 'Control') {
            this.isCtrlPressed = true;
        } else if (e.key === 'Alt') {
            this.isAltPressed = true;
        }

        // Check and play walking audio
        if (mainSprite && mainSprite.isMoving) {
            audio.playAudio("walkAudio", assets.load('walkAudio'), 'sfx', true);
        }
    },

    handleKeyUp: function(e) {
        const mainSprite = game.sprites[game.playerid];
        if (e.keyCode === 27) { // ESC key
            let maxZIndex = -Infinity;
            let maxZIndexElement = null;
            let attributeName = null;

            document.querySelectorAll("*").forEach(function (element) {
                const zIndex = parseInt(window.getComputedStyle(element).zIndex);
                if (!isNaN(zIndex) && zIndex > maxZIndex) {
                    maxZIndex = zIndex;
                    maxZIndexElement = element;
                    attributeName = element.getAttribute('data-attribute-name');
                }
            });

            if (maxZIndexElement) {
                maxZIndexElement.dispatchEvent(new Event('click'));
            } else if (attributeName) {
                const attributeElement = document.querySelector(`[data-attribute-name="${attributeName}"]`);
                if (attributeElement) {
                    attributeElement.dispatchEvent(new Event('click'));
                }
            }
        }

        const dir = this.keys[e.key];
        if (dir && mainSprite) {
            mainSprite.removeDirection(dir);

            // Stop walking audio if no directions are pressed
            if (!mainSprite.isMoving) {
                audio.stopLoopingAudio('walkAudio', 'sfx', 0.5);
            }
        }

        if (e.key === ' ') {
            this.isSpacePressed = false;
            if (mainSprite) {
                mainSprite.targetAim = false;
            }
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

        // Cancel pathfinding on right-click
        if (e.button === 2) { // Right mouse button
            const mainSprite = game.sprites[game.playerid];
            this.cancelPathfinding(mainSprite);
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
        const mainSprite = game.sprites[game.playerid];
        if (mainSprite && mainSprite.targetAim) {
            const rect = game.canvas.getBoundingClientRect();
            const newX = (e.clientX - rect.left) / game.zoomLevel + game.cameraX;
            const newY = (e.clientY - rect.top) / game.zoomLevel + game.cameraY;

            if (this.isSpacePressed) {
                // Fine-tune the movement by reducing the delta
                const deltaX = (newX - mainSprite.targetX) * 0.1;
                const deltaY = (newY - mainSprite.targetY) * 0.1;
                mainSprite.targetX += deltaX;
                mainSprite.targetY += deltaY;
            } else {
                mainSprite.targetX = newX;
                mainSprite.targetY = newY;
            }
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
                game.zoomLevel = Math.max(2, Math.min(10, game.zoomLevel));

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
        if (e.target.matches('[data-close], [data-esc]')) {
            console.log("data close clicked");
            var parent = modal.closest(e.target);
            modal.close(parent);
        }
    },

    rightClick: function(e) {
        e.preventDefault();
        console.log("right button clicked");
        const mainSprite = game.sprites[game.playerid];
        this.cancelPathfinding(mainSprite);
    },

    doubleClick: function(e) {},

    cancelPathfinding: function(sprite) {
        if (sprite && sprite.isMovingToTarget) {
            sprite.isMovingToTarget = false;
            sprite.path = [];
            sprite.moving = false;
            sprite.stopping = true;
            console.log("Pathfinding cancelled");
        }
    }
};
