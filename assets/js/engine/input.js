var input = {
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
    isShiftPressed: false,
    isCtrlPressed: false,
    isAltPressed: false,
    isDragging: false,
    directions: { up: false, down: false, left: false, right: false },
    l2Pressed: false, // Track L2 button state

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

        window.addEventListener('gamepadA', (e) => this.gamepadAButton(e));
        window.addEventListener('gamepadB', (e) => this.gamepadBButton(e));
        window.addEventListener('gamepadStartPressed', gamepad.throttle((e) => this.gamepadStart(e), 1000));
        window.addEventListener('gamepadAxes', (e) => this.handleAxes(e.detail));

        window.addEventListener('gamepadLeftTriggerPressed', (e) => this.gamepadLeftTrigger(true));
        window.addEventListener('gamepadLeftTriggerReleased', (e) => this.gamepadLeftTrigger(false));
    },

    gamepadLeftTrigger: function(isPressed) {
        this.l2Pressed = isPressed;
        if (isPressed) {
            if (game.mainSprite) {
                game.mainSprite.targetAim = true;
            }
        } else {
            if (game.mainSprite) {
                game.mainSprite.targetAim = false;
            }
        }
    },

    handleAxes: function(axes) {
        this.handleLeftAxes(axes);
        this.handleRightAxes(axes);
    },

    handleLeftAxes: function(axes) {
        const threshold = 0.2; // Dead zone threshold

        // Calculate axis pressures
        const leftStickX = Math.abs(axes[0]);
        const leftStickY = Math.abs(axes[1]);

        // Reset gamepad directions
        gamepad.directions = { left: false, right: false, up: false, down: false };

        if (leftStickX > threshold || leftStickY > threshold) {
            gamepad.axesPressures.leftStickX = leftStickX;
            gamepad.axesPressures.leftStickY = leftStickY;

            // Left stick movement (axes[0] = left/right, axes[1] = up/down)
            gamepad.directions.right = axes[0] > threshold;
            gamepad.directions.left = axes[0] < -threshold;
            gamepad.directions.down = axes[1] > threshold;
            gamepad.directions.up = axes[1] < -threshold;

            // Update the sprite's directions based on combined states
            this.updateSpriteDirections();
        } else {
            // Reset the directions for left stick if below threshold
            gamepad.axesPressures.leftStickX = 0;
            gamepad.axesPressures.leftStickY = 0;
        }

        // Update the sprite's directions based on combined states
        this.updateSpriteDirections();
    },

    handleRightAxes: function(axes) {
        const threshold = 0.9; // High threshold for zooming
        const deadZone = 0.2; // Dead zone threshold for minimal stick movement

        // Calculate axis pressures
        const rightStickX = Math.abs(axes[2]);
        const rightStickY = Math.abs(axes[3]);

        if (rightStickX > deadZone || rightStickY > deadZone) {
            gamepad.axesPressures.rightStickX = rightStickX;
            gamepad.axesPressures.rightStickY = rightStickY;

            // If the aim tool is active, update its position
            if (game.mainSprite && game.mainSprite.targetAim) {
                const aimSpeed = 10; // Adjust aim speed as necessary
                const newTargetX = game.mainSprite.targetX + axes[2] * aimSpeed;
                const newTargetY = game.mainSprite.targetY + axes[3] * aimSpeed;

                // Calculate distance from the main sprite
                const deltaX = newTargetX - (game.mainSprite.x + game.mainSprite.width / 2);
                const deltaY = newTargetY - (game.mainSprite.y + game.mainSprite.height / 2);
                const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);

                // Update sprite direction based on aim tool position
                const angle = Math.atan2(deltaY, deltaX);
                if (angle >= -Math.PI / 8 && angle < Math.PI / 8) {
                    game.mainSprite.direction = 'E';
                } else if (angle >= Math.PI / 8 && angle < 3 * Math.PI / 8) {
                    game.mainSprite.direction = 'SE';
                } else if (angle >= 3 * Math.PI / 8 && angle < 5 * Math.PI / 8) {
                    game.mainSprite.direction = 'S';
                } else if (angle >= 5 * Math.PI / 8 && angle < 7 * Math.PI / 8) {
                    game.mainSprite.direction = 'SW';
                } else if (angle >= 7 * Math.PI / 8 || angle < -7 * Math.PI / 8) {
                    game.mainSprite.direction = 'W';
                } else if (angle >= -7 * Math.PI / 8 && angle < -5 * Math.PI / 8) {
                    game.mainSprite.direction = 'NW';
                } else if (angle >= -5 * Math.PI / 8 && angle < -3 * Math.PI / 8) {
                    game.mainSprite.direction = 'N';
                } else if (angle >= -3 * Math.PI / 8 && angle < -Math.PI / 8) {
                    game.mainSprite.direction = 'NE';
                }

                // If within maxRange, update targetX and targetY
                if (distance <= game.mainSprite.maxRange) {
                    game.mainSprite.targetX = newTargetX;
                    game.mainSprite.targetY = newTargetY;
                } else {
                    // Otherwise, set target to maxRange in the same direction
                    const maxRangeX = game.mainSprite.x + game.mainSprite.width / 2 + Math.cos(angle) * game.mainSprite.maxRange;
                    const maxRangeY = game.mainSprite.y + game.mainSprite.height / 2 + Math.sin(angle) * game.mainSprite.maxRange;
                    game.mainSprite.targetX = Math.max(0, Math.min(maxRangeX, game.worldWidth));
                    game.mainSprite.targetY = Math.max(0, Math.min(maxRangeY, game.worldHeight));
                }
            }

            // Adjust zoom level based on right stick Y-axis movement with high threshold and increased throttling
            if (!this.l2Pressed) {
                const now = Date.now();
                if (axes[3] > threshold && (!gamepad.throttledEvents['zoomIn'] || now - gamepad.throttledEvents['zoomIn'] >= 200)) {
                    camera.lerpEnabled = false; // Disable lerp
                    game.zoomLevel = Math.max(2, game.zoomLevel - 1);
                    gamepad.throttledEvents['zoomIn'] = now;
                    setTimeout(() => { camera.lerpEnabled = true; }, 300); // Re-enable lerp after 300ms
                } else if (axes[3] < -threshold && (!gamepad.throttledEvents['zoomOut'] || now - gamepad.throttledEvents['zoomOut'] >= 200)) {
                    camera.lerpEnabled = false; // Disable lerp
                    game.zoomLevel = Math.min(10, game.zoomLevel + 1);
                    gamepad.throttledEvents['zoomOut'] = now;
                    setTimeout(() => { camera.lerpEnabled = true; }, 300); // Re-enable lerp after 300ms
                }
            }

        } else {
            // Reset the pressures for right stick if below dead zone threshold
            gamepad.axesPressures.rightStickX = 0;
            gamepad.axesPressures.rightStickY = 0;
        }

        gamepad.emitButtonEvent('RightStickMoved', { deltaX: rightStickX, deltaY: rightStickY });
    },

    keyUp: function(e) {
        game.updateInputMethod('keyboard');
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            e.preventDefault(); // Prevent default action for keyUp
            this.handleKeyUp(e);
        }
    },

    keyDown: function(e) {
        game.updateInputMethod('keyboard');
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
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
        this.handleControlStateChange(e, true);

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
                this.directions[dir] = true;
                this.updateSpriteDirections();
            }
        }

        if (e.key === 'f') {
            if (game.mainSprite) {
                game.mainSprite.targetAim = !game.mainSprite.targetAim; // Toggle target aiming mode
                if (game.mainSprite.targetAim) {
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
        this.handleControlStateChange(e, false);

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
        if (dir) {
            this.directions[dir] = false;
            this.updateSpriteDirections();
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
            this.cancelPathfinding(game.mainSprite);
        }
    },

    mouseMove: function(e) {
        if (this.isDragging) {
            const dx = (this.startX - e.clientX) / game.zoomLevel;
            const dy = (this.startY - e.clientY) / game.zoomLevel;

            camera.cameraX = Math.max(0, Math.min(game.worldWidth - window.innerWidth / game.zoomLevel, camera.cameraX + dx));
            camera.cameraY = Math.max(0, Math.min(game.worldHeight - window.innerHeight / game.zoomLevel, camera.cameraY + dy));

            this.startX = e.clientX;
            this.startY = e.clientY;
        }

        // Update mouse coordinates for target aiming
        if (game.mainSprite && game.mainSprite.targetAim) {
            const rect = game.canvas.getBoundingClientRect();
            const newX = (e.clientX - rect.left) / game.zoomLevel + camera.cameraX;
            const newY = (e.clientY - rect.top) / game.zoomLevel + camera.cameraY;

            game.mainSprite.targetX = newX;
            game.mainSprite.targetY = newY;
        }
    },

    mouseUp: function(e) {
        this.isDragging = false;
        document.body.classList.remove('move-cursor');
    },

    mouseWheelScroll: function(e) {
        game.updateInputMethod('keyboard');
        const isEventOnCanvas = e.target === game.canvas || game.canvas.contains(e.target);

        if (isEventOnCanvas) {
            e.preventDefault(); // Prevent default scroll behavior for all cases

            if (e.altKey) {
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
        game.updateInputMethod('keyboard');
        console.log("left button clicked");
        if (e.target.matches('[data-close], [data-esc]')) {
            console.log("data close clicked");
            var parent = modal.closest(e.target);
            modal.close(parent);
        }
    },

    rightClick: function(e) {
        e.preventDefault();
        game.updateInputMethod('keyboard');
        console.log("right button clicked");
        this.cancelPathfinding(game.mainSprite);
    },

    doubleClick: function(e) {},

    cancelPathfinding: function(sprite) {
        if (sprite && sprite.isMovingToTarget) {
            sprite.isMovingToTarget = false;
            sprite.path = [];
            sprite.moving = false; // Reset the moving flag
            audio.stopLoopingAudio('walkGrass', 'sfx', 0.5); // Stop walking audio
        }
    },

    handleControlStateChange: function(e, isPressed) {
        switch (e.key) {
            case 'Shift':
                this.isShiftPressed = isPressed;
                break;
            case 'Control':
                this.isCtrlPressed = isPressed;
                break;
            case 'Alt':
                this.isAltPressed = isPressed;
                break;
            case ' ':
                this.isSpacePressed = isPressed;
                break;
        }
    },

    updateSpriteDirections: function() {
        const combinedDirections = {
            up: (gamepad.directions && gamepad.directions.up) || this.directions.up,
            down: (gamepad.directions && gamepad.directions.down) || this.directions.down,
            left: (gamepad.directions && gamepad.directions.left) || this.directions.left,
            right: (gamepad.directions && gamepad.directions.right) || this.directions.right
        };

        const directions = ['up', 'down', 'left', 'right'];
        directions.forEach(direction => {
            if (game.mainSprite) {
                if (combinedDirections[direction]) {
                    game.mainSprite.addDirection(direction);
                } else {
                    game.mainSprite.removeDirection(direction);
                }
            }
        });

        // Stop walking audio if no directions are pressed
        if (game.mainSprite && !combinedDirections.up && !combinedDirections.down && !combinedDirections.left && !combinedDirections.right) {
            audio.stopLoopingAudio('walkGrass', 'sfx', 0.5);
        }
    },

    gamepadStart: function() {
        console.log("start button pressed in input.js");
        modal.load('console', null, "console", true);
        modal.front('console_window');
    }
};
