var game = {
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
    zoomLevel: 3,
    cameraX: 0,
    cameraY: 0,
    targetCameraX: 0,
    targetCameraY: 0,
    targetX: 0,
    targetY: 0,
    roomData: undefined,
    sprites: {},
    playerid: null,
    sceneid: null,
    viewportXStart: null,
    viewportXEnd: null,
    viewportYStart: null,
    viewportYEnd: null,
    desiredFPS: 60,
    fixedDeltaTime: 1000 / 60, // Fixed time step for 60 FPS
    accumulatedTime: 0,
    lastTime: null,
    maxAccumulatedTime: 1000, // To avoid spiral of death
    displayUsernames: false,
    displayChat: false,
    displaySprite: true,
    allowControls: true,
    activeCamera: true,
    selectedObject: null,
    selectedCache: null,
    pathfinding: true,
    particles: [],
    gameTime: {
        hours: 7,
        minutes: 0,
        seconds: 0,
        days: 0,
        speedMultiplier: 100, // Game time progresses 10 times faster than real time
        daysOfWeek: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        update: function(deltaTime) {
            const gameSeconds = (deltaTime / 1000) * this.speedMultiplier;
            this.seconds += gameSeconds;

            if (this.seconds >= 60) {
                this.minutes += Math.floor(this.seconds / 60);
                this.seconds = this.seconds % 60;
            }
            if (this.minutes >= 60) {
                this.hours += Math.floor(this.minutes / 60);
                this.minutes = this.minutes % 60;
            }
            if (this.hours >= 24) {
                this.days += Math.floor(this.hours / 24);
                this.hours = this.hours % 24;
            }
        },
        display: function() {
            const pad = (num) => String(num).padStart(2, '0');
            const dayOfWeek = this.daysOfWeek[this.days % 7];
            return `${dayOfWeek} ${pad(this.hours)}:${pad(this.minutes)}`;
        }
    },

    reloadGameData: function() {
        // Specify the assets you want to reload
        const assetsToReload = ['objectData', 'roomData']; // Add other assets as needed

        assets.reloadAssets(assetsToReload, () => {
            console.log("Game data reloaded");
            this.roomData = assets.load('roomData');
            // Perform any additional updates necessary for your game
            this.updateGameElements();
        });
    },

    updateGameElements: function() {
        // Any specific updates to game elements after reloading data can be added here
        console.log("Game elements updated");
        // Example: this.refreshSprites(); if you need to update sprites specifically
    },

    init: function() {
        this.playerid = network.getToken('renaccount') || `player_${Math.floor(Math.random() * 10000)}`;

        assets.preload([
            { name: 'character', path: 'img/sprites/character.png' },
            { name: 'hair', path: 'img/sprites/hair.png' },
            { name: 'hats', path: 'img/sprites/hats.png' },
            { name: 'glasses', path: 'img/sprites/glasses.png' },
            { name: 'facial', path: 'img/sprites/facial.png' },
            { name: 'outfit', path: 'img/sprites/outfit.png' },
            { name: '1', path: 'img/tiles/1.png' },
            { name: 'objectData', path: 'json/objectData.json' },
            { name: 'objectScript', path: 'json/objectScript.json' },
            { name: 'walkAudio', path: 'audio/sfx/walk.json' },

        ], () => {
            console.log("All assets loaded");
            this.canvas = document.createElement('canvas');
            this.ctx = this.canvas.getContext('2d');
            document.body.appendChild(this.canvas);
            this.resizeCanvas();
            this.objectData = assets.load('objectData');

            // Create player sprite
            const playerOptions = {
                id: this.playerid,
                x: 300,
                y: 450,
                isPlayer: true,
                speed: 90
            };
            sprite.create(playerOptions);

            weather.starsActive = true;
            weather.fogActive = true;
            weather.rainActive = true;
            weather.snowActive = false;
            weather.nightActive = true;

            weather.createStars();
            weather.createFog(0.05);
            weather.createRain(0.7);
            weather.createSnow(0.2);

            this.loadScene('66771b7e6c1c5b2f1708b75a');
            modal.load('ui');
            modal.load('pie_menu');
            modal.load('quick_menu');
            console.log("Connected to Main renzora server");

            this.loop();

            const mainSprite = game.sprites[this.playerid];
            if (mainSprite) {
                mainSprite.updateHealth(0);
                mainSprite.updateHealth(mainSprite.health);  
                mainSprite.updateEnergy(mainSprite.energy);  
            }

            // Send initial player state to the server
            network.send({
                command: 'playerStateUpdate',
                data: {
                    id: this.playerid,
                    ...this.sprites[this.playerid]
                }
            });

            // Add this line to allow triggering reload from the console or UI
            window.reloadGameData = this.reloadGameData.bind(this);
            this.canvas.addEventListener('click', this.handleCanvasClick.bind(this));
        });
    },

    loadScene: function(sceneId) {
        input.cancelPathfinding(game.sprites[game.playerid]);
        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'modals/servers/ajax/getSceneData.php',
            data: 'scene_id=' + encodeURIComponent(sceneId),
            success: function(data) {
                if (data.message === 'success') {
                    game.roomData = data.roomData;
                    game.sceneid = data.sceneid;
                    effects.transitions.start('fadeOut', 1000);
                    effects.transitions.start('fadeIn', 1000);
                    ui.notif("scene_change_notif", data.name, true);

                    console.log('Scene loaded. Room data:', game.roomData);
                } else {
                    console.log('Error: ' + data.message);
                }
            },
            error: function(data) {
                console.log(data);
            }
        });
    },

    resizeCanvas: function() {
        this.canvas.width = window.innerWidth;
        this.canvas.height = window.innerHeight;
        this.canvas.style.position = 'absolute';
        this.canvas.style.left = '50%';
        this.canvas.style.top = '50%';
        this.canvas.style.transform = 'translate(-50%, -50%)';
    },
       
    updateCamera: function() {
        if (this.activeCamera) {
            let mainSprite = game.sprites[this.playerid];
            if (mainSprite) {
                var scaledWindowWidth = window.innerWidth / game.zoomLevel;
                var scaledWindowHeight = window.innerHeight / game.zoomLevel;
    
                this.targetCameraX = mainSprite.x + mainSprite.width / 2 - scaledWindowWidth / 2;
                this.targetCameraY = mainSprite.y + mainSprite.height / 2 - scaledWindowHeight / 2;
    
                this.targetCameraX = Math.max(0, Math.min(this.targetCameraX, game.worldWidth - scaledWindowWidth));
                this.targetCameraY = Math.max(0, Math.min(this.targetCameraY, game.worldHeight - scaledWindowHeight));
    
                this.cameraX = this.lerp(this.cameraX, this.targetCameraX, this.lerpFactor);
                this.cameraY = this.lerp(this.cameraY, this.targetCameraY, this.lerpFactor);
    
                // Center map if smaller than viewport
                if (game.worldWidth < scaledWindowWidth) {
                    this.cameraX = -(scaledWindowWidth - game.worldWidth) / 2;
                }
                if (game.worldHeight < scaledWindowHeight) {
                    this.cameraY = -(scaledWindowHeight - game.worldHeight) / 2;
                }
    
                if (typeof debug_window !== 'undefined' && debug_window.camera) {
                    debug_window.camera();
                }
            } else {
                console.error('Main sprite not found.');
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

    grid: function() {
        game.ctx.strokeStyle = 'rgba(0, 0, 0, 0.1)';
        game.ctx.lineWidth = 1;
        for (var x = 0; x <= game.worldWidth; x += 16) {
          game.ctx.beginPath();
          game.ctx.moveTo(x, 0);
          game.ctx.lineTo(x, game.worldHeight);
          game.ctx.stroke();
        }
        for (var y = 0; y <= game.worldHeight; y += 16) {
          game.ctx.beginPath();
          game.ctx.moveTo(0, y);
          game.ctx.lineTo(game.worldWidth, y);
          game.ctx.stroke();
        }
      },


      handleCanvasClick: function(event) {
        const rect = this.canvas.getBoundingClientRect();
        const mouseX = (event.clientX - rect.left) / this.zoomLevel + this.cameraX;
        const mouseY = (event.clientY - rect.top) / this.zoomLevel + this.cameraY;

        this.selectedObject = this.findObjectAt(mouseX, mouseY);
        if (this.selectedObject) {
            console.log('Selected Object:', this.selectedObject);
            this.selectedCache = this.drawAndOutlineObjectImage(this.selectedObject);
            console.log('Selected Cache:', this.selectedCache);
        } else {
            this.selectedCache = null; // Clear the cache if no object is selected
        }

        if (!input.isSpacePressed && this.pathfinding) {
            const gridX = Math.floor(mouseX / 16);
            const gridY = Math.floor(mouseY / 16);

            this.sprites[this.playerid].walkToClickedTile(gridX, gridY);
        }
    },

    createWalkableGrid: function() {
        const width = this.worldWidth / 16;
        const height = this.worldHeight / 16;
        const grid = Array.from({ length: width }, () => Array(height).fill(1)); // Initialize all cells as walkable (1)
    
        if (!this.objectData) {
            console.error('Object Data is not defined.');
            return grid;
        }
    
        if (this.roomData && this.roomData.items) {
            this.roomData.items.forEach(item => {
                const itemData = this.objectData[item.id];
                if (itemData && itemData.length > 0) {
                    const tileData = itemData[0];
                    const xCoordinates = item.x || [];
                    const yCoordinates = item.y || [];
    
                    yCoordinates.forEach((tileY, rowIndex) => {
                        xCoordinates.forEach((tileX, colIndex) => {
                            const index = rowIndex * xCoordinates.length + colIndex;
                            const walkableData = Array.isArray(tileData.w) ? tileData.w[index % tileData.w.length] : tileData.w;
    
                            if (tileX >= 0 && tileX < width && tileY >= 0 && tileY < height) {
                                if (Array.isArray(walkableData) && walkableData.length === 4) {
                                    // Check if any direction is non-walkable
                                    const [north, east, south, west] = walkableData;
                                    if (north < 16 || east < 16 || south < 16 || west < 16) {
                                        grid[tileX][tileY] = 0;
                                    }
                                } else if (walkableData === 0) {
                                    grid[tileX][tileY] = 0; // Non-walkable if the value is 0
                                } else if (walkableData === 1 && grid[tileX][tileY] !== 0) {
                                    grid[tileX][tileY] = 1; // Walkable if the value is 1, but don't override if already non-walkable
                                }
                            }
                        });
                    });
                }
            });
        }
    
        return grid;
    },       

    findObjectAt: function(x, y) {
        if (!this.roomData || !this.roomData.items) return null;
    
        const renderQueue = [];
    
        // Populate renderQueue with room items
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
    
                        renderQueue.push({
                            tileIndex: tileFrameIndex,
                            posX: posX,
                            posY: posY,
                            z: Array.isArray(tileData.z) ? tileData.z[index % tileData.z.length] : tileData.z,
                            id: roomItem.id,
                            item: roomItem
                        });
    
                        index++;
                    }
                }
            }
        });
    
        // Sort renderQueue by z-index and render order
        renderQueue.sort((a, b) => a.z - b.z || a.renderOrder - b.renderOrder);
    
        // Find the object at the specified coordinates that rendered last
        let highestZIndexObject = null;
    
        for (const item of renderQueue) {
            const tileRect = {
                x: item.posX,
                y: item.posY,
                width: 16,
                height: 16
            };
    
            if (
                x >= tileRect.x &&
                x <= tileRect.x + tileRect.width &&
                y >= tileRect.y &&
                y <= tileRect.y + tileRect.height
            ) {
                highestZIndexObject = item.item;
            }
        }
    
        return highestZIndexObject;
    },    

    drawAndOutlineObjectImage: function(object) {
        if (!object) return null;
        
        const itemData = assets.load('objectData')[object.id];
        if (!itemData) return null;
        
        const xCoordinates = object.x.map(x => parseInt(x, 10) * 16);
        const yCoordinates = object.y.map(y => parseInt(y, 10) * 16);
        
        const minX = Math.min(...xCoordinates);
        const maxX = Math.max(...xCoordinates) + 16;
        const minY = Math.min(...yCoordinates);
        const maxY = Math.max(...yCoordinates) + 16;
        
        // Create an offscreen canvas
        const offscreenCanvas = document.createElement('canvas');
        const offscreenCtx = offscreenCanvas.getContext('2d');
        offscreenCanvas.width = maxX - minX;
        offscreenCanvas.height = maxY - minY;
        
        // Render object tiles onto the offscreen canvas
        for (let i = 0; i < xCoordinates.length; i++) {
            for (let j = 0; j < yCoordinates.length; j++) {
                const itemX = xCoordinates[i] - minX;
                const itemY = yCoordinates[j] - minY;
        
                const tileIndex = j * xCoordinates.length + i; // Calculate tile index based on row-major order
                const tileData = itemData[tileIndex % itemData.length];
        
                let tileFrameIndex;
                if (tileData.d) {
                    const currentFrame = tileData.currentFrame || 0;
                    tileFrameIndex = Array.isArray(tileData.i) ? tileData.i[currentFrame % tileData.i.length] : tileData.i;
                } else {
                    tileFrameIndex = tileData.i[tileIndex % tileData.i.length];
                }
        
                const srcX = (tileFrameIndex % 150) * 16;
                const srcY = Math.floor(tileFrameIndex / 150) * 16;
        
                offscreenCtx.drawImage(assets.load(tileData.t), srcX, srcY, 16, 16, itemX, itemY, 16, 16);
            }
        }
    
        const width = offscreenCanvas.width;
        const height = offscreenCanvas.height;
    
        // Get the image data from the offscreen canvas
        const imageData = offscreenCtx.getImageData(0, 0, width, height);
        const data = imageData.data;
    
        // Create a copy of the image data to preserve the original image
        const outlineData = new Uint8ClampedArray(data);
    
        // Function to check and outline a pixel
        const outlinePixel = (x, y) => {
            const index = (y * width + x) * 4;
            outlineData[index] = 255;     // Red
            outlineData[index + 1] = 255; // Green
            outlineData[index + 2] = 255; // Blue
            outlineData[index + 3] = 255; // Alpha
        };
    
        // Outline inner transparent neighbor pixels
        for (let y = 0; y < height; y++) {
            for (let x = 0; x < width; x++) {
                const index = (y * width + x) * 4;
                if (data[index + 3] === 0) {
                    const neighbors = [
                        {x: x - 1, y: y},     // Left
                        {x: x + 1, y: y},     // Right
                        {x: x, y: y - 1},     // Top
                        {x: x, y: y + 1},     // Bottom
                        {x: x - 1, y: y - 1}, // Top-left
                        {x: x + 1, y: y - 1}, // Top-right
                        {x: x - 1, y: y + 1}, // Bottom-left
                        {x: x + 1, y: y + 1}  // Bottom-right
                    ];
    
                    for (const neighbor of neighbors) {
                        if (
                            neighbor.x >= 0 && neighbor.x < width &&
                            neighbor.y >= 0 && neighbor.y < height
                        ) {
                            const neighborIndex = (neighbor.y * width + neighbor.x) * 4;
                            if (data[neighborIndex + 3] !== 0) {
                                outlinePixel(x, y);
                                break;
                            }
                        }
                    }
                }
            }
        }
    
        // Outline the very edge pixels
        for (let x = 0; x < width; x++) {
            if (data[x * 4 + 3] !== 0) outlinePixel(x, 0); // Top edge
            if (data[((height - 1) * width + x) * 4 + 3] !== 0) outlinePixel(x, height - 1); // Bottom edge
        }
        for (let y = 0; y < height; y++) {
            if (data[(y * width) * 4 + 3] !== 0) outlinePixel(0, y); // Left edge
            if (data[(y * width + (width - 1)) * 4 + 3] !== 0) outlinePixel(width - 1, y); // Right edge
        }
    
        // Put the modified image data back into the outlined canvas
        const outlinedImageData = new ImageData(outlineData, width, height);
        offscreenCtx.putImageData(outlinedImageData, 0, 0);
    
        return offscreenCanvas;
    },    

    render: function() {
        this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
        this.ctx.setTransform(1, 0, 0, 1, 0, 0);
        this.ctx.scale(this.zoomLevel, this.zoomLevel);
        this.ctx.translate(-Math.round(this.cameraX), -Math.round(this.cameraY));

        const mainSprite = this.sprites[this.playerid];
    
        const renderQueue = [];
    
        // Calculate the boundaries of the viewport in world coordinates
        this.viewportXStart = Math.max(0, Math.floor(this.cameraX / 16));
        this.viewportXEnd = Math.min(this.worldWidth / 16, Math.ceil((this.cameraX + window.innerWidth / this.zoomLevel) / 16));
        this.viewportYStart = Math.max(0, Math.floor(this.cameraY / 16));
        this.viewportYEnd = Math.min(this.worldHeight / 16, Math.ceil((this.cameraY + window.innerHeight / this.zoomLevel) / 16));
    
        let tileCount = 0;
    
        game.grid();
    
        if (this.roomData && this.roomData.items) {
            this.roomData.items.forEach(roomItem => {
                const itemData = this.objectData[roomItem.id];
                if (itemData && itemData.length > 0) {
                    const tileData = itemData[0];
                    const xCoordinates = roomItem.x || [];
                    const yCoordinates = roomItem.y || [];
    
                    let index = 0;
    
                    for (let y = Math.min(...yCoordinates); y <= Math.max(...yCoordinates); y++) {
                        for (let x = Math.min(...xCoordinates); x <= Math.max(...xCoordinates); x++) {
                            // Only add tiles within the viewport to the render queue
                            if (x >= this.viewportXStart && x < this.viewportXEnd && y >= this.viewportYStart && y < this.viewportYEnd) {
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
                                    id: roomItem.id,
                                    draw: function() {
                                        game.ctx.drawImage(assets.load(tileData.t), srcX, srcY, 16, 16, this.posX, this.posY, 16, 16);
                                    }
                                });
    
                                tileCount++;
                            }
    
                            index++;
                        }
                    }
    
                    // Check if this is the selected object and add it to the renderQueue as a single image
                    if (roomItem === this.selectedObject && this.selectedCache) {
                        const minX = Math.min(...xCoordinates) * 16;
                        const minY = Math.min(...yCoordinates) * 16;
                        renderQueue.push({
                            z: Array.isArray(tileData.z) ? Math.max(...tileData.z) : tileData.z,
                            draw: function() {
                                game.ctx.drawImage(game.selectedCache, minX, minY);
                            }
                        });
                    }
                }
            });
        }
    
        let spriteCount = 0;
    
        for (let id in this.sprites) {
            const sprite = this.sprites[id];
            const spriteRight = sprite.x + sprite.width;
            const spriteBottom = sprite.y + sprite.height;
    
            // Check if sprite is within the viewport
            if (spriteRight >= this.viewportXStart * 16 && sprite.x < this.viewportXEnd * 16 &&
                spriteBottom >= this.viewportYStart * 16 && sprite.y < this.viewportYEnd * 16) {
                renderQueue.push({
                    z: 1,
                    draw: function() {
                        game.sprites[id].draw();
                    }
                });
                spriteCount++;
            }
        }
    
        renderQueue.sort((a, b) => a.z - b.z);
        renderQueue.forEach((item, index) => {
            item.renderOrder = index;
            item.draw();
        });
        this.ctx.imageSmoothingEnabled = false;

        if (mainSprite && mainSprite.path && mainSprite.path.length > 0) {
            this.ctx.strokeStyle = 'rgba(255, 0, 0, 0.5)';
            this.ctx.lineWidth = 2;
            this.ctx.beginPath();
    
            // Move to the first point
            this.ctx.moveTo(mainSprite.path[0].x * 16 + 8, mainSprite.path[0].y * 16 + 8);
    
            // Draw quadratic curves between points
            for (let i = 1; i < mainSprite.path.length - 1; i++) {
                const currentPoint = mainSprite.path[i];
                const nextPoint = mainSprite.path[i + 1];
                const midX = (currentPoint.x + nextPoint.x) * 8 + 8;
                const midY = (currentPoint.y + nextPoint.y) * 8 + 8;
    
                this.ctx.quadraticCurveTo(currentPoint.x * 16 + 8, currentPoint.y * 16 + 8, midX, midY);
            }
    
            // Draw the last segment
            const lastPoint = mainSprite.path[mainSprite.path.length - 1];
            this.ctx.lineTo(lastPoint.x * 16 + 8, lastPoint.y * 16 + 8);
    
            this.ctx.stroke();
        }
    
        weather.applyNightColorFilter(); // Apply the night color filter
        weather.drawSnow();
        weather.drawRain();
        weather.drawFog();
        weather.drawStars();
        weather.drawLightning();
        this.handleAimAttack();

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
    
        // Draw ID bubbles last to ensure they appear above all other elements
        if(this.displayUsernames && this.displayChat) {
            for (let id in this.sprites) {
                if(this.displayUsernames) { this.drawIdBubble(this.sprites[id]); }
                if(this.displayChat) { this.drawChatBubble(this.sprites[id]); }
            }
        }

        effects.renderParticles();
        effects.transitions.render();

    }, 

    randomNpcMessage: function(sprite) {
        if (sprite.messages && sprite.messages.length > 0) {
            const randomIndex = Math.floor(Math.random() * sprite.messages.length);
            const message = sprite.messages[randomIndex];
            this.updateChatMessages(sprite, message);
        }
    },
    
    drawIdBubble: function(sprite) {
        if (!sprite || !sprite.id) return;
    
        // Truncate text if it's longer than 16 characters
        let text = sprite.id;
        if (text.length > 16) {
            text = text.slice(0, 13);
        }
    
        const bubbleHeight = 7;
        const bubblePadding = 2;
        const fontSize = 3;
        const characterSpacing = -0.1; // Adjust this value for tighter or looser tracking
        
        // Calculate text width
        this.ctx.font = `${fontSize}px Tahoma`;
        let textWidth = 0;
        for (let char of text) {
            textWidth += this.ctx.measureText(char).width + characterSpacing;
        }
        textWidth -= characterSpacing; // Remove the extra spacing added after the last character
    
        // Calculate bubble dimensions
        const bubbleWidth = textWidth + 2 * bubblePadding;
    
        // Calculate bubble position
        const bubbleX = sprite.x + sprite.width / 2 - bubbleWidth / 2;
        const bubbleY = sprite.y - bubbleHeight - bubblePadding + 5; // Adjust this value to bring the bubble down
    
        // Draw rounded rectangle bubble with less pronounced corners
        const radius = 2; // Adjust the radius for subtler rounded corners
        this.ctx.fillStyle = 'rgba(0, 0, 0, 0.7)';
        this.ctx.beginPath();
        this.ctx.moveTo(bubbleX + radius, bubbleY);
        this.ctx.lineTo(bubbleX + bubbleWidth - radius, bubbleY);
        this.ctx.quadraticCurveTo(bubbleX + bubbleWidth, bubbleY, bubbleX + bubbleWidth, bubbleY + radius);
        this.ctx.lineTo(bubbleX + bubbleWidth, bubbleY + bubbleHeight - radius);
        this.ctx.quadraticCurveTo(bubbleX + bubbleWidth, bubbleY + bubbleHeight, bubbleX + bubbleWidth - radius, bubbleY + bubbleHeight);
        this.ctx.lineTo(bubbleX + radius, bubbleY + bubbleHeight);
        this.ctx.quadraticCurveTo(bubbleX, bubbleY + bubbleHeight, bubbleX, bubbleY + bubbleHeight - radius);
        this.ctx.lineTo(bubbleX, bubbleY + radius);
        this.ctx.quadraticCurveTo(bubbleX, bubbleY, bubbleX + radius, bubbleY);
        this.ctx.closePath();
        this.ctx.fill();
    
        // Draw each character with fixed spacing
        this.ctx.fillStyle = 'white';
        this.ctx.font = `${fontSize}px Tahoma`;
        let charX = bubbleX + bubblePadding;
        for (let char of text) {
            this.ctx.fillText(char, charX, bubbleY + bubbleHeight / 2 + fontSize / 3);
            charX += this.ctx.measureText(char).width + characterSpacing;
        }
    },
    
    drawChatBubble: function(sprite) {
        if (!sprite.chatMessages || sprite.chatMessages.length === 0) return;

        // Iterate through each message
        for (let i = 0; i < sprite.chatMessages.length; i++) {
            const messageData = sprite.chatMessages[i];
            const elapsedTime = Date.now() - messageData.time;
            
            if (elapsedTime > 5000) {
                sprite.chatMessages.splice(i, 1);
                i--;
                continue;
            }
            
            const fadeOutTime = 1000; // 1 second fade-out duration
            const alpha = elapsedTime > 4000 ? (1 - (elapsedTime - 4000) / fadeOutTime) : 1; // Start fading out after 4 seconds
        
            const message = messageData.text;
            const bubbleHeight = 7;
            const bubblePadding = 2;
            const fontSize = 3;
            const characterSpacing = -0.1; // Adjust this value for tighter or looser tracking
        
            // Calculate text width
            game.ctx.font = `${fontSize}px Tahoma`;
            let textWidth = 0;
            for (let char of message) {
                textWidth += game.ctx.measureText(char).width + characterSpacing;
            }
            textWidth -= characterSpacing; // Remove the extra spacing added after the last character
        
            // Calculate bubble dimensions
            const bubbleWidth = textWidth + 2 * bubblePadding;
        
            // Calculate bubble position
            const bubbleX = sprite.x + sprite.width / 2 - bubbleWidth / 2;
            const baseBubbleY = sprite.y - 12; // Move the first bubble up by 2-3 pixels
            const bubbleY = baseBubbleY - (i * (bubbleHeight + bubblePadding - 1)); // Reduce vertical spacing between bubbles
    
            // Draw rounded rectangle bubble with blue color
            const radius = 2; // Adjust the radius for subtler rounded corners
            game.ctx.fillStyle = `rgba(0, 0, 255, ${alpha * 0.9})`; // Blue color with fading effect
            game.ctx.beginPath();
            game.ctx.moveTo(bubbleX + radius, bubbleY);
            game.ctx.lineTo(bubbleX + bubbleWidth - radius, bubbleY);
            game.ctx.quadraticCurveTo(bubbleX + bubbleWidth, bubbleY, bubbleX + bubbleWidth, bubbleY + radius);
            game.ctx.lineTo(bubbleX + bubbleWidth, bubbleY + bubbleHeight - radius);
            game.ctx.quadraticCurveTo(bubbleX + bubbleWidth, bubbleY + bubbleHeight, bubbleX + bubbleWidth - radius, bubbleY + bubbleHeight);
            game.ctx.lineTo(bubbleX + radius, bubbleY + bubbleHeight);
            game.ctx.quadraticCurveTo(bubbleX, bubbleY + bubbleHeight, bubbleX, bubbleY + bubbleHeight - radius);
            game.ctx.lineTo(bubbleX, bubbleY + radius);
            game.ctx.quadraticCurveTo(bubbleX, bubbleY, bubbleX + radius, bubbleY);
            game.ctx.closePath();
            game.ctx.fill();
        
            // Draw each character with fixed spacing
            game.ctx.fillStyle = `rgba(255, 255, 255, ${alpha})`;
            game.ctx.font = `${fontSize}px Tahoma`;
            let charX = bubbleX + bubblePadding;
            for (let char of message) {
                game.ctx.fillText(char, charX, bubbleY + bubbleHeight / 2 + fontSize / 2);
                charX += game.ctx.measureText(char).width + characterSpacing;
            }
        }
    },

    updateChatMessages: function(sprite, newMessage) {
        if (!sprite.chatMessages) {
            sprite.chatMessages = [];
        }
        // Add new message to the queue
        sprite.chatMessages.push({ text: newMessage, time: Date.now() });
        
        // Ensure only the last 3 messages are kept
        if (sprite.chatMessages.length > 3) {
            sprite.chatMessages.shift();
        }
    },
    
    loop: function(timestamp) {
        if (!this.lastTime) {
            this.lastTime = timestamp;
            requestAnimationFrame(this.loop.bind(this));
            return;
        }
    
        // Calculate time elapsed since the last frame was drawn
        const timeElapsed = timestamp - this.lastTime;
    
        // If the tab was inactive and a large time delay occurred, cap the time step
        if (timeElapsed > 1000) { // 1000 milliseconds threshold, can be adjusted
            this.accumulatedTime = this.fixedDeltaTime; // Skip the catch-up frames
        } else {
            this.accumulatedTime += timeElapsed;
        }
    
        this.deltaTime = this.fixedDeltaTime; // Use fixed delta time for stable updates
        this.lastTime = timestamp;
    
        // Process the game logic in fixed steps
        while (this.accumulatedTime >= this.fixedDeltaTime) {
            this.updateGameLogic(this.fixedDeltaTime);
            this.accumulatedTime -= this.fixedDeltaTime;
        }
    
        this.render();
    
        // FPS monitoring
        var debugFPS = document.getElementById('gameFps');
        var fps = 1000 / timeElapsed; // Calculate FPS using time elapsed since last frame
        if (debugFPS) {
            debugFPS.innerHTML = "FPS: " + fps.toFixed(2);
        }
    
        // Continue the loop
        requestAnimationFrame(this.loop.bind(this));
    },

    updateGameLogic: function(deltaTime) {
        // Update game time
        this.gameTime.update(deltaTime);
    
        for (let id in this.sprites) {
            if (this.sprites[id].update) {
                this.sprites[id].update(deltaTime);
            }
        }
    
        this.updateAnimatedTiles(deltaTime);
        weather.updateSnow(deltaTime);
        weather.updateRain(deltaTime);
        weather.updateFog(deltaTime);
        weather.updateStars(deltaTime);
        weather.updateLightning(deltaTime);
        this.updateCamera();
        effects.updateParticles(deltaTime);
        effects.transitions.update();
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
                const itemData = this.objectData[roomItem.id];
                if (!itemData) return false;
    
                const xCoordinates = roomItem.x || [];
                const yCoordinates = roomItem.y || [];
    
                return yCoordinates.some((tileY, rowIndex) => {
                    return xCoordinates.some((tileX, colIndex) => {
                        const index = rowIndex * xCoordinates.length + colIndex;
                        const tileData = itemData[0]; // Assuming we are dealing with the first tile data group
                        const tilePosX = tileX * 16 + tileData.a[index % tileData.a.length];
                        const tilePosY = tileY * 16 + tileData.b[index % tileData.b.length];
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
                            if (tileData.w === 1) {
                                collisionArray = [16, 16, 16, 16]; // Fully walkable
                            } else if (tileData.w === 0) {
                                collisionArray = [0, 0, 0, 0]; // Fully non-walkable
                            }
                        }
    
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

    findFreeLocation: function(width, height) {
        const maxAttempts = 30; // Maximum number of attempts to find a free location
        for (let attempt = 0; attempt < maxAttempts; attempt++) {
            let x = Math.floor(Math.random() * (this.worldWidth - width));
            let y = Math.floor(Math.random() * (this.worldHeight - height));
    
            // Create a dummy sprite for collision testing
            let testSprite = {
                x: x,
                y: y,
                width: width,
                height: height,
                scale: 1 // Assuming scale is 1 for simplicity
            };
    
            // Check for overlaps with other sprites
            let overlappingSprites = Object.values(this.sprites).some(sprite => {
                return x < sprite.x + sprite.width && x + width > sprite.x &&
                       y < sprite.y + sprite.height && y + height > sprite.y;
            });
    
            // Check for tile collisions
            let tileCollision = this.collision(x, y, testSprite);
    
            // If no overlapping sprites and no tile collision, return this location
            if (!overlappingSprites && !tileCollision) {
                return { x, y }; // Returns a free location as an object with x and y properties
            }
        }
        return null; // Return null if no free location is found
    }
};
