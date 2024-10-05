var render = {
    overlappingTiles: [],
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
                    // Handle case where each frame is an array (animation frames)
                    return frame.map(value => {
                        if (typeof value === 'string' && value.includes('-')) {
                            return this.parseRange(value);
                        }
                        return value;
                    }).flat();  // Flatten after expanding ranges
                } else if (typeof frame === 'string' && frame.includes('-')) {
                    // Handle single range
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
                sprite.checkTileActions();
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
    renderBackground: function (viewportXStart, viewportXEnd, viewportYStart, viewportYEnd) {
        let tileCount = 0;
        const bgTileData = game.objectData[game.sceneBg][0];
        for (let y = viewportYStart; y < viewportYEnd; y++) {
            for (let x = viewportXStart; x < viewportXEnd; x++) {
                const posX = x * 16;
                const posY = y * 16;
                const tileFrameIndex = bgTileData.i;
                const srcX = (tileFrameIndex % 150) * 16;
                const srcY = Math.floor(tileFrameIndex / 150) * 16;

                game.ctx.drawImage(assets.load(bgTileData.t), srcX, srcY, 16, 16, posX, posY, 16, 16);
                tileCount++;
            }
        }
        return tileCount;
    },

    renderAll: function(viewportXStart, viewportXEnd, viewportYStart, viewportYEnd) {
        const renderQueue = [];
        let backgroundTileCount = 0;
        let tileCount = 0;
        let spriteCount = 0;
    
        // Apply expandTileData to objectData
        const expandedObjectData = Object.keys(game.objectData).reduce((acc, key) => {
            acc[key] = game.objectData[key].map(this.expandTileData.bind(this));
            return acc;
        }, {});
    
        backgroundTileCount = this.renderBackground(viewportXStart, viewportXEnd, viewportYStart, viewportYEnd);
    
        if (game.roomData && game.roomData.items) {
            game.roomData.items.forEach(roomItem => {
                const itemData = expandedObjectData[roomItem.id];
                if (itemData && itemData.length > 0) {
                    const tileData = itemData[0];
                    const xCoordinates = roomItem.x || [];
                    const yCoordinates = roomItem.y || [];
    
                    let index = 0;
                    for (let y = Math.min(...yCoordinates); y <= Math.max(...yCoordinates); y++) {
                        for (let x = Math.min(...xCoordinates); x <= Math.max(...xCoordinates); x++) {
                            if (x >= viewportXStart && x < viewportXEnd && y >= viewportYStart && y < viewportYEnd) {
                                const posX = x * 16;
                                const posY = y * 16;
    
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
    
                                    renderQueue.push({
                                        zIndex: Array.isArray(tileData.zIndex) ? tileData.zIndex[index % tileData.zIndex.length] : tileData.zIndex,
                                        draw: function () {
                                            game.ctx.drawImage(assets.load(tileData.t), srcX, srcY, 16, 16, posX, posY, 16, 16);
                                        }
                                    });
    
                                    tileCount++;
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
                        game.sprites[id].draw();
                    }
                });
    
                spriteCount++; // Increment sprite count
            }
        }
    
        // Sort the combined renderQueue by zIndex before rendering
        renderQueue.sort((a, b) => a.zIndex - b.zIndex);
    
        // Draw the items in the renderQueue
        renderQueue.forEach(item => {
            item.draw();
        });
    
        // Return counts, including background tiles
        return { backgroundTileCount, tileCount, spriteCount };
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

    renderLightingEffects: function () {
        lighting.drawNightFilter();
        game.ctx.globalCompositeOperation = lighting.compositeOperation;
        game.ctx.drawImage(lighting.createLightMask(), 0, 0);
        game.ctx.globalCompositeOperation = 'source-over';
        game.ctx.imageSmoothingEnabled = false;
    },

    renderWeatherEffects: function () {
        weather.drawSnow();
        weather.drawRain();
        weather.drawFireflys();
        lighting.drawGreyFilter();
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

    updateUI: function (tileCount, spriteCount) {
        var tilesRenderedDisplay = document.getElementById('tiles_rendered');
        if (tilesRenderedDisplay) {
            tilesRenderedDisplay.innerHTML = `Tiles: ${tileCount} | Sprites: ${spriteCount}`;
        }

        var lightsRenderedDisplay = document.getElementById('lights_rendered');
        if (lightsRenderedDisplay) {
            lightsRenderedDisplay.innerHTML = `Lights: ${lighting.lights.length}`;
        }

        var effectsRenderedDisplay = document.getElementById('effects_rendered');
        if (effectsRenderedDisplay) {
            effectsRenderedDisplay.innerHTML = `Effects: ${Object.keys(particles.activeEffects).length}`;
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
    
            const crosshairSize = 10;
            const sniperLineLength = 15;
    
            // Draw the line in white
            game.ctx.strokeStyle = 'rgba(255, 255, 255, 0.8)';
            game.ctx.lineWidth = 1;
            game.ctx.setLineDash([5, 5]);
            game.ctx.beginPath();
            game.ctx.moveTo(handX, handY);
            game.ctx.lineTo(finalTargetX, finalTargetY);
            game.ctx.stroke();
            game.ctx.setLineDash([]);
    
            // Determine the color for the aim circle and crosshairs based on R2
            const aimCrosshairColor = gamepad.buttons.includes('r2') ? 'rgba(255, 0, 0, 0.8)' : 'rgba(255, 255, 255, 0.8)';
    
            // Draw the aim circle
            game.ctx.strokeStyle = aimCrosshairColor;
            game.ctx.beginPath();
            game.ctx.arc(finalTargetX, finalTargetY, game.mainSprite.targetRadius, 0, 2 * Math.PI);
            game.ctx.stroke();
    
            // Draw the crosshairs
            game.ctx.beginPath();
            game.ctx.moveTo(finalTargetX - crosshairSize, finalTargetY);
            game.ctx.lineTo(finalTargetX + crosshairSize, finalTargetY);
            game.ctx.moveTo(finalTargetX, finalTargetY - crosshairSize);
            game.ctx.lineTo(finalTargetX, finalTargetY + crosshairSize);
            game.ctx.stroke();
    
            // Draw the sniper lines in the same color as the crosshairs
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
}