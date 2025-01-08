input = {
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
        window.addEventListener('gamepadyPressed', gamepad.throttle((e) => this.gamepadYButtonPressed(e), 1000));
        window.addEventListener('gamepadyReleased', gamepad.throttle((e) => this.gamepadYButtonReleased(e), 1000));
        window.addEventListener('gamepadxPressed', (e) => input.gamepadXButton(e));
        window.addEventListener('gamepadxReleased', (e) => input.gamepadXButtonReleased(e));
        window.addEventListener('gamepady', (e) => this.gamepadYButton(e));
        window.addEventListener('gamepadl1Pressed', (e) => this.gamepadLeftBumper(e.detail));
        window.addEventListener('gamepadstartPressed', gamepad.throttle((e) => this.gamepadStart(e), 1000));
        window.addEventListener('gamepadAxes', (e) => this.handleAxes(e.detail));
        window.addEventListener('gamepadl2Pressed', (e) => this.gamepadLeftTrigger(e));
        window.addEventListener('gamepadr2Pressed', gamepad.throttle((e) => this.gamepadRightTrigger(e), 50));
        window.addEventListener('gamepadr2Released', gamepad.throttle((e) => this.gamepadRightTriggerReleased(),50));
        window.addEventListener('gamepadl2Released', (e) => this.gamepadLeftTrigger());

    window.addEventListener('gamepadrightStickPressed', gamepad.throttle(() => {
        this.toggleSubmenu();
        this.flashR3Button();
    }, 500));
    
    },

    updateInputMethod: function(method, name = '') {
        const inputMethodDisplay = document.getElementById('input_method');
        if (inputMethodDisplay) {
            inputMethodDisplay.innerText = `Input: ${method}${name ? ' (' + name + ')' : ''}`;
        }
    },

    gamepadXButton: function(e) {
        console.log("X button held down");
        if (ui_overlay_window.remainingBullets < ui_overlay_window.bulletsPerRound && ui_overlay_window.remainingRounds > 0 && !ui_overlay_window.isReloading) {
            console.log("Starting manual reload");
            ui_overlay_window.startReloading();
        } else if (ui_overlay_window.remainingRounds <= 0) {
            console.log("X button held - No rounds left");
            audio.playAudio("empty_gun", assets.use('empty_gun'), 'sfx', false);
        }
    },
    
    gamepadXButtonReleased: function(e) {
        if (ui_overlay_window.isReloading) {
            console.log("X button released - Stopping reload");
            ui_overlay_window.stopReloading();
        }
        ui_overlay_window.justReloaded = false;
    },

    gamepadYButtonPressed: function () {
        if (!game.mainSprite || !game.sprites[game.playerid]) return;

        this.isYButtonHeld = true;
    
        const currentSprite = game.mainSprite;
        const currentPlayerId = game.playerid;
    
        const radius = 32; // Define the radius in pixels (2 tiles for 16x16 tiles)
    
        if (currentSprite.isVehicle) {
            // Switch back to the player
            const playerSprite = game.sprites[currentSprite.riderId || currentPlayerId];
            if (playerSprite) {
                // Update player's position to match the vehicle's position
                playerSprite.x = currentSprite.x;
                playerSprite.y = currentSprite.y;
    
                // Restore visibility of the player sprite
                playerSprite.activeSprite = true;
    
                // Update the player ID
                game.playerid = playerSprite.id;
    
                // Reset the rider ID on the vehicle
                currentSprite.riderId = null;

                plugin.show('ui_inventory_window');

            }

        } else {
            // Find a nearby vehicle within the specified radius
            const nearbyVehicle = Object.values(game.sprites).find(sprite => {
                if (!sprite.isVehicle) return false;
    
                // Calculate distance between player and vehicle
                const dx = sprite.x - currentSprite.x;
                const dy = sprite.y - currentSprite.y;
                const distance = Math.sqrt(dx * dx + dy * dy);
                return distance <= radius;
            });
    
            if (nearbyVehicle) {
                // Update playerid to the vehicle's id and store the player's id in the vehicle
                game.playerid = nearbyVehicle.id;
                nearbyVehicle.riderId = currentPlayerId;
    
                // Hide the player sprite
                currentSprite.activeSprite = false;

                plugin.minimize('ui_inventory_window');
                plugin.front('ui_overlay_window');
            } else {
                console.log("No nearby vehicle within radius to switch to.");
            }

        }
    
        // Update the mainSprite to reflect the new active sprite
        game.mainSprite = game.sprites[game.playerid];
    
        console.log(`Switched control to sprite with ID: ${game.playerid}`);
    },
    
    

    gamepadYButtonReleased: function(e) {
        this.isYButtonHeld = false;
    },

    gamepadLeftTrigger: function(event) {
        const sprite = game.mainSprite;
    
        if (!sprite) {
            console.error("No sprite detected for L2 action.");
            return;
        }
    
        const pressure = event?.detail?.pressure || 0; // pressure from the trigger
    
        if (sprite.isVehicle) {
            if (pressure > 0) {
                // If vehicle is moving forward, brake
                if (sprite.currentSpeed > 0) {
                    sprite.currentSpeed = Math.max(
                        0,
                        sprite.currentSpeed - sprite.braking * pressure * (game.deltaTime / 16.67)
                    );
                    console.log("Braking Vehicle, Current Speed:", sprite.currentSpeed);
                } else {
                    // Go full reverse speed
                    sprite.currentSpeed = Math.max(
                        -sprite.maxSpeed,
                        sprite.currentSpeed - (sprite.acceleration * 10) * pressure * (game.deltaTime / 16.67)
                    );
                    console.log("Reversing Vehicle at higher speed, Current Speed:", sprite.currentSpeed);
                }
    
                // Update vehicle movement
                sprite.moveVehicle();
            }
        } else {
            // Non-vehicle logic remains unchanged
            if (gamepad.buttons.includes('l2')) {
                if (sprite) {
                    sprite.targetAim = true;
                }
            } else {
                if (sprite) {
                    sprite.targetAim = false;
                }
            }
        }
    },
    
    

    gamepadRightTrigger: function(event) {
        const sprite = game.mainSprite;
    
        if (!sprite) {
            console.error("No sprite detected for R2 action.");
            return;
        }
    
        const pressure = event.detail.pressure || 0;
        console.log("R2 Pressure (from event):", pressure);
    
        if (sprite.isVehicle) {
            // Acceleration logic for vehicles
            if (pressure > 0) {
                sprite.currentSpeed = Math.min(
                    sprite.maxSpeed,
                    sprite.currentSpeed + sprite.acceleration * pressure * (game.deltaTime / 16.67) // Scale with deltaTime
                );
                console.log("Accelerating Vehicle, Current Speed:", sprite.currentSpeed);
            } else {
                sprite.currentSpeed = Math.max(
                    0,
                    sprite.currentSpeed - sprite.braking * (game.deltaTime / 16.67) // Smooth deceleration
                );
                console.log("Decelerating Vehicle, Current Speed:", sprite.currentSpeed);
            }
    
            // Move the vehicle
            sprite.moveVehicle();
        } else if (sprite.canShoot) {
            // Shooting logic
            if (sprite.targetAim && sprite.canShoot) {
                if (ui_overlay_window.remainingBullets > 0) {
                    sprite.dealDamage();
                    ui_overlay_window.updateBullets(ui_overlay_window.remainingBullets - 1);
                    audio.playAudio("machinegun1", assets.use('machinegun1'), 'sfx', true);
                    effects.shakeMap(200, 2);
                    sprite.overrideAnimation = 'shooting_gun';
                } else {
                    audio.stopLoopingAudio('machinegun1', 'sfx', 1.0);
                    if (ui_overlay_window.remainingRounds > 0) {
                        console.log("Out of bullets! Reload needed.");
                        audio.playAudio("empty_gun", assets.use('empty_gun'), 'sfx', false);
                        ui.notif("no_bullets_notif", `Out of bullets! Press 'X' to reload.`, true);
                        sprite.overrideAnimation = null;
                    } else {
                        console.log("No bullets and no rounds left");
                        audio.playAudio("empty_gun", assets.use('empty_gun'), 'sfx', false);
                        ui_overlay_window.noBulletsLeft();
                    }
                }
            }
        } else {
            console.log("R2 pressed but no applicable action for the sprite.");
        }
    },
    
      
    
    gamepadRightTriggerReleased: function() {
        if (!game.mainSprite) {
            return;
        }
        this.changeSpeed();
        audio.stopLoopingAudio('machinegun1', 'sfx', 1.0);
        const player = game.mainSprite;
        player.changeAnimation('shooting_gun');
    },    
    
    changeSpeed: function() {
        game.mainSprite.speed = 70;
        game.mainSprite.overrideAnimation = null;
    },

    gamepadLeftBumper: function(e) {
        if (!game.mainSprite) {
            return;
        }
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
        const threshold = 0.1; // Deadzone threshold
        const leftStickX = axes[0];
        const leftStickY = axes[1];
    
        if (!game.mainSprite) {
            return;
        }
    
        const sprite = game.mainSprite;
    
        if (sprite.isVehicle) {
            // Adjust steering gradually via angle updates
            if (Math.abs(leftStickX) > threshold) {
                sprite.updateVehicleDirection(leftStickX, game.deltaTime);
            } else {
                // If no steering input, angle remains unchanged
                sprite.updateVehicleDirection(0, game.deltaTime); 
            }
    
            // Handle acceleration/braking with triggers (already done in gamepad triggers)
            // If neither R2 nor L2 are pressed significantly, apply gentle deceleration
            const r2Pressure = gamepad.axesPressures.rightTrigger || 0;
            const l2Pressure = gamepad.axesPressures.leftTrigger || 0;
    
            // Acceleration handled in gamepadRightTrigger, braking in gamepadLeftTrigger.
            // If no input on R2:
            if (r2Pressure < threshold && l2Pressure < threshold) {
                sprite.currentSpeed = Math.max(
                    0,
                    sprite.currentSpeed - sprite.braking * (game.deltaTime / 1000)
                );
            }
    
            // Move the vehicle after updates
            sprite.moveVehicle();
        } else {
            // Non-vehicle logic remains the same
            gamepad.directions = { left: false, right: false, up: false, down: false };
    
            if (Math.abs(leftStickX) > threshold || Math.abs(leftStickY) > threshold) {
                const pressure = Math.min(1, Math.sqrt(leftStickX ** 2 + leftStickY ** 2));
                const minSpeed = 10;
                const topSpeed = sprite.topSpeed;
                const r2Boost = gamepad.buttons.includes('r2') ? 1.0 : 0.6;
                const relativeSpeed = minSpeed + (pressure * (topSpeed - minSpeed) * r2Boost);
    
                sprite.speed = Math.min(relativeSpeed, topSpeed);
                const angle = Math.atan2(leftStickY, leftStickX);
    
                this.updateGamepadDirections(angle);
                this.updateSpriteDirections();
    
                effects.dirtCloudEffect.create(sprite, '#DAF7A6');
            } else {
                gamepad.axesPressures.leftStickX = 0;
                gamepad.axesPressures.leftStickY = 0;
                if (sprite) {
                    sprite.speed = 0;
                }
                this.updateSpriteDirections();
            }
        }
    },    
    
    updateGamepadDirections: function(angle) {
        const up = (angle >= -Math.PI / 8 && angle < Math.PI / 8);
        const upRight = (angle >= Math.PI / 8 && angle < 3 * Math.PI / 8);
        const right = (angle >= 3 * Math.PI / 8 && angle < 5 * Math.PI / 8);
        const downRight = (angle >= 5 * Math.PI / 8 && angle < 7 * Math.PI / 8);
        const down = (angle >= 7 * Math.PI / 8 || angle < -7 * Math.PI / 8);
        const downLeft = (angle >= -7 * Math.PI / 8 && angle < -5 * Math.PI / 8);
        const left = (angle >= -5 * Math.PI / 8 && angle < -3 * Math.PI / 8);
        const upLeft = (angle >= -3 * Math.PI / 8 && angle < -Math.PI / 8);
    
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
        if (!game.mainSprite) {
            return;
        }
        const deadZone = 0.1;
        const rightStickX = axes[2];
        const rightStickY = axes[3];
    
        if (Math.abs(rightStickX) > deadZone || Math.abs(rightStickY) > deadZone) {
            gamepad.axesPressures.rightStickX = Math.abs(rightStickX);
            gamepad.axesPressures.rightStickY = Math.abs(rightStickY);
    
            if (game.mainSprite && game.mainSprite.targetAim) {
                const aimSpeed = 10;
                const newTargetX = game.mainSprite.targetX + rightStickX * aimSpeed;
                const newTargetY = game.mainSprite.targetY + rightStickY * aimSpeed;
                const deltaX = newTargetX - (game.mainSprite.x + game.mainSprite.width / 2);
                const deltaY = newTargetY - (game.mainSprite.y + game.mainSprite.height / 2);
                const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
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
    
                if (distance <= game.mainSprite.maxRange) {
                    game.mainSprite.targetX = newTargetX;
                    game.mainSprite.targetY = newTargetY;
                } else {
                    const maxRangeX = game.mainSprite.x + game.mainSprite.width / 2 + Math.cos(angle) * game.mainSprite.maxRange;
                    const maxRangeY = game.mainSprite.y + game.mainSprite.height / 2 + Math.sin(angle) * game.mainSprite.maxRange;
                    game.mainSprite.targetX = Math.max(0, Math.min(maxRangeX, game.worldWidth));
                    game.mainSprite.targetY = Math.max(0, Math.min(maxRangeY, game.worldHeight));
                }
            }
    
        } else {
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
        if (!game.mainSprite) {
            return;
        }
        const sprite = game.mainSprite;
        const aheadDistance = 30;
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
        sprite.targetX = Math.max(0, Math.min(sprite.targetX, game.worldWidth));
        sprite.targetY = Math.max(0, Math.min(sprite.targetY, game.worldHeight));
    },

    findNearestTarget: function(centerX, centerY, maxRadius) {
        let nearestTarget = null;
        let nearestDistance = Infinity;

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

        if (game.roomData && game.roomData.items) {
            game.roomData.items.forEach(item => {
                const itemData = assets.use('objectData')[item.id];
                if (itemData) {
                    const itemCenterX = item.x[0] * 16 + 8;
                    const itemCenterY = item.y[0] * 16 + 8;
                    const distance = Math.sqrt(
                        (centerX - itemCenterX) ** 2 +
                        (centerY - itemCenterY) ** 2
                    );
                    if (distance < nearestDistance && distance <= maxRadius) {
                        nearestDistance = distance;
                        nearestTarget = { ...item, x: itemCenterX, y: itemCenterY };
                    }
                }
            });
        }

        return nearestTarget;
    },

    keyDown: function(e) {
        if (game.isEditMode || this.isFormInputFocused()) return;
        if (!game.allowControls) return;
        this.updateInputMethod('keyboard');
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            if (e.key === 'Tab') {
                e.preventDefault();
            }
            if (e.key === ' ') {
                e.preventDefault();
            }
    
            if (e.key.toLowerCase() === 'x') {
                if (ui_overlay_window.remainingRounds > 0 && !ui_overlay_window.reloadInterval) {
                    ui_overlay_window.startReloading();
                } else if (ui_overlay_window.remainingBullets === 0 && ui_overlay_window.remainingRounds > 0) {
                    ui_overlay_window.handleReload();
                } else {
                    console.log("No rounds left to reload");
                    audio.playAudio("empty_gun", assets.use('empty_gun'), 'sfx', false);
                }
            }
    
            this.handleKeyDown(e);
        }
    },
    
    keyUp: function(e) {
        if (game.isEditMode || this.isFormInputFocused()) return;
        this.updateInputMethod('keyboard');
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            e.preventDefault();
            this.handleKeyUp(e);
        }
    
        if (e.key.toLowerCase() === 'x') {
            ui_overlay_window.stopReloading();
        }
    },

    handleKeyDown: function(e) {
        this.handleControlStateChange(e, true);
    
        if (e.key === 'Tab') {
            e.preventDefault();
            plugin.load({
                id: 'console_window',
                url: 'console/index.php',
                name: 'console',
                drag: false,
                reload: true,
                onAfterLoad: function (id) {
                    plugin.load({ id: 'edit_mode_window', url: 'editor/index.php', name: 'Editor', drag: true, reload: true });
                }
            });

        } else {
            const dir = this.keys[e.key];
            if (dir) {
                this.directions[dir] = true;
                this.updateSpriteDirections();
            }
        }
    
        if (e.key === 'f') {
            if (game.mainSprite) {
                game.mainSprite.targetAim = !game.mainSprite.targetAim;
                if (game.mainSprite.targetAim) {
                    console.log('Target aim activated');
                } else {
                    console.log('Target aim deactivated');
                }
            } else {
                console.error('Main sprite not found.');
            }
        } else if (e.key === ' ') {
            utils.fullScreen();
        }
    },

    handleKeyUp: function(e) {
        this.handleControlStateChange(e, false);

        if (e.keyCode === 27) {
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
        if (game.isEditMode) return;
        if (e.button === 1) {
            this.isDragging = true;
            this.startX = e.clientX;
            this.startY = e.clientY;
            document.body.classList.add('move-cursor');
        }
    
        if (e.button === 2) {
            this.cancelPathfinding(game.mainSprite);
        }
    },
    
    mouseMove: function(e) {
        if (!game.mainSprite) return;
        if (game.isEditMode) return;
        if (this.isDragging) {
            const dx = (this.startX - e.clientX) / game.zoomLevel;
            const dy = (this.startY - e.clientY) / game.zoomLevel;
    
            camera.cameraX = Math.max(0, Math.min(game.worldWidth - window.innerWidth / game.zoomLevel, camera.cameraX + dx));
            camera.cameraY = Math.max(0, Math.min(game.worldHeight - window.innerHeight / game.zoomLevel, camera.cameraY + dy));
    
            this.startX = e.clientX;
            this.startY = e.clientY;
        }
    
        if (game.mainSprite && game.mainSprite.targetAim) {
            const rect = game.canvas.getBoundingClientRect();
            const newX = (e.clientX - rect.left) / game.zoomLevel + camera.cameraX;
            const newY = (e.clientY - rect.top) / game.zoomLevel + camera.cameraY;
    
            game.mainSprite.targetX = newX;
            game.mainSprite.targetY = newY;
        }
    },
    
    mouseUp: function(e) {
        if (game.isEditMode) return;
        this.isDragging = false;
        document.body.classList.remove('move-cursor');
    },

    mouseWheelScroll: function(e) {

    },    

    leftClick: function(e) {
        if (game.isEditMode) return;
        this.updateInputMethod('keyboard');
        console.log("left button clicked");
        if (e.target.matches('[data-close], [data-esc]')) {
            console.log("data close clicked");
            var parent = plugin.closest(e.target);
            plugin.close(parent);
        }
    },
    
    rightClick: function(e) {
        if (game.isEditMode) return;
        e.preventDefault();
        this.updateInputMethod('keyboard');
        console.log("right button clicked");
        this.cancelPathfinding(game.mainSprite);
    },

    doubleClick: function(e) {},

    flashR3Button: function() {
        const r3Button = document.getElementById('toggle-submenu');
        if (r3Button) {
            r3Button.classList.add('bg-green-500');
            setTimeout(() => {
                r3Button.classList.remove('bg-green-500');
            }, 200);
        }
    },

    toggleSubmenu: function() {
        const submenu = document.getElementById('submenu');
        if (submenu) {
            submenu.classList.toggle('max-h-0');
            submenu.classList.toggle('max-h-[500px]');
        }
    },

    cancelPathfinding: function(sprite) {
        if (sprite && sprite.isMovingToTarget) {
            sprite.isMovingToTarget = false;
            sprite.path = [];
            sprite.moving = false;
            audio.stopLoopingAudio('footsteps1', 'sfx', 0.5);
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
        if (!game.allowControls) return;
    
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
    
        if (game.mainSprite && !combinedDirections.up && !combinedDirections.down && !combinedDirections.left && !combinedDirections.right) {
            audio.stopLoopingAudio('footsteps1', 'sfx', 0.5);
        }
    },

    gamepadStart: function() {
        plugin.load({ id: 'overview_menu', url: 'menus/overview/index.php', name:'start menu', reload: true, drag: false, hidden: false })
    },

    isFormInputFocused: function() {
        const activeElement = document.activeElement;
        return (
            activeElement &&
            (
                activeElement.tagName === 'INPUT' ||
                activeElement.tagName === 'TEXTAREA' ||
                activeElement.tagName === 'SELECT' ||
                activeElement.isContentEditable
            )
        );
    }    
};
