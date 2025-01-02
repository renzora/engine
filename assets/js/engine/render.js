
var render = {
    spriteCount: 0,
    animationCount: 0,
    backgroundTileCount: 0,
    tileCount: 0,
    overlappingTiles: [],
    renderQueue: [],
    sceneBg: null,
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
                    }).flat();
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
    weather.snow.update(deltaTime);
    weather.rain.update(deltaTime);
    weather.fireflies.update(deltaTime);
    weather.clouds.update(deltaTime);
    particles.updateParticles(deltaTime);
    effects.transitions.update();

    if (weather.rainActive) {
        audio.playAudio("rain", assets.use('rain'), 'ambience', true);
    } else {
        audio.stopLoopingAudio('rain', 'ambience', 0.5);
    }

    actions.checkForNearbyItems();
},

renderBackground: function(viewportXStart, viewportXEnd, viewportYStart, viewportYEnd) {
    this.backgroundTileCount = 0;

    const tileSize = 16;

    // Fill the entire viewport with black
    game.ctx.fillStyle = 'black';
    game.ctx.fillRect(
        viewportXStart * tileSize, 
        viewportYStart * tileSize, 
        (viewportXEnd - viewportXStart + 1) * tileSize, 
        (viewportYEnd - viewportYStart + 1) * tileSize
    );

    if (!this.sceneBg) {
        return;
    }

    const bgTileData = game.objectData[this.sceneBg][0];

    for (let y = Math.floor(viewportYStart); y <= Math.floor(viewportYEnd); y++) {
        for (let x = Math.floor(viewportXStart); x <= Math.floor(viewportXEnd); x++) {
            const posX = x * tileSize;
            const posY = y * tileSize;

            const tileFrameIndex = bgTileData.i;
            const srcX = (tileFrameIndex % 150) * tileSize;
            const srcY = Math.floor(tileFrameIndex / 150) * tileSize;

            game.ctx.drawImage(assets.use(bgTileData.t), srcX, srcY, tileSize, tileSize, posX, posY, tileSize, tileSize);

            this.backgroundTileCount++;
            this.tileCount++;
        }
    }
    utils.tracker('render.renderBackground');
},

