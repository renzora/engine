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
        window.addEventListener('gamepada', (e) => this.gamepadAButton(e));
        window.addEventListener('gamepadb', (e) => this.gamepadBButton(e));
        window.addEventListener('gamepadyPressed', (e) => this.gamepadYButtonPressed(e));
        window.addEventListener('gamepadyReleased', (e) => this.gamepadYButtonReleased(e));
        window.addEventListener('gamepadxPressed', (e) => input.gamepadXButton(e));
        window.addEventListener('gamepadxReleased', (e) => input.gamepadXButtonReleased(e));
        window.addEventListener('gamepady', (e) => this.gamepadYButton(e));
        window.addEventListener('gamepadl1Pressed', (e) => this.gamepadLeftBumper(e.detail));
        window.addEventListener('gamepadstartPressed', gamepad.throttle((e) => this.gamepadStart(e), 1000));
        window.addEventListener('gamepadAxes', (e) => this.handleAxes(e.detail));
        window.addEventListener('gamepadl2Pressed', (e) => this.gamepadLeftTrigger());
        window.addEventListener('gamepadr2Pressed', gamepad.throttle((e) => this.gamepadRightTrigger(e), 50));
        window.addEventListener('gamepadr2Released', gamepad.throttle((e) => this.gamepadRightTriggerReleased(),50));

        window.addEventListener('gamepadl2Released', (e) => this.gamepadLeftTrigger());

    window.addEventListener('gamepadrightStickPressed', gamepad.throttle(() => {
        this.toggleSubmenu();
        this.flashR3Button();
    }, 500)); // 500ms delay
    },

    gamepadXButton: function(e) {
        console.log("X button held down");
        if (ui_overlay_window.remainingBullets < ui_overlay_window.bulletsPerRound && ui_overlay_window.remainingRounds > 0 && !ui_overlay_window.isReloading) {
            console.log("Starting manual reload");
            ui_overlay_window.startReloading(); // Start the reloading process
        } else if (ui_overlay_window.remainingRounds <= 0) {
            console.log("X button held - No rounds left");
            audio.playAudio("empty_gun", assets.use('empty_gun'), 'sfx', false);
        }
    },
    
    gamepadXButtonReleased: function(e) {
        if (ui_overlay_window.isReloading) {
            console.log("X button released - Stopping reload");
            ui_overlay_window.stopReloading(); // Stop the reloading process if the button is released
        }
        ui_overlay_window.justReloaded = false; // Allow reloading to start again on next press
    },

    gamepadYButtonPressed: function(e) {
        this.isYButtonHeld = true; // Set the flag to true when Y button is held
    },

    // Function to handle Y button release
    gamepadYButtonReleased: function(e) {
        this.isYButtonHeld = false; // Set the flag to false when Y button is released
    },

    gamepadLeftTrigger: function() {
        if (gamepad.buttons.includes('l2')) {
            if (game.mainSprite) {
                game.mainSprite.targetAim = true; // Enable aiming
            }
        } else {
            if (game.mainSprite) {
                game.mainSprite.targetAim = false; // Disable aiming when l2 is released
            }
        }
    },

    gamepadRightTrigger: function() {
        if (gamepad.buttons.includes('l2')) { 
            // If l2 is held, fire the weapon
            if (ui_overlay_window.remainingBullets > 0) {  // Check if bullets are available
                game.mainSprite.speed = 120; // Fire rate speed
                gamepad.vibrate(500, 1.0, 1.0); // Trigger vibration
    
                if (game.mainSprite.targetAim) {
                    game.mainSprite.dealDamage();
                    ui_overlay_window.updateBullets(ui_overlay_window.remainingBullets - 1);
                    audio.playAudio("machinegun1", assets.use('machinegun1'), 'sfx', true);
                    effects.shakeMap(200, 4);
                }
            } else {
                // Handle no bullets case
                audio.stopLoopingAudio('machinegun1', 'sfx', 1.0);
    
                if (ui_overlay_window.remainingBullets <= 0 && ui_overlay_window.remainingRounds > 0) {
                    console.log("Out of bullets! Press and hold 'X' on the gamepad to reload.");
                    audio.playAudio("empty_gun", assets.use('empty_gun'), 'sfx', false);
                    ui.notif("no_bullets_notif", `Out of bullets! Press and hold 'X' on the gamepad to reload.`, true);
                } else if (ui_overlay_window.remainingBullets <= 0 && ui_overlay_window.remainingRounds <= 0) {
                    console.log("No bullets and no rounds left");
                    audio.playAudio("empty_gun", assets.use('empty_gun'), 'sfx', false);
                    ui_overlay_window.noBulletsLeft();
                }
            }
        } else {
            // Sprinting when only r2 is pressed (without l2)
            game.mainSprite.speed = 170; // Sprint speed
            console.log("Sprinting", game.mainSprite.speed);
        }
    },    
    
    gamepadRightTriggerReleased: function() {
        this.changeSpeed();
        audio.stopLoopingAudio('machinegun1', 'sfx', 1.0); // Stop the machine gun sound
    },    
    
    changeSpeed: function() {
        game.mainSprite.speed = 70; // Reset to normal speed (default walking speed)
    },

    gamepadLeftBumper: function(e) {
        
        if (gamepad.buttons.includes('l1')) {
            const player = game.mainSprite;
            const nearestTarget = this.findNearestTarget(player.targetX, player.targetY, player.maxRange);
    
            if (nearestTarget) {
                const targetCenterX = nearestTarget.x + (nearestTarget.width ? nearestTarget.width / 2 : 0);
                const targetCenterY = nearestTarget.y + (nearestTarget.height ? nearestTarget.height / 2 : 0);
                if (this.isWithinMaxRange(nearestTarget, player)) {
                    player.targetX = targetCenterX;
                    player.targetY = targetCenterY;
                    player.targetAim = true;
                } else {
                    // If the nearest target is not within max range, set the aim tool position in the same direction
                    const playerCenterX = player.x + player.width / 2;
                    const playerCenterY = player.y + player.height / 2;
                    const deltaX = targetCenterX - playerCenterX;
                    const deltaY = targetCenterY - playerCenterY;
                    const angle = Math.atan2(deltaY, deltaX);
                    player.targetX = playerCenterX + Math.cos(angle) * player.maxRange;
                    player.targetY = playerCenterY + Math.sin(angle) * player.maxRange;
                    player.targetAim = true;
                }
            } else {
                player.targetAim = false;
            }
        } else {
            game.mainSprite.targetAim = false;
        }
    },       

    handleAxes: function(axes) {
        this.handleLeftAxes(axes);
        this.handleRightAxes(axes);
    },

    handleLeftAxes: function(axes) {
        const threshold = 0.1; // Dead zone threshold for minimal stick movement
        
        // Calculate axis pressures
        const leftStickX = axes[0];
        const leftStickY = axes[1];
    
        // Reset gamepad directions
        gamepad.directions = { left: false, right: false, up: false, down: false };
    
        // Only adjust speed if r2 is NOT being pressed
        if (!gamepad.buttons.includes('r2')) {
            if (Math.abs(leftStickX) > threshold || Math.abs(leftStickY) > threshold) {
                // The stick is moved beyond the threshold, so update speed and movement directions
                gamepad.axesPressures.leftStickX = Math.abs(leftStickX);
                gamepad.axesPressures.leftStickY = Math.abs(leftStickY);
    
                // Calculate speed based on stick pressure
                const pressure = Math.max(Math.abs(leftStickX), Math.abs(leftStickY));
                game.mainSprite.speed = 25 + (pressure * 60); // Speed ranges from 25 to 85 depending on pressure
    
                // Set directions based on movement angle
                const angle = Math.atan2(leftStickY, leftStickX); // Get angle in radians (-PI to PI)
                this.updateGamepadDirections(angle);
    
                // Update the sprite's directions based on combined states
                this.updateSpriteDirections();
            } else {
                // If stick is in the dead zone (neutral position), reset the speed to default
                gamepad.axesPressures.leftStickX = 0;
                gamepad.axesPressures.leftStickY = 0;
    
                // Reset speed to default when gamepad stick is released
                game.mainSprite.speed = 70; // Replace 70 with your default walking speed
    
                this.updateSpriteDirections();
            }
        } else {
            // If r2 is being pressed, don't adjust speed here, just update the directions
            if (Math.abs(leftStickX) > threshold || Math.abs(leftStickY) > threshold) {
                const angle = Math.atan2(leftStickY, leftStickX); // Get angle in radians (-PI to PI)
                this.updateGamepadDirections(angle);
                this.updateSpriteDirections();
            }
        }
    },    
    
    updateGamepadDirections: function(angle) {
        const up = (angle >= -Math.PI / 8 && angle < Math.PI / 8);             // Right
        const upRight = (angle >= Math.PI / 8 && angle < 3 * Math.PI / 8);     // SE
        const right = (angle >= 3 * Math.PI / 8 && angle < 5 * Math.PI / 8);   // Down
        const downRight = (angle >= 5 * Math.PI / 8 && angle < 7 * Math.PI / 8); // SW
        const down = (angle >= 7 * Math.PI / 8 || angle < -7 * Math.PI / 8);   // Left
        const downLeft = (angle >= -7 * Math.PI / 8 && angle < -5 * Math.PI / 8); // NW
        const left = (angle >= -5 * Math.PI / 8 && angle < -3 * Math.PI / 8);  // Up
        const upLeft = (angle >= -3 * Math.PI / 8 && angle < -Math.PI / 8);    // NE
    
        // Set directions based on the angle ranges
        if (up) {
            gamepad.directions.right = true;
        } else if (upRight) {
            gamepad.directions.down = true;
            gamepad.directions.right = true;
        } else if (right) {
            gamepad.directions.down = true;
        } else if (downRight) {
            gamepad.directions.down = true;
            gamepad.directions.left = true;
        } else if (down) {
            gamepad.directions.left = true;
        } else if (downLeft) {
            gamepad.directions.up = true;
            gamepad.directions.left = true;
        } else if (left) {
            gamepad.directions.up = true;
        } else if (upLeft) {
            gamepad.directions.up = true;
            gamepad.directions.right = true;
        }
    },
 

    handleRightAxes: function(axes) {
        const deadZone = 0.1; // Dead zone threshold for minimal stick movement
    
        // Calculate axis pressures
        const rightStickX = axes[2];
        const rightStickY = axes[3];
    
        if (Math.abs(rightStickX) > deadZone || Math.abs(rightStickY) > deadZone) {
            gamepad.axesPressures.rightStickX = Math.abs(rightStickX);
            gamepad.axesPressures.rightStickY = Math.abs(rightStickY);
    
            // If the aim tool is active, update its position
            if (game.mainSprite && game.mainSprite.targetAim) {
                const aimSpeed = 10; // Adjust aim speed as necessary
                const newTargetX = game.mainSprite.targetX + rightStickX * aimSpeed;
                const newTargetY = game.mainSprite.targetY + rightStickY * aimSpeed;
    
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
    
            // Zooming logic removed from here
        } else {
            // Reset the pressures for right stick if below dead zone threshold
            gamepad.axesPressures.rightStickX = 0;
            gamepad.axesPressures.rightStickY = 0;
        }
    },    

    isWithinMaxRange: function(target, player) {
        const targetCenterX = target.x + (target.width ? target.width / 2 : 0);
        const targetCenterY = target.y + (target.height ? target.height / 2 : 0);
        const playerCenterX = player.x + player.width / 2;
        const playerCenterY = player.y + player.height / 2;
        const deltaX = targetCenterX - playerCenterX;
        const deltaY = targetCenterY - playerCenterY;
        const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
        return distance <= player.maxRange;
    },

    updateAimToolPosition: function() {
        const sprite = game.mainSprite;
        const aheadDistance = 30; // Distance ahead of the sprite to set the aim tool
        const directionOffsets = {
            'N': { x: 0, y: -aheadDistance },
            'S': { x: 0, y: aheadDistance },
            'E': { x: aheadDistance, y: 0 },
            'W': { x: -aheadDistance, y: 0 },
            'NE': { x: aheadDistance / Math.sqrt(2), y: -aheadDistance / Math.sqrt(2) },
            'NW': { x: -aheadDistance / Math.sqrt(2), y: -aheadDistance / Math.sqrt(2) },
            'SE': { x: aheadDistance / Math.sqrt(2), y: aheadDistance / Math.sqrt(2) },
            'SW': { x: -aheadDistance / Math.sqrt(2), y: aheadDistance / Math.sqrt(2) }
        };

        const offset = directionOffsets[sprite.direction] || { x: 0, y: 0 };

        sprite.targetX = sprite.x + sprite.width / 2 + offset.x;
        sprite.targetY = sprite.y + sprite.height / 2 + offset.y;

        // Clamp the target position within the canvas bounds
        sprite.targetX = Math.max(0, Math.min(sprite.targetX, game.worldWidth));
        sprite.targetY = Math.max(0, Math.min(sprite.targetY, game.worldHeight));
    },

    findNearestTarget: function(centerX, centerY, maxRadius) {
        let nearestTarget = null;
        let nearestDistance = Infinity;

        // Check sprites (enemies)
        for (let id in game.sprites) {
            const sprite = game.sprites[id];
            if (sprite.isEnemy) {
                const spriteCenterX = sprite.x + sprite.width / 2;
                const spriteCenterY = sprite.y + sprite.height / 2;
                const distance = Math.sqrt(
                    (centerX - spriteCenterX) ** 2 +
                    (centerY - spriteCenterY) ** 2
                );
                if (distance < nearestDistance && distance <= maxRadius) {
                    nearestDistance = distance;
                    nearestTarget = sprite;
                }
            }
        }

        // Check objects
        if (game.roomData && game.roomData.items) {
            game.roomData.items.forEach(item => {
                const itemData = assets.use('objectData')[item.id];
                if (itemData) {
                    const itemCenterX = item.x[0] * 16 + 8; // Center X coordinate
                    const itemCenterY = item.y[0] * 16 + 8; // Center Y coordinate
                    const distance = Math.sqrt(
                        (centerX - itemCenterX) ** 2 +
                        (centerY - itemCenterY) ** 2
                    );
                    if (distance < nearestDistance && distance <= maxRadius) {
                        nearestDistance = distance;
                        nearestTarget = { ...item, x: itemCenterX, y: itemCenterY }; // Include center coordinates
                    }
                }
            });
        }

        return nearestTarget;
    },

    keyDown: function(e) {
        if (game.isEditMode) return; // Prevent key presses in edit mode
        if (!game.allowControls) return;
        game.updateInputMethod('keyboard');
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            if (e.key === 'Tab') {
                e.preventDefault();
            }
            if (e.key === ' ') {
                e.preventDefault(); // Prevent default behavior for Space bar
            }
    
            // Check if 'X' is pressed for reloading
            if (e.key.toLowerCase() === 'x') {
                if (ui_overlay_window.remainingRounds > 0 && !ui_overlay_window.reloadInterval) {
                    ui_overlay_window.startReloading(); // Start the reloading process
                } else if (ui_overlay_window.remainingBullets === 0 && ui_overlay_window.remainingRounds > 0) {
                    ui_overlay_window.handleReload(); // Handle reload if bullets are at zero
                } else {
                    console.log("No rounds left to reload");
                    audio.playAudio("empty_gun", assets.use('empty_gun'), 'sfx', false);
                }
            }
    
            this.handleKeyDown(e);
        }
    },
    
    keyUp: function(e) {
        if (game.isEditMode) return; // Prevent key releases in edit mode
        game.updateInputMethod('keyboard');
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            e.preventDefault(); // Prevent default action for keyUp
            this.handleKeyUp(e);
        }
    
        if (e.key.toLowerCase() === 'x') {
            ui_overlay_window.stopReloading(); // Stop the reloading process if 'X' is released
        }
    },

    handleKeyDown: function(e) {
        this.handleControlStateChange(e, true);
    
        if (e.altKey && e.key === 'c') {
            if (e.key === 'c') {
                modal.load({ id: 'mishell_window', url: 'mishell/index.php', name: 'Mishell', drag: true, reload: true });
            }
        } else if (e.key === 'Tab') {
            e.preventDefault();
            // Load the editor when Tab is pressed
            modal.load({ id: 'edit_mode_window', url: 'editor/index.php', name: 'Editor', drag: true, reload: true });
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
        if (game.isEditMode) return; // Prevent mouse clicks in edit mode
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
        if (game.isEditMode) return; // Prevent mouse movement in edit mode
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
        if (game.isEditMode) return; // Prevent mouse up in edit mode
        this.isDragging = false;
        document.body.classList.remove('move-cursor');
    },

    mouseWheelScroll: function(e) {

    },    

    leftClick: function(e) {
        if (game.isEditMode) return; // Prevent left clicks in edit mode
        game.updateInputMethod('keyboard');
        console.log("left button clicked");
        if (e.target.matches('[data-close], [data-esc]')) {
            console.log("data close clicked");
            var parent = modal.closest(e.target);
            modal.close(parent);
        }
    },
    
    rightClick: function(e) {
        if (game.isEditMode) return; // Prevent right clicks in edit mode
        e.preventDefault();
        game.updateInputMethod('keyboard');
        console.log("right button clicked");
        this.cancelPathfinding(game.mainSprite);
    },

    doubleClick: function(e) {},

    flashR3Button: function() {
        const r3Button = document.getElementById('toggle-submenu');
        if (r3Button) {
            // Apply the temporary color change
            r3Button.classList.add('bg-green-500');  // Change this to your desired highlight color
            
            // Revert back to the original color after a short delay
            setTimeout(() => {
                r3Button.classList.remove('bg-green-500');
            }, 200); // 200ms for the color flash duration
        }
    },

    toggleSubmenu: function() {
        const submenu = document.getElementById('submenu');
        if (submenu) {
            submenu.classList.toggle('max-h-0');
            submenu.classList.toggle('max-h-[500px]'); // Adjust based on your content height
        }
    },

    cancelPathfinding: function(sprite) {
        if (sprite && sprite.isMovingToTarget) {
            sprite.isMovingToTarget = false;
            sprite.path = [];
            sprite.moving = false; // Reset the moving flag
            audio.stopLoopingAudio('footsteps1', 'sfx', 0.5); // Stop walking audio
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
        if (!game.allowControls) return; // Prevent control updates when controls are disabled
    
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
            audio.stopLoopingAudio('footsteps1', 'sfx', 0.5);
        }
    },

    gamepadStart: function() {
        console_window.toggleConsoleWindow(true, 'servers');
    }
};
