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

        for (let id in game.sprites) {
            const sprite = game.sprites[id];
            if (sprite.update) {
                sprite.update(deltaTime);
            }
        }

        camera.update();
        game.gameTime.update(deltaTime);
        lighting.updateDayNightCycle();
        animate.updateAnimatedTiles(deltaTime);
        weather.updateSnow(deltaTime);
        weather.updateRain(deltaTime);
        weather.updateFireflys(deltaTime);
        particles.updateParticles(deltaTime);
        effects.transitions.update();
        lighting.updateLights(deltaTime);

        if (weather.rainActive) {
            audio.playAudio("rain", assets.load('rain'), 'ambience', true);
        } else {
            audio.stopLoopingAudio('rain', 'ambience', 0.5);
        }

        actions.checkForNearbyItems();

        if (typeof ui_window !== 'undefined' && ui_window.checkAndUpdateUIPositions) {
            ui_window.checkAndUpdateUIPositions();
        }
    },

    // Modify renderBackground to use cached background for the entire world
    renderBackground: function(viewportXStart, viewportXEnd, viewportYStart, viewportYEnd) {
        this.backgroundTileCount = 0;  // Reset background tile count
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
                game.ctx.drawImage(assets.load(bgTileData.t), srcX, srcY, tileSize, tileSize, posX, posY, tileSize, tileSize);

                // Increment the visible background tile count
                this.backgroundTileCount++;
                this.tileCount++;  // Increment total tile count
            }
        }
    },

    renderAll: function(viewportXStart, viewportXEnd, viewportYStart, viewportYEnd) {
        const renderQueue = [];
        this.backgroundTileCount = 0;
        this.tileCount = 0;
        this.spriteCount = 0;
    
        // WebGPU setup: Create a command encoder to record rendering commands
        const commandEncoder = game.gpuDevice.createCommandEncoder();
    
        // Create a texture view from the current context
        const textureView = game.gpuContext.getCurrentTexture().createView();
    
        // Configure the rendering pass
        const renderPassDescriptor = {
            colorAttachments: [{
                view: textureView,
                loadOp: 'clear', // Clear the canvas before rendering
                storeOp: 'store', // Store the result in the texture
                clearValue: { r: 0.3, g: 0.3, b: 0.3, a: 1.0 } // Clear to a gray background
            }]
        };
    
        // Begin the render pass
        const passEncoder = commandEncoder.beginRenderPass(renderPassDescriptor);
        passEncoder.setPipeline(game.gpuPipeline); // Use the pipeline set up for rendering
    
        // Apply expandTileData to objectData
        const expandedObjectData = Object.keys(game.objectData).reduce((acc, key) => {
            acc[key] = game.objectData[key].map(this.expandTileData.bind(this));
            return acc;
        }, {});
    
        // 1. Render the background first
        this.renderBackground(viewportXStart, viewportXEnd, viewportYStart, viewportYEnd);
    
        // 2. Render the grid if enabled
        if (utils.objExists('editor_utils_window.renderGrid')) {
            editor_utils_window.renderGrid();
        }
    
        // 3. Render the game objects using WebGPU
        if (game.roomData && game.roomData.items) {
            game.roomData.items.forEach(roomItem => {
                const itemData = expandedObjectData[roomItem.id];
                if (itemData && itemData.length > 0) {
                    const tileData = itemData[0];
                    const xCoordinates = roomItem.x || [];
                    const yCoordinates = roomItem.y || [];
    
                    const centerX = (Math.min(...xCoordinates) + Math.max(...xCoordinates)) / 2;
                    const bottomY = Math.max(...yCoordinates);
    
                    let rotation = tileData.rotation || 0;
                    if (roomItem.isRotating) {
                        actions.handleRotation(roomItem);
                        rotation = roomItem.rotation;
                    }
    
                    if (tileData.sway === true) {
                        rotation += this.handleSway(roomItem);
                    }
    
                    let index = 0;
                    for (let i = 0; i < yCoordinates.length; i++) {
                        const y = yCoordinates[i];
                        for (let j = 0; j < xCoordinates.length; j++) {
                            const x = xCoordinates[j];
    
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
                                    this.renderObject(passEncoder, posX, posY, rotation, tileData);
                                }
    
                                this.tileCount++;
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
    
                this.renderObject(passEncoder, sprite.x, sprite.y, 0, sprite); // Render sprite via WebGPU
                this.spriteCount++;
            }
        }
    
        // End the render pass and submit the commands to the GPU
        passEncoder.end();
        game.gpuDevice.queue.submit([commandEncoder.finish()]);
    },
    
    renderObject: function(passEncoder, x, y, rotation, tileData) {
        const tileSize = 16;
        const vertices = new Float32Array([
            x, y, 0.0, // Top-left corner
            x + tileSize, y, 0.0, // Top-right corner
            x + tileSize, y + tileSize, 0.0, // Bottom-right corner
            x, y + tileSize, 0.0  // Bottom-left corner
        ]);
    
        // Create the vertex buffer
        const vertexBuffer = game.gpuDevice.createBuffer({
            size: vertices.byteLength,
            usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
            mappedAtCreation: true,
        });
    
        new Float32Array(vertexBuffer.getMappedRange()).set(vertices);
        vertexBuffer.unmap();
    
        // Set the vertex buffer and rotation
        passEncoder.setVertexBuffer(0, vertexBuffer);
        passEncoder.setBindGroup(0, game.gpuBindGroup); // Bind group for the texture and uniforms
        passEncoder.draw(4, 1, 0, 0); // Draw 4 vertices (rectangle)
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

    drawBorderAroundObject: function(roomItem) {
        if (!roomItem) return;

        // Calculate the object's bounding box using the min/max of its coordinates
        const xCoordinates = roomItem.x || [];
        const yCoordinates = roomItem.y || [];
    
        const minX = Math.min(...xCoordinates) * 16;  // Convert to pixel positions
        const minY = Math.min(...yCoordinates) * 16;
        const maxX = Math.max(...xCoordinates) * 16 + 16;  // +16 for tile size
        const maxY = Math.max(...yCoordinates) * 16 + 16;  // +16 for tile size
    
        // Draw the border around the object using its calculated position and size
        game.ctx.save();
        game.ctx.strokeStyle = 'red';  // Border color
        game.ctx.lineWidth = 2;
        game.ctx.strokeRect(minX - camera.cameraX, minY - camera.cameraY, maxX - minX, maxY - minY);
        game.ctx.restore();
    },

    renderCarriedObjects: function () {
        if (game.mainSprite && game.mainSprite.isCarrying) {
            const carriedItemId = game.mainSprite.carriedItem;
            const itemX = game.mainSprite.x - 8;
            const itemY = game.mainSprite.y - 32 - (game.objectData[carriedItemId][0].b.length);

            game.drawCarriedObject(game.ctx, carriedItemId, itemX, itemY);
        }
    },

    renderLightingEffects: function () {
        lighting.drawNightFilter();
        game.ctx.globalCompositeOperation = lighting.compositeOperation;
        game.ctx.drawImage(lighting.createLightMask(), 0, 0);
        game.ctx.globalCompositeOperation = 'source-over';
        game.ctx.imageSmoothingEnabled = false;
    },

    renderWeatherEffects: function () {
        //weather.drawSnow();
        //weather.drawRain();
        //weather.drawFireflys();
        //lighting.drawGreyFilter();
        render.aimTool();
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

    updateUI: function () {
        var tilesRenderedDisplay = document.getElementById('tiles_rendered');

        if (tilesRenderedDisplay) {
            tilesRenderedDisplay.innerHTML = `Tiles: ${this.tileCount}`;
        }

        var background_rendered = document.getElementById('background_rendered');

        if (background_rendered) {
            background_rendered.innerHTML = `Background: ${this.backgroundTileCount}`;
        }
    
        var lightsRenderedDisplay = document.getElementById('lights_rendered');
        if (lightsRenderedDisplay) {
            lightsRenderedDisplay.innerHTML = `Lights: ${lighting.lights.length}`;
        }
    
        var effectsRenderedDisplay = document.getElementById('effects_rendered');
        if (effectsRenderedDisplay) {
            effectsRenderedDisplay.innerHTML = `Effects: ${Object.keys(particles.activeEffects).length}`;
        }
    
        var animationsRenderedDisplay = document.getElementById('animations_rendered');
        if (animationsRenderedDisplay) {
            animationsRenderedDisplay.innerHTML = `Animations: ${this.animationCount}`; // Update animation count
        }
    }, 

    highlightOverlappingTiles: function () {
        this.overlappingTiles.forEach(tile => {
            game.ctx.fillStyle = 'rgba(255, 0, 0, 0.5)';
            game.ctx.fillRect(tile.x * 16, tile.y * 16, 16, 16);
        });
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

                        const hours = game.gameTime.hours;
                        const minutes = game.gameTime.minutes;
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
                        const itemData = assets.load('objectData')[roomItem.id];
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
    
            const crosshairSize = 8; // Slightly smaller for pixel look
            const sniperLineLength = 12; // Shorter lines for a blocky look
    
            // Pixelated line style
            game.ctx.strokeStyle = 'rgba(200, 100, 0, 1)'; // Muted orange, no transparency
            game.ctx.lineWidth = 2; // Thicker lines for pixelated feel
            game.ctx.lineCap = 'butt'; // Square line ends for pixel look
    
            // Draw the blocky line
            game.ctx.beginPath();
            game.ctx.moveTo(Math.floor(handX), Math.floor(handY)); // Floor to integers for pixel style
            game.ctx.lineTo(Math.floor(finalTargetX), Math.floor(finalTargetY));
            game.ctx.stroke();
    
            // Crosshair and aim circle
            const aimCrosshairColor = gamepad.buttons.includes('r2') ? 'rgba(200, 0, 0, 1)' : 'rgba(255, 255, 255, 1)';
    
            // Blocky aim circle
            game.ctx.strokeStyle = aimCrosshairColor;
            game.ctx.lineWidth = 1.5;
            game.ctx.beginPath();
            game.ctx.arc(Math.floor(finalTargetX), Math.floor(finalTargetY), game.mainSprite.targetRadius, 0, 2 * Math.PI);
            game.ctx.stroke();
    
            // Pixelated crosshairs
            game.ctx.strokeStyle = aimCrosshairColor;
            game.ctx.lineWidth = 1.5;
            game.ctx.beginPath();
            game.ctx.moveTo(finalTargetX - crosshairSize, finalTargetY);
            game.ctx.lineTo(finalTargetX + crosshairSize, finalTargetY);
            game.ctx.moveTo(finalTargetX, finalTargetY - crosshairSize);
            game.ctx.lineTo(finalTargetX, finalTargetY + crosshairSize);
            game.ctx.stroke();
    
            // Pixelated sniper lines
            game.ctx.beginPath();
            game.ctx.moveTo(finalTargetX - sniperLineLength, finalTargetY);
            game.ctx.lineTo(finalTargetX - crosshairSize, finalTargetY);
            game.ctx.moveTo(finalTargetX + crosshairSize, finalTargetY);
            game.ctx.lineTo(finalTargetX + sniperLineLength, finalTargetY);
            game.ctx.moveTo(finalTargetX, finalTargetY - sniperLineLength);
            game.ctx.lineTo(finalTargetX, finalTargetY - crosshairSize);
            game.ctx.moveTo(finalTargetX, finalTargetY + crosshairSize);
            game.ctx.lineTo(finalTargetX, finalTargetY + sniperLineLength);
            game.ctx.stroke();
        }
    }
    
    
};