renderAll: function(viewportXStart, viewportXEnd, viewportYStart, viewportYEnd) {
    const renderQueue = [];
    this.backgroundTileCount = 0;
    this.tileCount = 0;
    this.spriteCount = 0;

    // Expand objectData for any ranged tile indices
    const expandedObjectData = Object.keys(game.objectData).reduce((acc, key) => {
        acc[key] = game.objectData[key].map(this.expandTileData.bind(this));
        return acc;
    }, {});

    // Draw Room Items (Tiles/Objects)
    if (game.roomData && game.roomData.items) {
        game.roomData.items.forEach(roomItem => {
            const itemData = expandedObjectData[roomItem.id];
            if (!itemData || itemData.length === 0) return;

            const tileData = itemData[0];
            const xCoordinates = roomItem.x || [];
            const yCoordinates = roomItem.y || [];

            // Compute the "bottom" (max Y) in tile-space
            const bottomYTile = Math.max(...yCoordinates);
            const bottomYPixel = bottomYTile * 16;

            // Handle rotation
            let rotation = tileData.rotation || 0;
            if (roomItem.isRotating) {
                actions.handleRotation(roomItem);
                rotation = roomItem.rotation;
            }
            if (tileData.sway === true) {
                rotation += this.handleSway(roomItem);
            }

            // Render each tile
            let index = 0;
            for (let i = 0; i < yCoordinates.length; i++) {
                const tileY = yCoordinates[i];
                for (let j = 0; j < xCoordinates.length; j++) {
                    const tileX = xCoordinates[j];

                    // Only render if within camera’s viewport
                    if (
                        (tileX * 16 + 16) >= (viewportXStart * 16) &&
                        (tileX * 16) < (viewportXEnd * 16) &&
                        (tileY * 16 + 16) >= (viewportYStart * 16) &&
                        (tileY * 16) < (viewportYEnd * 16)
                    ) {
                        // Position relative to center/bottom for rotation
                        const centerX = (Math.min(...xCoordinates) + Math.max(...xCoordinates)) / 2;
                        const maxY = Math.max(...yCoordinates);
                        const posX = (tileX - centerX) * 16;
                        const posY = (tileY - maxY) * 16;

                        // Animated frames or single frame
                        let tileFrameIndex;
                        if (Array.isArray(tileData.i[0])) {
                            // Multiple animation frames
                            const animationData = tileData.i;
                            const currentFrame = tileData.currentFrame || 0;
                            tileFrameIndex = animationData[currentFrame][ index % animationData[currentFrame].length ];
                        } else {
                            // Single set of frames
                            tileFrameIndex = tileData.i[index];
                        }

                        if (tileFrameIndex !== undefined) {
                            // Figure out the z layering
                            let z = tileData.z;
                            let isZUndefined = false;
                            if (Array.isArray(tileData.z)) {
                                // If 'z' is an array, pick correct index
                                z = tileData.z[index % tileData.z.length];
                            }
                            if (z === undefined) {
                                isZUndefined = true;
                                // We'll do the standard foot calculation
                            }

                            // The final objectZIndex logic
                            let objectZIndex;
                            if (z === 0 && !isZUndefined) {
                                // z=0 => behind everything else => zIndex=0
                                objectZIndex = 0;
                            } else if (z === 1) {
                                // z=1 => in front of z=0, but behind everything else => zIndex=1
                                objectZIndex = 1;
                            } else if (isZUndefined) {
                                // no z => standard foot calc => bottomYPixel
                                objectZIndex = bottomYPixel;
                            } else {
                                // If z is anything else (like 2, 5, etc.),
                                // we just do bottomYPixel + z
                                objectZIndex = bottomYPixel + z;
                            }

                            // Compute source coords in the sprite sheet
                            const srcX = (tileFrameIndex % 150) * 16;
                            const srcY = Math.floor(tileFrameIndex / 150) * 16;

                            // Push to the renderQueue
                            renderQueue.push({
                                zIndex: objectZIndex,
                                draw: () => {
                                    game.ctx.save();
                                    const centerXPixel = centerX * 16;
                                    const bottomYPixelVal = maxY * 16;
                                    game.ctx.translate(centerXPixel, bottomYPixelVal);
                                    game.ctx.rotate(rotation);

                                    game.ctx.drawImage(
                                        assets.use(tileData.t),
                                        srcX, srcY, 16, 16,
                                        posX, posY, 16, 16
                                    );

                                    game.ctx.restore();
                                }
                            });

                            this.tileCount++;
                        }
                    }
                    index++;
                }
            }

            // Lights & Effects
            this.handleLights(tileData, roomItem, viewportXStart, viewportXEnd, viewportYStart, viewportYEnd);
            this.handleEffects(tileData, roomItem, viewportXStart, viewportXEnd, viewportYStart, viewportYEnd);
        });
    }

    // 3) Draw Sprites
    for (let id in game.sprites) {
        const sprite = game.sprites[id];
        const spriteRight = sprite.x + sprite.width;
        const spriteBottom = sprite.y + sprite.height;

        // Only render if in the viewport
        if (
            spriteRight >= viewportXStart * 16 && sprite.x < viewportXEnd * 16 &&
            spriteBottom >= viewportYStart * 16 && sprite.y < viewportYEnd * 16
        ) {
            // Use sprite’s foot for layering
            const spriteZIndex = sprite.y + sprite.height;

            renderQueue.push({
                zIndex: spriteZIndex,
                draw: () => {
                    // any special draws, shadows, etc.
                    render.renderPathfinderLine();
                    sprite.drawShadow();
                    weather.clouds.draw();
                    effects.dirtCloudEffect.updateAndRender(game.deltaTime);
                    sprite.draw();
                    effects.bubbleEffect.updateAndRender(game.deltaTime);
                }
            });
            this.spriteCount++;
        }
    }

    // 4) Sort in ascending order => lower zIndex drawn *behind* higher zIndex
    renderQueue.sort((a, b) => a.zIndex - b.zIndex);

    // 5) Render in sorted order
    renderQueue.forEach(item => item.draw());

    // 6) If the editor’s grid is active, render it last
    if (utils.objExists("editor_context_menu_window.renderIsometricGrid")) {
        editor_context_menu_window.renderIsometricGrid();
    }

    utils.tracker("render.renderAll");
},


    handleSway: function(roomItem) {
        if (!roomItem.swayInitialized) {
            roomItem.swayAngle = Math.PI / (160 + Math.random() * 40);
            roomItem.swaySpeed = 5000 + Math.random() * 2000;
            roomItem.swayInitialized = true;
        }

        if (roomItem.isInViewport) {
            const elapsedTime = roomItem.swayElapsed || 0;
            roomItem.swayElapsed = elapsedTime + game.deltaTime;
            const sway = Math.sin((roomItem.swayElapsed / roomItem.swaySpeed) * Math.PI * 2) * roomItem.swayAngle;

            return sway;
        }

        utils.tracker('render.handleSway');

        return 0;
    },

    initializeSway: function(roomItem) {
        roomItem.swayAngle = Math.PI / (160 + Math.random() * 40);
        roomItem.swaySpeed = 5000 + Math.random() * 2000;
        roomItem.swayElapsed = 0;
        roomItem.swayInitialized = true;
    },
       
    renderPathfinderLine: function () {
        if (game.mainSprite && game.mainSprite.path && game.mainSprite.path.length > 0) {
            const ctx = game.ctx;
    
            // Draw the rippling rings at the destination
            const lastPoint = game.mainSprite.path[game.mainSprite.path.length - 1];
            const elapsed = Date.now() % 1000;
            const progress1 = (elapsed % 1000) / 1000; // Progress for the first ring
            const progress2 = ((elapsed + 500) % 1000) / 1000; // Delayed progress for the second ring
    
            const ring1Radius = 3 + progress1 * 10; // First ring expands outward
            const ring2Radius = 3 + progress2 * 12; // Second ring expands outward
    
            const ringOpacity1 = 0.4 - progress1 * 0.4; // First ring fades out
            const ringOpacity2 = 0.4 - progress2 * 0.4; // Second ring fades out
    
            // Draw the first pixelated ring
            const pixelSize = 2; // Size of each pixel block
            const pixelatedRing1Radius = Math.floor(ring1Radius / pixelSize) * pixelSize;
            for (let y = -pixelatedRing1Radius; y <= pixelatedRing1Radius; y += pixelSize) {
                for (let x = -pixelatedRing1Radius; x <= pixelatedRing1Radius; x += pixelSize) {
                    const distance = Math.sqrt(x * x + y * y);
                    if (distance >= pixelatedRing1Radius - pixelSize && distance <= pixelatedRing1Radius) {
                        ctx.fillStyle = `rgba(0, 102, 255, ${ringOpacity1})`;
                        ctx.fillRect(
                            lastPoint.x * 16 + 8 + x - pixelSize / 2,
                            lastPoint.y * 16 + 8 + y - pixelSize / 2,
                            pixelSize,
                            pixelSize
                        );
                    }
                }
            }
    
            // Draw the second pixelated ring
            const pixelatedRing2Radius = Math.floor(ring2Radius / pixelSize) * pixelSize;
            for (let y = -pixelatedRing2Radius; y <= pixelatedRing2Radius; y += pixelSize) {
                for (let x = -pixelatedRing2Radius; x <= pixelatedRing2Radius; x += pixelSize) {
                    const distance = Math.sqrt(x * x + y * y);
                    if (distance >= pixelatedRing2Radius - pixelSize && distance <= pixelatedRing2Radius) {
                        ctx.fillStyle = `rgba(0, 102, 255, ${ringOpacity2})`;
                        ctx.fillRect(
                            lastPoint.x * 16 + 8 + x - pixelSize / 2,
                            lastPoint.y * 16 + 8 + y - pixelSize / 2,
                            pixelSize,
                            pixelSize
                        );
                    }
                }
            }
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
        if (tileData.l && Array.isArray(tileData.l)) {
            tileData.l.forEach((light, lightIndex) => {
                if (light.x !== undefined && light.y !== undefined) {

                    const objectTopLeftX = Math.min(...roomItem.x) * 16;
                    const objectTopLeftY = Math.min(...roomItem.y) * 16;
                    const posX = objectTopLeftX + light.x;
                    const posY = objectTopLeftY + light.y;
                    const radius = tileData.lr || 200;
    
                    const isInView = (posX + radius) >= (viewportXStart * 16) && (posX - radius) < (viewportXEnd * 16) &&
                        (posY + radius) >= (viewportYStart * 16) && (posY - radius) < (viewportYEnd * 16);
    
                    const lightId = `${roomItem.id}_${Math.round(posX)}_${Math.round(posY)}`;
                    const hours = utils.gameTime.hours;
                    const minutes = utils.gameTime.minutes;
                    const time = hours + minutes / 60;
                    const isNightTime = time >= 22 || time < 7;
    
                    if (isInView && isNightTime) {
                        const existingLight = lighting.lights.find(light => light.id === lightId);
    
                        if (!existingLight) {
                            const color = tileData.lc || { r: 255, g: 255, b: 255 };
                            const intensity = tileData.li || 1;
                            const flickerSpeed = tileData.lfs || 0.03;
                            const flickerAmount = tileData.lfa || 0.04;
                            const lampType = tileData.lt || "lamp";
                            const shape = light.shape || null;
    
                            lighting.addLight(lightId, posX, posY, radius, color, intensity, lampType, true, flickerSpeed, flickerAmount, shape);
                        }
                    } else {
                        lighting.lights = lighting.lights.filter(light => light.id !== lightId);
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
                duration: 2000,
                startTime: Date.now(),
            };
        }
    
        const ctx = game.ctx;
        const currentTime = Date.now();
        const elapsedTime = currentTime - sprite.bubbleEffect.startTime;
    
        if (elapsedTime > sprite.bubbleEffect.duration) {
            delete sprite.bubbleEffect;
            return;
        }
    
        if (sprite.bubbleEffect.bubbles.length < 10) {
            sprite.bubbleEffect.bubbles.push({
                x: Math.random() * sprite.width - sprite.width / 2,
                y: Math.random() * -10,
                radius: Math.random() * 3 + 2,
                opacity: 1,
                riseSpeed: Math.random() * 0.5 + 0.2,
            });
        }
    
        sprite.bubbleEffect.bubbles.forEach((bubble, index) => {
            const bubbleX = sprite.x + sprite.width / 2 + bubble.x;
            const bubbleY = sprite.y - bubble.y;
            const colorWithOpacity = `${colorHex}${Math.floor(bubble.opacity * 255).toString(16).padStart(2, '0')}`;
            ctx.fillStyle = colorWithOpacity;
            ctx.beginPath();
            ctx.arc(bubbleX, bubbleY, bubble.radius, 0, Math.PI * 2);
            ctx.fill();

            bubble.y += bubble.riseSpeed * game.deltaTime / 16;
            bubble.opacity -= 0.01;
    
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
    
            const crosshairSize = 2;
            const sniperLineLength = 4;
            const targetRadius = game.mainSprite.targetRadius * 0.75;
    
            game.ctx.save();
            game.ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
            game.ctx.shadowBlur = 2;
            game.ctx.shadowOffsetX = 0.3;
            game.ctx.shadowOffsetY = 0.3;
            game.ctx.strokeStyle = 'rgba(255, 0, 0, 0.4)';
            game.ctx.lineWidth = 1;
            game.ctx.lineCap = 'butt';
    
            const centerX = finalTargetX;
            const centerY = finalTargetY;
    
            game.ctx.beginPath();
            game.ctx.moveTo(handX, handY);
            game.ctx.lineTo(centerX, centerY);
            game.ctx.stroke();
    
            const aimCrosshairColor = gamepad.buttons.includes('r2') ? 'rgba(200, 0, 0, 0.8)' : 'rgba(255, 0, 0, 0.4)';
    
            game.ctx.strokeStyle = aimCrosshairColor;
            game.ctx.lineWidth = 1;
            game.ctx.beginPath();
            game.ctx.arc(centerX, centerY, targetRadius, 0, 2 * Math.PI);
            game.ctx.stroke();
            game.ctx.beginPath();
            game.ctx.moveTo(centerX - crosshairSize, centerY);
            game.ctx.lineTo(centerX + crosshairSize, centerY);
            game.ctx.moveTo(centerX, centerY - crosshairSize);
            game.ctx.lineTo(centerX, centerY + crosshairSize);
            game.ctx.stroke();
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
            game.ctx.restore();
        }
    }
};
