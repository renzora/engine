var render = {
    spriteCount: 0,
    animationCount: 0,
    backgroundTileCount: 0,  // Count background tiles in the viewport
    tileCount: 0,  // Count room tiles in the viewport
    overlappingTiles: [],
    renderQueue: [],   
    parseRange: function(rangeString) {
        const [start, end] = rangeString.split('-').map(Number);
        const rangeArray = [];

        if (start > end) {
            for (let i = start; i >= end; i--) {
                rangeArray.push(i);
            }
        } else {
            for (let i = start; i <= end; i++) {
                rangeArray.push(i);
            }
        }

        return rangeArray;
    },

    expandTileData: function(tileData) {
        const expandedTileData = { ...tileData };

        if (Array.isArray(tileData.i)) {
            expandedTileData.i = tileData.i.map(frame => {
                if (Array.isArray(frame)) {
                    return frame.map(value => {
                        if (typeof value === 'string' && value.includes('-')) {
                            return this.parseRange(value);
                        }
                        return value;
                    }).flat();  // Flatten after expanding ranges
                } else if (typeof frame === 'string' && frame.includes('-')) {
                    return this.parseRange(frame);
                }
                return frame;
            });
        }

        return expandedTileData;
    },

updateGameLogic: function(deltaTime) {
    gamepad.updateGamepadState();

    const viewportXStart = Math.floor(camera.cameraX / 16);
    const viewportXEnd = Math.ceil((camera.cameraX + window.innerWidth / game.zoomLevel) / 16);
    const viewportYStart = Math.floor(camera.cameraY / 16);
    const viewportYEnd = Math.ceil((camera.cameraY + window.innerHeight / game.zoomLevel) / 16);

    for (let id in game.sprites) {
        const sprite = game.sprites[id];
        const spriteRight = sprite.x + sprite.width;
        const spriteBottom = sprite.y + sprite.height;

        // Only update sprites in the viewport
        if (
            spriteRight >= viewportXStart * 16 && sprite.x < viewportXEnd * 16 &&
            spriteBottom >= viewportYStart * 16 && sprite.y < viewportYEnd * 16
        ) {
            if (sprite.update) {
                sprite.update(deltaTime);
            }
        }
    }

    camera.update();
    utils.gameTime.update(deltaTime);
    lighting.updateDayNightCycle();
    lighting.updateLights();
    animate.updateAnimatedTiles(deltaTime);
    weather.updateSnow(deltaTime);
    weather.updateRain(deltaTime);
    weather.updateFireflys(deltaTime);
    weather.updateClouds(deltaTime);
    particles.updateParticles(deltaTime);
    effects.transitions.update();

    if (weather.rainActive) {
        audio.playAudio("rain", assets.use('rain'), 'ambience', true);
    } else {
        audio.stopLoopingAudio('rain', 'ambience', 0.5);
    }

    actions.checkForNearbyItems();
},


    // Modify renderBackground to use cached background for the entire world
renderBackground: function(viewportXStart, viewportXEnd, viewportYStart, viewportYEnd) {
    this.backgroundTileCount = 0; // Reset background tile count
    
    // Check if sceneBg is null
    if (!game.sceneBg) {
        return;
    }

    const bgTileData = game.objectData[game.sceneBg][0];
    const tileSize = 16;

    // Loop through only the tiles in the viewport
    for (let y = Math.floor(viewportYStart); y <= Math.floor(viewportYEnd); y++) {
        for (let x = Math.floor(viewportXStart); x <= Math.floor(viewportXEnd); x++) {
            const posX = x * tileSize;
            const posY = y * tileSize;

            const tileFrameIndex = bgTileData.i;
            const srcX = (tileFrameIndex % 150) * tileSize;
            const srcY = Math.floor(tileFrameIndex / 150) * tileSize;

            // Draw each background tile inside the viewport
            game.ctx.drawImage(assets.use(bgTileData.t), srcX, srcY, tileSize, tileSize, posX, posY, tileSize, tileSize);

            // Increment the visible background tile count
            this.backgroundTileCount++;
            this.tileCount++; // Increment total tile count
        }
    }
    utils.tracker('render.renderBackground');
},

    renderAll: function(viewportXStart, viewportXEnd, viewportYStart, viewportYEnd) {
        const renderQueue = [];
        this.backgroundTileCount = 0;
        this.tileCount = 0;
        this.spriteCount = 0;
    
        // Apply expandTileData to objectData
        const expandedObjectData = Object.keys(game.objectData).reduce((acc, key) => {
            acc[key] = game.objectData[key].map(this.expandTileData.bind(this));
            return acc;
        }, {});
    
        // 3. Render the game objects
        if (game.roomData && game.roomData.items) {
            game.roomData.items.forEach(roomItem => {
                const itemData = expandedObjectData[roomItem.id];
                if (itemData && itemData.length > 0) {
                    const tileData = itemData[0];
                    const xCoordinates = roomItem.x || [];
                    const yCoordinates = roomItem.y || [];
    
                    // Calculate the center (or bottom) of the object for correct rotation
                    const centerX = (Math.min(...xCoordinates) + Math.max(...xCoordinates)) / 2;
                    const bottomY = Math.max(...yCoordinates);
    
                    // Handle rotation for the entire object
                    let rotation = tileData.rotation || 0;
                    if (roomItem.isRotating) {
                        actions.handleRotation(roomItem);  // Apply the rotation logic
                        rotation = roomItem.rotation;      // Use the updated rotation value
                    }
    
                    // Apply sway if necessary
                    if (tileData.sway === true) {
                        rotation += this.handleSway(roomItem);
                    }
    
                    let index = 0;
                    for (let i = 0; i < yCoordinates.length; i++) {
                        const y = yCoordinates[i];  // Use the decimal values for y
                        for (let j = 0; j < xCoordinates.length; j++) {
                            const x = xCoordinates[j];  // Use the decimal values for x
    
                            // Check if any part of the tile is inside the viewport (edge case handling)
                            if ((x * 16 + 16) >= (viewportXStart * 16) && (x * 16) < (viewportXEnd * 16) &&
                                (y * 16 + 16) >= (viewportYStart * 16) && (y * 16) < (viewportYEnd * 16)) {
    
                                const posX = (x - centerX) * 16;
                                const posY = (y - bottomY) * 16;
    
                                let tileFrameIndex;
                                if (Array.isArray(tileData.i[0])) {
                                    const animationData = tileData.i;
                                    const currentFrame = tileData.currentFrame || 0;
                                    tileFrameIndex = animationData[currentFrame][index % animationData[currentFrame].length];
                                } else {
                                    tileFrameIndex = tileData.i[index];
                                }
    
                                if (tileFrameIndex !== undefined) {
                                    const srcX = (tileFrameIndex % 150) * 16;
                                    const srcY = Math.floor(tileFrameIndex / 150) * 16;
    
                                    // Determine the z (zIndex) with fallback to 1 if undefined
                                    let z;
                                    if (Array.isArray(tileData.z)) {
                                        z = tileData.z[index % tileData.z.length];
                                    } else {
                                        z = tileData.z;
                                    }
    
                                    if (z === undefined) {
                                        z = 1;  // Fallback value
                                    }
    
                                    renderQueue.push({
                                        zIndex: z,
                                        draw: () => {
                                            game.ctx.save();
    
                                            // Translate to the center of the object before rotating
                                            const centerXPixel = centerX * 16;
                                            const bottomYPixel = bottomY * 16;
                                            game.ctx.translate(centerXPixel, bottomYPixel);
    
                                            // Apply rotation to the entire object
                                            game.ctx.rotate(rotation);
    
                                            // Now render each tile relative to the object's center
                                            game.ctx.drawImage(assets.use(tileData.t), srcX, srcY, 16, 16, posX, posY, 16, 16);
    
                                            game.ctx.restore();
                                        }
                                    });
    
                                    this.tileCount++;
                                }
                            }
                            index++;
                        }
                    }
    
                    this.handleLights(tileData, roomItem, viewportXStart, viewportXEnd, viewportYStart, viewportYEnd);
                    this.handleEffects(tileData, roomItem, viewportXStart, viewportXEnd, viewportYStart, viewportYEnd);
                }
            });
        }
    
        // Collect sprites for rendering
        for (let id in game.sprites) {
            const sprite = game.sprites[id];
            const spriteRight = sprite.x + sprite.width;
            const spriteBottom = sprite.y + sprite.height;
    
            if (spriteRight >= viewportXStart * 16 && sprite.x < viewportXEnd * 16 &&
                spriteBottom >= viewportYStart * 16 && sprite.y < viewportYEnd * 16) {
    
                renderQueue.push({
                    zIndex: 2,  // All sprites should have a zIndex of 2
                    draw: function () {
                        game.sprites[id].drawShadow();
                        weather.drawClouds();
                        render.renderPathfinderLine();
                        game.sprites[id].draw();
                        effects.bubbleEffect.updateAndRender(game.deltaTime);
                    }
                });
    
                this.spriteCount++;
            }
        }
    
        // Sort the renderQueue by zIndex before rendering
        renderQueue.sort((a, b) => a.zIndex - b.zIndex);
    
        // Draw the items in the renderQueue
        renderQueue.forEach(item => {
            item.draw();
        });

                // 2. Render the grid if enabled
        if (utils.objExists('editor_utils_window.renderGrid')) {
            editor_utils_window.renderGrid();
        }

        utils.tracker('render.renderAll');
    },    

    handleSway: function(roomItem) {
        // Initialize random sway values once per object to avoid jittering
        if (!roomItem.swayInitialized) {
            roomItem.swayAngle = Math.PI / (160 + Math.random() * 40); // Randomize between a subtle range
            roomItem.swaySpeed = 5000 + Math.random() * 2000; // Randomize between 5 and 7 seconds per cycle
            roomItem.swayInitialized = true; // Mark as initialized
        }

        // Only update sway when the object is within the viewport
        if (roomItem.isInViewport) {
            // Calculate the elapsed time for the sway effect
            const elapsedTime = roomItem.swayElapsed || 0;
            roomItem.swayElapsed = elapsedTime + game.deltaTime;

            // Use a sine wave for smooth back-and-forth motion, applying the precomputed random sway angle and speed
            const sway = Math.sin((roomItem.swayElapsed / roomItem.swaySpeed) * Math.PI * 2) * roomItem.swayAngle;

            return sway;
        }

        utils.tracker('render.handleSway');

        return 0;  // No sway if outside the viewport
    },

    initializeSway: function(roomItem) {
        roomItem.swayAngle = Math.PI / (160 + Math.random() * 40); // Randomize between a subtle range
        roomItem.swaySpeed = 5000 + Math.random() * 2000; // Randomize between 5 and 7 seconds per cycle
        roomItem.swayElapsed = 0;  // Initialize elapsed time
        roomItem.swayInitialized = true;  // Mark as initialized
    },
       

    renderPathfinderLine: function () {
        if (game.mainSprite && game.mainSprite.path && game.mainSprite.path.length > 0) {
            game.ctx.strokeStyle = 'rgba(255, 255, 0, 0.8)';
            game.ctx.lineWidth = 2;
            game.ctx.beginPath();

            game.ctx.moveTo(game.mainSprite.path[0].x * 16 + 8, game.mainSprite.path[0].y * 16 + 8);

            for (let i = 1; i < game.mainSprite.path.length - 1; i++) {
                const currentPoint = game.mainSprite.path[i];
                const nextPoint = game.mainSprite.path[i + 1];
                const midX = (currentPoint.x + nextPoint.x) * 8 + 8;
                const midY = (currentPoint.y + nextPoint.y) * 8 + 8;

                game.ctx.quadraticCurveTo(currentPoint.x * 16 + 8, currentPoint.y * 16 + 8, midX, midY);
            }

            const lastPoint = game.mainSprite.path[game.mainSprite.path.length - 1];
            game.ctx.lineTo(lastPoint.x * 16 + 8, lastPoint.y * 16 + 8);

            game.ctx.stroke();
        }
    },



    renderCarriedObjects: function () {
        if (game.mainSprite && game.mainSprite.isCarrying) {
            const carriedItemId = game.mainSprite.carriedItem;
            const itemX = game.mainSprite.x - 8;
            const itemY = game.mainSprite.y - 32 - (game.objectData[carriedItemId][0].b.length);

            game.drawCarriedObject(game.ctx, carriedItemId, itemX, itemY);
        }
    },

    handleDebugUtilities: function () {
        if (typeof debug_window !== 'undefined') {
            if (game.showGrid && debug_window.grid) {
                debug_window.grid();
            }
            if (game.showCollision && debug_window.tiles) {
                debug_window.tiles();
            }
            if (game.showTiles && debug_window.tiles) {
                debug_window.tiles();
            }
        }
    },

    handleLights: function (tileData, roomItem, viewportXStart, viewportXEnd, viewportYStart, viewportYEnd) {
        if (tileData.l && tileData.l.length > 0) {
            tileData.l.forEach(light => {
                if (Array.isArray(light) && light.length === 2) {
                    const lightXIndex = light[0];
                    const lightYIndex = light[1];

                    if (lightXIndex >= 0 && lightXIndex < roomItem.x.length &&
                        lightYIndex >= 0 && lightYIndex < roomItem.y.length) {

                        const tileX = roomItem.x[lightXIndex];
                        const tileY = roomItem.y[lightYIndex];

                        const posX = tileX * 16 + 8;
                        const posY = tileY * 16 + 8;
                        const radius = tileData.lr || 200;

                        const isInView = (posX + radius) >= (viewportXStart * 16) && (posX - radius) < (viewportXEnd * 16) &&
                            (posY + radius) >= (viewportYStart * 16) && (posY - radius) < (viewportYEnd * 16);

                        const lightId = `${roomItem.id}_${tileX}_${tileY}`;

                        const hours = utils.gameTime.hours;
                        const minutes = utils.gameTime.minutes;
                        const time = hours + minutes / 60;
                        const isNightTime = time >= 22 || time < 7;

                        if (isInView && isNightTime) {
                            const existingLight = effects.lights.find(light => light.id === lightId);

                            if (!existingLight) {
                                const color = tileData.lc || { r: 255, g: 255, b: 255 };
                                const intensity = tileData.li || 1;
                                const flickerSpeed = tileData.lfs || 0.03;
                                const flickerAmount = tileData.lfa || 0.04;
                                const lampType = tileData.lt || "lamp";

                                lighting.addLight(lightId, posX, posY, radius, color, intensity, lampType, true, flickerSpeed, flickerAmount);
                            }
                        } else {
                            lighting.lights = lighting.lights.filter(light => light.id !== lightId);
                        }
                    }
                }

            });
        }
    },

    handleEffects: function (tileData, roomItem, viewportXStart, viewportXEnd, viewportYStart, viewportYEnd) {
        if (tileData.fx && game.fxData[tileData.fx]) {
            const fxData = game.fxData[tileData.fx];

            tileData.fxp.forEach((fxPosition, fxIndex) => {
                const fxXIndex = fxPosition[0];
                const fxYIndex = fxPosition[1];

                if (fxXIndex >= 0 && fxXIndex < roomItem.x.length &&
                    fxYIndex >= 0 && fxYIndex < roomItem.y.length) {

                    const tileX = roomItem.x[fxXIndex];
                    const tileY = roomItem.y[fxYIndex];

                    const posX = tileX * 16 + 8;
                    const posY = tileY * 16 + 8;

                    const isInView = posX >= (viewportXStart * 16) && posX < (viewportXEnd * 16) &&
                        posY >= (viewportYStart * 16) && posY < (viewportYEnd * 16);

                    const fxId = `${roomItem.id}_${tileX}_${tileY}`;

                    if (isInView) {
                        if (!particles.activeEffects[fxId]) {
                            const options = {
                                count: fxData.count,
                                speed: fxData.speed,
                                angle: fxData.baseAngle,
                                spread: fxData.spread,
                                colors: fxData.color.map(color => `rgba(${color.join(',')}, ${fxData.Opacity})`),
                                life: fxData.frames,
                                size: fxData.size,
                                type: 'default',
                                repeat: fxData.repeat,
                                glow: fxData.Glow,
                                opacity: fxData.Opacity,
                                blur: fxData.Blur,
                                shape: fxData.Shape.toLowerCase()
                            };

                            particles.createParticles(posX, posY, options, fxId);
                            console.log(`Effect added: ${fxId}`);
                        }
                    } else {
                        if (particles.activeEffects[fxId]) {
                            delete particles.activeEffects[fxId];
                            console.log(`Effect removed: ${fxId}`);
                        }
                    }

                }
            });
        }
    },

    renderBubbles: function(sprite, colorHex) {
        if (!sprite.bubbleEffect) {
            sprite.bubbleEffect = {
                bubbles: [],
                duration: 2000, // Animation duration in milliseconds
                startTime: Date.now(),
            };
        }
    
        const ctx = game.ctx;
        const currentTime = Date.now();
        const elapsedTime = currentTime - sprite.bubbleEffect.startTime;
    
        if (elapsedTime > sprite.bubbleEffect.duration) {
            // Clear the bubble effect after the duration ends
            delete sprite.bubbleEffect;
            return;
        }
    
        // Add new bubbles periodically
        if (sprite.bubbleEffect.bubbles.length < 10) {
            sprite.bubbleEffect.bubbles.push({
                x: Math.random() * sprite.width - sprite.width / 2, // Random x-offset relative to sprite center
                y: Math.random() * -10, // Random starting height
                radius: Math.random() * 3 + 2, // Random size for bubbles
                opacity: 1, // Start fully visible
                riseSpeed: Math.random() * 0.5 + 0.2, // Speed at which the bubble rises
            });
        }
    
        // Render and update bubbles
        sprite.bubbleEffect.bubbles.forEach((bubble, index) => {
            const bubbleX = sprite.x + sprite.width / 2 + bubble.x;
            const bubbleY = sprite.y - bubble.y;
    
            // Set the bubble's color with opacity
            const colorWithOpacity = `${colorHex}${Math.floor(bubble.opacity * 255).toString(16).padStart(2, '0')}`;
            ctx.fillStyle = colorWithOpacity;
    
            // Draw the bubble
            ctx.beginPath();
            ctx.arc(bubbleX, bubbleY, bubble.radius, 0, Math.PI * 2);
            ctx.fill();
    
            // Update bubble properties
            bubble.y += bubble.riseSpeed * game.deltaTime / 16; // Move upwards
            bubble.opacity -= 0.01; // Fade out gradually
    
            // Remove bubbles that are fully transparent or out of range
            if (bubble.opacity <= 0 || bubbleY < sprite.y - 40) {
                sprite.bubbleEffect.bubbles.splice(index, 1);
            }
        });
    },    

    aimTool: function() {
        if (game.mainSprite && game.mainSprite.targetAim) {
            const handX = game.mainSprite.x + game.mainSprite.width / 2 + game.mainSprite.handOffsetX;
            const handY = game.mainSprite.y + game.mainSprite.height / 2 + game.mainSprite.handOffsetY;
    
            const deltaX = game.mainSprite.targetX - handX;
            const deltaY = game.mainSprite.targetY - handY;
            const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
    
            let adjustedTargetX = game.mainSprite.targetX;
            let adjustedTargetY = game.mainSprite.targetY;
            if (distance > game.mainSprite.maxRange) {
                const ratio = game.mainSprite.maxRange / distance;
                adjustedTargetX = handX + deltaX * ratio;
                adjustedTargetY = handY + deltaY * ratio;
            }
    
            // Obstruction check logic (same as before)
            const isObstructed = (x, y) => {
                if (game.roomData && game.roomData.items) {
                    for (const roomItem of game.roomData.items) {
                        const itemData = assets.use('objectData')[roomItem.id];
                        if (!itemData) continue;
    
                        const xCoordinates = roomItem.x || [];
                        const yCoordinates = roomItem.y || [];
    
                        for (let i = 0; i < xCoordinates.length; i++) {
                            const itemX = parseInt(xCoordinates[i], 10) * 16;
                            const itemY = parseInt(yCoordinates[i], 10) * 16;
                            const tileRect = {
                                x: itemX,
                                y: itemY,
                                width: 16,
                                height: 16
                            };
    
                            if (
                                x >= tileRect.x &&
                                x <= tileRect.x + tileRect.width &&
                                y >= tileRect.y &&
                                y <= tileRect.y + tileRect.height
                            ) {
                                const tileData = itemData[0]; 
                                if (tileData.w !== 1) {
                                    return { obstructed: true, collisionX: x, collisionY: y };
                                }
                            }
                        }
                    }
                }
                return { obstructed: false };
            };
    
            let finalTargetX = adjustedTargetX;
            let finalTargetY = adjustedTargetY;
            const steps = Math.ceil(distance);
            let obstructionDetected = false;
    
            for (let i = 1; i <= steps; i++) {
                const stepX = handX + (deltaX * i) / steps;
                const stepY = handY + (deltaY * i) / steps;
                const result = isObstructed(stepX, stepY);
                if (result.obstructed) {
                    finalTargetX = result.collisionX;
                    finalTargetY = result.collisionY;
                    obstructionDetected = true;
                    break;
                }
            }
    
            if (obstructionDetected && Math.sqrt((finalTargetX - handX) ** 2 + (finalTargetY - handY) ** 2) < 10) {
                return;
            }
    
            // Adjusted properties for slightly thicker and still small aim with subtle shadow
            const crosshairSize = 2; // Increased size for a more noticeable crosshair
            const sniperLineLength = 4; // Increased length for longer sniper lines
            const targetRadius = game.mainSprite.targetRadius * 0.75; // Optionally adjust the radius
    
            // Set subtle shadow properties
            game.ctx.save(); // Save the current state
            game.ctx.shadowColor = 'rgba(0, 0, 0, 0.5)'; // Maintain a transparent black shadow
            game.ctx.shadowBlur = 2; // Maintain minimal blur for subtlety
            game.ctx.shadowOffsetX = 0.3; // Maintain minimal horizontal offset
            game.ctx.shadowOffsetY = 0.3; // Maintain minimal vertical offset
    
            // Set the stroke style to red with slightly increased transparency
            game.ctx.strokeStyle = 'rgba(255, 0, 0, 0.4)'; // Red color with 40% opacity
            game.ctx.lineWidth = 1; // Increased line width for slightly thicker lines
            game.ctx.lineCap = 'butt'; // Square line ends for pixelated look
    
            // Use precise (rounded) coordinates to ensure perfect alignment
            const centerX = finalTargetX;
            const centerY = finalTargetY;
    
            // Draw the blocky aiming line
            game.ctx.beginPath();
            game.ctx.moveTo(handX, handY); // Use precise coordinates
            game.ctx.lineTo(centerX, centerY);
            game.ctx.stroke();
    
            // Determine the crosshair color based on gamepad input with slightly increased transparency
            const aimCrosshairColor = gamepad.buttons.includes('r2') ? 'rgba(200, 0, 0, 0.8)' : 'rgba(255, 0, 0, 0.4)'; // Red with 40% opacity
    
            // Draw the crosshair and aim circle with minimal shadow
            game.ctx.strokeStyle = aimCrosshairColor;
            game.ctx.lineWidth = 1; // Slightly thicker lines for crosshairs and sniper lines
    
            // Draw the aim circle centered at (centerX, centerY)
            game.ctx.beginPath();
            game.ctx.arc(centerX, centerY, targetRadius, 0, 2 * Math.PI);
            game.ctx.stroke();
    
            // Draw the crosshairs centered at (centerX, centerY)
            game.ctx.beginPath();
            game.ctx.moveTo(centerX - crosshairSize, centerY);
            game.ctx.lineTo(centerX + crosshairSize, centerY);
            game.ctx.moveTo(centerX, centerY - crosshairSize);
            game.ctx.lineTo(centerX, centerY + crosshairSize);
            game.ctx.stroke();
    
            // Draw the sniper lines centered at (centerX, centerY)
            game.ctx.beginPath();
            game.ctx.moveTo(centerX - sniperLineLength, centerY);
            game.ctx.lineTo(centerX - crosshairSize, centerY);
            game.ctx.moveTo(centerX + crosshairSize, centerY);
            game.ctx.lineTo(centerX + sniperLineLength, centerY);
            game.ctx.moveTo(centerX, centerY - sniperLineLength);
            game.ctx.lineTo(centerX, centerY - crosshairSize);
            game.ctx.moveTo(centerX, centerY + crosshairSize);
            game.ctx.lineTo(centerX, centerY + sniperLineLength);
            game.ctx.stroke();
    
            game.ctx.restore(); // Restore the state to remove shadow for other drawings
        }
    }
    
    
};
