const npcArea = {
    x: 280,
    y: 355,
    width: 80,
    height: 190
};

var game = {
    isBlackAndWhite: localStorage.getItem('blackAndWhite') === 'true',
    lerpFactor: parseFloat(localStorage.getItem('lerpFactor')) || 0.1,
    needsFilterUpdate: true,
    canvas: undefined,
    ctx: undefined,
    isEditMode: false,
    timestamp: 0,
    lastTime: 0,
    deltaTime: 0,
    worldWidth: 640,
    worldHeight: 640,
    zoomLevel: 4,
    cameraX: 0,
    cameraY: 0,
    targetCameraX: 0,
    targetCameraY: 0,
    targetX: 0,
    targetY: 0,
    roomData: undefined,
    sprites: {},

    init: function () {
        assets.preload([
            { name: 'character', path: 'img/sprites/character.png' },
            { name: 'hair', path: 'img/sprites/hair.png' },
            { name: 'hats', path: 'img/sprites/hats.png' },
            { name: 'glasses', path: 'img/sprites/glasses.png' },
            { name: 'facial', path: 'img/sprites/facial.png' },
            { name: 'outfit', path: 'img/sprites/outfit.png' },
            { name: '1', path: 'img/tiles/1.png' },
            { name: 'objectData', path: 'json/objectData.json' },
            { name: 'roomData', path: 'json/roomData.json' },
        ], () => {
            console.log("All assets loaded");
            this.canvas = document.createElement('canvas');
            this.ctx = this.canvas.getContext('2d');
            document.body.appendChild(this.canvas);
            this.resizeCanvas();
            this.roomData = assets.load('roomData');
            this.createMainSprite();
            this.createNPCSprite();
            weather.createStars();
            weather.initRain(0.2); // Initialize rain with 20% opacity
            weather.createSnow(0.2); // Initialize snow with 20% opacity
            this.applyBlackAndWhiteMode(); // Apply the filter initially
            this.setBlackAndWhiteMode(this.isBlackAndWhite);
            this.utils = new this.Utils();
            this.loop();
        });
    },

    Utils: function() {
        // Find an item in roomData by its ID and coordinates
        this.findItem = function(itemId, x, y) {
            if (!game.roomData || !game.roomData.items) return null;

            return game.roomData.items.find(item => {
                return item.id === itemId && item.x.includes(x) && item.y.includes(y);
            });
        };

        this.moveItem = function(itemId, oldX, oldY, newX, newY) {
            const item = this.findItem(itemId, oldX, oldY);
            if (item) {
                const xIndex = item.x.indexOf(oldX);
                const yIndex = item.y.indexOf(oldY);
                if (xIndex !== -1 && yIndex !== -1) {
                    item.x[xIndex] = newX;
                    item.y[yIndex] = newY;
                }
            }
        };

        this.animateItem = function(itemId, oldX, oldY, toX, toY, duration) {
            const item = this.findItem(itemId, oldX, oldY);
            if (!item) return;

            const startX = item.x.slice(); // copy array
            const startY = item.y.slice(); // copy array
            const deltaX = toX - startX[0];
            const deltaY = toY - startY[0];
            const startTime = performance.now();

            function animate() {
                const currentTime = performance.now();
                const elapsedTime = currentTime - startTime;
                const progress = Math.min(elapsedTime / duration, 1);

                for (let i = 0; i < item.x.length; i++) {
                    item.x[i] = startX[i] + deltaX * progress;
                    item.y[i] = startY[i] + deltaY * progress;
                }

                if (progress < 1) {
                    requestAnimationFrame(animate);
                }
            }

            requestAnimationFrame(animate);
        };

        this.addItem = function(itemId, itemData) {
            if (game.roomData && game.roomData.items) {
                const newItem = {
                    id: itemId,
                    x: itemData.x,
                    y: itemData.y,
                    // Add any other relevant properties from itemData
                };
                game.roomData.items.push(newItem);
            }
        };

        this.removeItem = function(itemId, x, y) {
            if (game.roomData && game.roomData.items) {
                const itemIndex = game.roomData.items.findIndex(item => {
                    return item.id === itemId && item.x.includes(x) && item.y.includes(y);
                });
                if (itemIndex !== -1) {
                    game.roomData.items.splice(itemIndex, 1);
                }
            }
        };

        this.shakeItem = function(itemId, x, y, intensity = 1, duration = 100) {
            const item = this.findItem(itemId, x, y);
            if (!item) return;

            const originalX = item.x.slice();
            const originalY = item.y.slice();
            const startTime = performance.now();

            function shake() {
                const currentTime = performance.now();
                const elapsedTime = currentTime - startTime;
                const progress = Math.min(elapsedTime / duration, 1);
                const angle = Math.random() * Math.PI * 2;

                for (let i = 0; i < item.x.length; i++) {
                    item.x[i] = originalX[i] + Math.cos(angle) * intensity * (1 - progress);
                    item.y[i] = originalY[i] + Math.sin(angle) * intensity * (1 - progress);
                }

                if (progress < 1) {
                    requestAnimationFrame(shake);
                } else {
                    for (let i = 0; i < item.x.length; i++) {
                        item.x[i] = originalX[i];
                        item.y[i] = originalY[i];
                    }
                }
            }

            requestAnimationFrame(shake);
        };

        this.scatterItem = function(itemId, x, y, scatterDistance = 3) {
            const item = this.findItem(itemId, x, y);
            if (!item) return;

            const originalX = item.x.slice();
            const originalY = item.y.slice();

            for (let i = 0; i < item.x.length; i++) {
                const angle = Math.random() * Math.PI * 2;
                const distance = Math.random() * scatterDistance;
                const newX = originalX[i] + Math.cos(angle) * distance;
                const newY = originalY[i] + Math.sin(angle) * distance;

                const startX = originalX[i];
                const startY = originalY[i];
                const deltaX = newX - startX;
                const deltaY = newY - startY;
                const startTime = performance.now();

                function animate() {
                    const currentTime = performance.now();
                    const elapsedTime = currentTime - startTime;
                    const progress = Math.min(elapsedTime / 500, 1);

                    item.x[i] = startX + deltaX * progress;
                    item.y[i] = startY + deltaY * progress;

                    if (progress < 1) {
                        requestAnimationFrame(animate);
                    }
                }

                requestAnimationFrame(animate);
            }
        };
    },

    setBlackAndWhiteMode: function(value) {
        this.isBlackAndWhite = value;
        this.needsFilterUpdate = true;
        localStorage.setItem('blackAndWhite', value); // Ensure the setting is saved when changed
        this.applyBlackAndWhiteMode(); // Apply the filter immediately when the setting changes
    },

    applyBlackAndWhiteMode: function() {
        if (this.needsFilterUpdate) {
            if (this.isBlackAndWhite) {
                document.body.classList.add('grayscale');
                this.canvas.classList.add('grayscale');
            } else {
                document.body.classList.remove('grayscale');
                this.canvas.classList.remove('grayscale');
            }
            this.needsFilterUpdate = false;
        }
    },

    createMainSprite: function () {
        this.mainSprite = sprite.create({
            x: 80,
            y: 0,
            hairstyle: 5,
            outfit: 3,
            facialHair: 1,
            hat: 0,
            glasses: 0
        });
        this.sprites['main'] = this.mainSprite;
    },

    createNPCSprite: function () {
        const randomOptions = {
            id: 'npc1',
            x: 300,
            y: 400,
            hairstyle: Math.floor(Math.random() * 6),
            outfit: Math.floor(Math.random() * 6),
            facialHair: Math.floor(Math.random() * 3),
            hat: Math.floor(Math.random() * 3),
            glasses: Math.floor(Math.random() * 3)
        };
        const npcSprite = sprite.createSprite(randomOptions);
        sprite.npcMovement(npcSprite, npcArea);
    },

    resizeCanvas: function() {
        this.canvas.width = window.innerWidth;
        this.canvas.height = window.innerHeight;
    },

    updateCamera: function() {
        if(!game.isEditMode) {

        var scaledWindowWidth = window.innerWidth / game.zoomLevel;
        var scaledWindowHeight = window.innerHeight / game.zoomLevel;

        if (game.worldWidth < scaledWindowWidth || game.worldHeight < scaledWindowHeight) {
            var xOffset = game.worldWidth < scaledWindowWidth ? (scaledWindowWidth - game.worldWidth) / 2 : 0;
            var yOffset = game.worldHeight < scaledWindowHeight ? (scaledWindowHeight - game.worldHeight) / 2 : 0;

            this.targetCameraX = -xOffset;
            this.targetCameraY = -yOffset;
        } else {
            let mainSprite = game.sprites['main'];
            if (mainSprite) {
                this.targetCameraX = mainSprite.x + mainSprite.width / 2 - scaledWindowWidth / 2;
                this.targetCameraY = mainSprite.y + mainSprite.height / 2 - scaledWindowHeight / 2;

                this.targetCameraX = Math.max(0, Math.min(this.targetCameraX, game.worldWidth - scaledWindowWidth));
                this.targetCameraY = Math.max(0, Math.min(this.targetCameraY, game.worldHeight - scaledWindowHeight));
            } else {
                console.error('Main sprite not found.');
            }
        }

        this.cameraX = this.lerp(this.cameraX, this.targetCameraX, this.lerpFactor);
        this.cameraY = this.lerp(this.cameraY, this.targetCameraY, this.lerpFactor);

        if (typeof debug_window !== 'undefined' && debug_window.camera) {
            debug_window.camera();
        }
        }
    },

    lerp: function(start, end, t) {
        return start * (1 - t) + end * t;
    },

    handleAimAttack: function () {
        for (let id in this.sprites) {
            if (this.sprites[id].isEnemy) {
                this.sprites[id].handleAimAttack();
            }
        }
    },

    render: function() {
        this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
        this.ctx.setTransform(1, 0, 0, 1, 0, 0);
        this.ctx.scale(this.zoomLevel, this.zoomLevel);
        this.ctx.translate(-Math.round(this.cameraX), -Math.round(this.cameraY));
    
        let renderQueue = [];
    
        if (this.roomData && this.roomData.items) {
            this.roomData.items.forEach(roomItem => {
                const itemData = assets.load('objectData')[roomItem.id];
                if (itemData && itemData.length > 0) {
                    const tileData = itemData[0];
                    const xCoordinates = roomItem.x || [];
                    const yCoordinates = roomItem.y || [];
    
                    let index = 0;
    
                    for (let y = Math.min(...yCoordinates); y <= Math.max(...yCoordinates); y++) {
                        for (let x = Math.min(...xCoordinates); x <= Math.max(...xCoordinates); x++) {
                            const posX = x * 16;
                            const posY = y * 16;
    
                            let tileFrameIndex;
                            if (tileData.d) {
                                const currentFrame = tileData.currentFrame || 0;
                                tileFrameIndex = Array.isArray(tileData.i) ? tileData.i[(currentFrame + index) % tileData.i.length] : tileData.i;
                            } else {
                                tileFrameIndex = tileData.i[index];
                            }
    
                            const srcX = (tileFrameIndex % 150) * 16;
                            const srcY = Math.floor(tileFrameIndex / 150) * 16;
    
                            renderQueue.push({
                                tileIndex: tileFrameIndex,
                                posX: posX,
                                posY: posY,
                                z: Array.isArray(tileData.z) ? tileData.z[index % tileData.z.length] : tileData.z,
                                draw: function() {
                                    game.ctx.drawImage(assets.load(tileData.t), srcX, srcY, 16, 16, this.posX, this.posY, 16, 16);
                                }
                            });
    
                            index++;
                        }
                    }
                }
            });
        }
    
        for (let id in this.sprites) {
            renderQueue.push({
                z: 1,
                draw: function() {
                    game.sprites[id].draw();
                }
            });
        }
    
        renderQueue.sort((a, b) => a.z - b.z);
        renderQueue.forEach(item => item.draw());
        this.ctx.imageSmoothingEnabled = false;
    
        // Draw weather effects
        weather.updateStars();
        weather.updateRain();
        weather.updateSnow();
        this.handleAimAttack();
    
        // Draw target aimer if active
        const mainSprite = this.sprites['main'];
        if (mainSprite && mainSprite.targetAim) {
            const handX = mainSprite.x + mainSprite.width / 2 + mainSprite.handOffsetX;
            const handY = mainSprite.y + mainSprite.height / 2 + mainSprite.handOffsetY;
    
            const deltaX = mainSprite.targetX - handX;
            const deltaY = mainSprite.targetY - handY;
            const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
    
            // Calculate the adjusted target position
            let adjustedTargetX = mainSprite.targetX;
            let adjustedTargetY = mainSprite.targetY;
            if (distance > mainSprite.maxRange) {
                const ratio = mainSprite.maxRange / distance;
                adjustedTargetX = handX + deltaX * ratio;
                adjustedTargetY = handY + deltaY * ratio;
            }
    
            // Function to check for collision with non-walkable map objects
            const isObstructed = (x, y) => {
                if (this.roomData && this.roomData.items) {
                    for (const roomItem of this.roomData.items) {
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
    
                            // Check if the point is within the tile's bounds
                            if (
                                x >= tileRect.x &&
                                x <= tileRect.x + tileRect.width &&
                                y >= tileRect.y &&
                                y <= tileRect.y + tileRect.height
                            ) {
                                const tileData = itemData[0]; // Assuming single tile data for simplicity
                                // Check if the tile is walkable
                                if (tileData.w !== 1) { // Assuming w=1 means walkable, otherwise it's not
                                    return { obstructed: true, collisionX: x, collisionY: y };
                                }
                            }
                        }
                    }
                }
                return { obstructed: false };
            };
    
            // Check for obstruction along the line of sight
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
    
            // Do not show the aim if the obstruction is very close
            if (obstructionDetected && Math.sqrt((finalTargetX - handX) ** 2 + (finalTargetY - handY) ** 2) < 10) {
                return;
            }
    
            this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.8)';
            this.ctx.setLineDash([5, 5]); // Dotted line
            this.ctx.beginPath();
            this.ctx.moveTo(handX, handY);
            this.ctx.lineTo(finalTargetX, finalTargetY);
            this.ctx.stroke();
            this.ctx.setLineDash([]); // Reset line dash
    
            // Draw target radius at the final target position
            this.ctx.beginPath();
            this.ctx.arc(finalTargetX, finalTargetY, mainSprite.targetRadius, 0, 2 * Math.PI);
            this.ctx.stroke();
        }
    
        if (game.isEditMode && edit_mode_window.isSelecting && edit_mode_window.selectionStart && edit_mode_window.selectionEnd) {
            const startX = Math.min(edit_mode_window.selectionStart.x, edit_mode_window.selectionEnd.x);
            const startY = Math.min(edit_mode_window.selectionStart.y, edit_mode_window.selectionEnd.y);
            const endX = Math.max(edit_mode_window.selectionStart.x, edit_mode_window.selectionEnd.x) + 16;
            const endY = Math.max(edit_mode_window.selectionStart.y, edit_mode_window.selectionEnd.y) + 16;
    
            this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.8)';
            this.ctx.lineWidth = 4 / this.zoomLevel;
            this.ctx.strokeRect(startX, startY, endX - startX, endY - startY);
        }
    
        if (game.isEditMode) {
            edit_mode_window.selectedTiles.forEach(tile => {
                this.ctx.fillStyle = 'rgba(0, 255, 0, 0.2)';
                this.ctx.fillRect(tile.x, tile.y, 16, 16);
            });
        }
    
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

    loop: function(timestamp) {
        if (!this.lastTime) {
            this.lastTime = timestamp;
        }
    
        this.deltaTime = timestamp - this.lastTime;
        this.lastTime = timestamp;
        this.fps = 1000 / this.deltaTime;
    
        for (let id in this.sprites) {
            this.sprites[id].update();
        }
    
        this.updateAnimatedTiles(this.deltaTime);
        weather.updateStars();
        weather.updateRain();
        weather.updateSnow();
        this.updateCamera();
        this.render();
        requestAnimationFrame(this.loop.bind(this));
    },

    updateAnimatedTiles: function(deltaTime) {
        if (!this.roomData || !this.roomData.items) return;

        this.roomData.items.forEach(roomItem => {
            const itemData = assets.load('objectData')[roomItem.id];
            if (itemData && itemData.length > 0) {
                itemData.forEach(tileData => {
                    if (tileData.d) {
                        if (!tileData.currentFrame) {
                            tileData.currentFrame = 0;
                        }
                        if (!tileData.elapsedTime) {
                            tileData.elapsedTime = 0;
                        }
                        tileData.elapsedTime += deltaTime;
                        if (tileData.elapsedTime >= tileData.d) {
                            tileData.elapsedTime = 0;
                            tileData.currentFrame = (tileData.currentFrame + 1) % tileData.i.length;
                        }
                    }
                });
            }
        });
    },

    collision: function(x, y, sprite) {
        let collisionDetected = false;
        const extraHeadroom = 2;

        // Define the collision box for the sprite
        const spriteCollisionBox = {
            x: x,
            y: y + extraHeadroom,
            width: sprite.width * sprite.scale,
            height: sprite.height * sprite.scale - 2 * extraHeadroom
        };

        const objectCollisionBox = {
            x: x,
            y: y + sprite.height * sprite.scale / 2,
            width: sprite.width * sprite.scale,
            height: sprite.height * sprite.scale / 2
        };

        if (this.roomData && this.roomData.items) {
            collisionDetected = this.roomData.items.some(roomItem => {
                const itemData = assets.load('objectData')[roomItem.id];
                if (!itemData) return false;

                const xCoordinates = roomItem.x || [];
                const yCoordinates = roomItem.y || [];

                let index = 0;

                return yCoordinates.some((yCoord, j) => {
                    return xCoordinates.some((xCoord, i) => {
                        const tileData = itemData[0]; // Assuming we are dealing with the first tile data group
                        const tilePosX = parseInt(xCoord, 10) * 16 + tileData.a[index % tileData.a.length];
                        const tilePosY = parseInt(yCoord, 10) * 16 + tileData.b[index % tileData.b.length];
                        const tileRect = {
                            x: tilePosX,
                            y: tilePosY,
                            width: 16,
                            height: 16
                        };

                        let collisionArray;
                        if (Array.isArray(tileData.w) && tileData.w.length > 0) {
                            collisionArray = tileData.w[index % tileData.w.length];
                        } else if (typeof tileData.w === 'number') {
                            collisionArray = [0, 0, 0, 0]; // Default offsets for non-walkable
                            if (tileData.w === 1) {
                                collisionArray = [16, 16, 16, 16]; // Fully walkable
                            }
                        }

                        index++;

                        if (collisionArray) {
                            const [nOffset, eOffset, sOffset, wOffset] = collisionArray;
                            return (
                                objectCollisionBox.x < tileRect.x + tileRect.width - eOffset &&
                                objectCollisionBox.x + objectCollisionBox.width > tileRect.x + wOffset &&
                                objectCollisionBox.y < tileRect.y + tileRect.height - sOffset &&
                                objectCollisionBox.y + objectCollisionBox.height > tileRect.y + nOffset
                            );
                        }

                        return false;
                    });
                });
            });
        }

        if (!collisionDetected) {
            for (let id in this.sprites) {
                if (this.sprites[id] !== sprite) {
                    const otherSprite = this.sprites[id];
                    const otherCollisionBox = {
                        x: otherSprite.x,
                        y: otherSprite.y + extraHeadroom,
                        width: otherSprite.width * otherSprite.scale,
                        height: otherSprite.height * otherSprite.scale - 2 * extraHeadroom
                    };

                    if (
                        spriteCollisionBox.x < otherCollisionBox.x + otherCollisionBox.width &&
                        spriteCollisionBox.x + spriteCollisionBox.width > otherCollisionBox.x &&
                        spriteCollisionBox.y < otherCollisionBox.y + otherCollisionBox.height &&
                        spriteCollisionBox.y + spriteCollisionBox.height > otherCollisionBox.y
                    ) {
                        collisionDetected = true;
                        break;
                    }
                }
            }
        }

        return collisionDetected;
    },
};