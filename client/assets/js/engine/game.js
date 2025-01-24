game = {
    needsFilterUpdate: true,
    canvas: undefined,
    ctx: undefined,
    isEditMode: false,
    x: null,
    y: null,
    timestamp: 0,
    worldWidth: 1280,
    worldHeight: 944,
    zoomLevel: localStorage.getItem('zoomLevel') ? parseInt(localStorage.getItem('zoomLevel')) : 5,
    targetX: 0,
    targetY: 0,
    roomData: undefined,
    sprites: {},
    objectData: null,
    spriteData: null,
    playerid: null,
    sceneid: null,
    desiredFPS: 60,
    fixedDeltaTime: 1000 / 60,
    deltaTime: null,
    accumulatedTime: 0,
    lastTime: null,
    maxAccumulatedTime: 1000,
    allowControls: true,
    selectedObjects: [],
    selectedCache: [],
    pathfinding: true,
    selectedTiles: [],
    overlappingTiles: [],
    isPaused: false,
    inputMethod: 'keyboard',
    fpsHistory: [],
    maxFpsHistory: 60,
    spriteCount: 0,
    animationCount: 0,
    backgroundTileCount: 0,
    tileCount: 0,
    renderQueue: [],
    sceneBg: null,
    viewportXStart: 0,
    viewportXEnd: 0,
    viewportYStart: 0,
    viewportYEnd: 0,
    currentTileData: null,
    currentRoomItem: null,

    loadGameAssets: function(onLoaded) {
        fetch('/api/tileset_manager/list_sheets')
            .then(response => response.json())
            .then(data => {
                if (!data.success || !Array.isArray(data.sheets)) {
                    throw new Error('Failed to load sheets list from server');
                }
                const sheetPreloadArray = data.sheets.map(sheetName => ({
                    name: sheetName,
                    path: `assets/img/sheets/${sheetName}.png`
                }));
                assets.preload(sheetPreloadArray, () => {
                    console.log('All sheets have been preloaded.');
                    if (typeof onLoaded === 'function') {
                        onLoaded();
                    }
                });
            })
            .catch(err => {
                console.error('Error loading sheets:', err);
                if (typeof onLoaded === 'function') {
                    onLoaded(err);
                }
            });
    },

    create: function(config = {}) {
        this.loadGameAssets(() => {
            this.canvas = document.createElement('canvas');
            this.ctx = this.canvas.getContext('2d');
            this.ctx.imageSmoothingEnabled = false;
            document.body.appendChild(this.canvas);

            input.assign('resize', (e) => this.resizeCanvas(e));
            this.resizeCanvas();
            this.loop();
            gamepad.init(config);
            plugin.hook('onGameCreate');

            this.canvas.addEventListener('mouseup', this.handleMouseUp.bind(this));
            this.canvas.addEventListener('contextmenu', (e) => e.preventDefault());

            document.addEventListener('visibilitychange', () => {
                if (document.hidden) {
                    this.pause();
                } else {
                    this.resume();
                }
            });

            if (config.objectData) {
                this.objectData = config.objectData;
            }
            if (config.spriteData) {
                this.spriteData = config.spriteData;
            }

            if (config.player) {
                this.mainSprite = config.player;
                this.playerid = this.mainSprite.id;
                this.sprites[this.playerid] = this.mainSprite;
            }

            if (typeof config.after === 'function') {
                config.after();
            }
        });
    },

    pause: function() {
        cancelAnimationFrame(this.animationFrameId);
        plugin.audio.pauseAll();
        this.isPaused = true;
    },
    
    resume: function() {
        plugin.network.send({
            command: 'requestGameState',
            playerId: this.playerid
        });

        plugin.audio.resumeAll();
    },

    resizeCanvas: function() {
        this.setZoomLevel();
    
        const consoleElement = document.getElementById('console_window');
        const adjacentMenu = document.getElementById('tabs');
        let consoleWidth = 0;
        let menuWidth = 0;
    
        if (!this.isEditMode && consoleElement && console_window.isOpen) {
            consoleWidth = consoleElement.offsetWidth;
        }
        if (adjacentMenu && adjacentMenu.style.display !== 'none') {
            menuWidth = adjacentMenu.offsetWidth;
        }
    
        const totalOffsetWidth = consoleWidth + menuWidth;
        const availableWidth = window.innerWidth - totalOffsetWidth;
        const availableHeight = window.innerHeight;
        const canvasWidth = Math.min(this.worldWidth * this.zoomLevel, availableWidth);
        const canvasHeight = Math.min(this.worldHeight * this.zoomLevel, availableHeight);

        this.canvas.width = canvasWidth;
        this.canvas.height = canvasHeight;
        this.canvas.style.width = `${canvasWidth}px`;
        this.canvas.style.height = `${canvasHeight}px`;

        const horizontalOffset = (availableWidth - canvasWidth) / 2 + totalOffsetWidth;
        const verticalOffset = (availableHeight - canvasHeight) / 2;
        this.canvas.style.position = 'absolute';
        this.canvas.style.left = `${horizontalOffset}px`;
        this.canvas.style.top = `${verticalOffset}px`;
    },
    
    handleMouseUp: function(event) {
        if (this.isEditMode || (this.mainSprite && this.mainSprite.targetAim)) return;
    
        const rect = this.canvas.getBoundingClientRect();
        const mouseX = (event.clientX - rect.left) / this.zoomLevel + camera.cameraX;
        const mouseY = (event.clientY - rect.top) / this.zoomLevel + camera.cameraY;
        this.x = Math.floor(mouseX / 16);
        this.y = Math.floor(mouseY / 16);
        
        if (plugin.exists('collision')) {
            if (!plugin.collision.isTileWalkable(this.x, this.y)) {
                console.log(`[handleMouseUp] => Tile (${this.x}, ${this.y}) is NOT walkable.`);
                return;
            }
        } else {
            console.log("[handleMouseUp] => No collision plugin; skipping walkable check...");
        }
    
        if (plugin.exists('pathfinding')) {
            console.log(`[handleMouseUp] => Telling pathfinding to move sprite to (${this.x}, ${this.y}).`);
            plugin.pathfinding.walkToClickedTile(this.mainSprite, this.x, this.y);
        } else {
            console.log("[handleMouseUp] => No pathfinding plugin found. Unable to path.");
        }
    },
       
    scene: function(sceneId) {
        plugin.pathfinding.cancelPathfinding(this.sprites[this.playerid]);
    
        fetch(`/api/scenes/${encodeURIComponent(sceneId)}`, {
            method: 'GET',
            headers: {
                'Content-Type': 'application/json'
            }
        })
        .then(response => {
            if (!response.ok) {
                throw new Error(`Network response was not ok: ${response.statusText}`);
            }
            return response.json();
        })
        .then(data => {
            console.log('Scene response:', data);
    
            if (data.message === 'success') {
                plugin.lighting.clearLightsAndEffects();
                game.roomData = data.roomData;
                game.sceneid = data._id;
                game.serverid = data.server_id;
                game.worldWidth = data.width || 1280;
                game.worldHeight = data.height || 944;
                game.x = data.startingX || 0;
                game.y = data.startingY || 0;
    
                const playerSprite = game.sprites[game.playerid];
                if (playerSprite) {
                    playerSprite.x = game.x;
                    playerSprite.y = game.y;
                }
    
                this.sceneBg = data.bg || null;
                game.resizeCanvas();
    
                plugin.collision.walkableGridCache = null;
                plugin.collision.createWalkableGrid();
    
                game.overlappingTiles = [];
                camera.update();
    
                plugin.effects.start('fadeOut', 1000);
                plugin.effects.start('fadeIn', 1000);
    
                console.log('scene loaded successfully');
            } else {
                console.log('Scene load error:', data.message);
                plugin.load('errors', { ext: 'html' });
            }
        })
        .catch(error => {
            console.error('Scene load error:', error);
            plugin.load('errors', { ext: 'html' });
        });
    },
    

    loop: function(timestamp) {
        if (!this.lastTime) {
            this.lastTime = timestamp;
            this.lastFpsUpdateTime = timestamp;
            requestAnimationFrame(this.loop.bind(this));
            return;
        }
    
        const timeElapsed = timestamp - this.lastTime;
        if (timeElapsed > 1000) {
            this.accumulatedTime = this.fixedDeltaTime;
        } else {
            this.accumulatedTime += timeElapsed;
        }
        this.deltaTime = this.fixedDeltaTime;
        this.lastTime = timestamp;
    
        while (this.accumulatedTime >= this.fixedDeltaTime) {
            gamepad.updateGamepadState();
            
            this.viewportXStart = Math.floor(camera.cameraX / 16);
            this.viewportXEnd   = Math.ceil((camera.cameraX + window.innerWidth / this.zoomLevel) / 16);
            this.viewportYStart = Math.floor(camera.cameraY / 16);
            this.viewportYEnd   = Math.ceil((camera.cameraY + window.innerHeight / this.zoomLevel) / 16);
            this.viewportXStart = Math.max(0, this.viewportXStart);
            this.viewportXEnd   = Math.min(this.worldWidth / 16, this.viewportXEnd);
            this.viewportYStart = Math.max(0, this.viewportYStart);
            this.viewportYEnd   = Math.min(this.worldHeight / 16, this.viewportYEnd);
    
            for (let id in this.sprites) {
                const sprite = this.sprites[id];
                const spriteRight  = sprite.x + sprite.width;
                const spriteBottom = sprite.y + sprite.height;
    
                if (
                    spriteRight >= this.viewportXStart * 16 &&
                    sprite.x    <  this.viewportXEnd   * 16 &&
                    spriteBottom >= this.viewportYStart * 16 &&
                    sprite.y    <  this.viewportYEnd   * 16
                ) {
                    if (sprite.update) {
                        sprite.update();
                    }
                }
            }
    
            camera.update();
            this.updateAnimatedTiles();
    
            this.accumulatedTime -= this.fixedDeltaTime;
        }
    
        this.ctx.imageSmoothingEnabled = false;
        this.ctx.setTransform(1, 0, 0, 1, 0, 0);
        this.ctx.scale(this.zoomLevel, this.zoomLevel);
        this.ctx.translate(-Math.round(camera.cameraX), -Math.round(camera.cameraY));
    
        this.renderBackground();
        this.render();

        plugin.hook('onRender');    
    
        this.renderCarriedObjects();
        this.handleDebugUtilities();
    
        if (plugin.ui_console_editor_inventory.selectedInventoryItem) {
            plugin.ui_console_editor_inventory.render();
        }
    
        plugin.ui_console_tab_window.renderCollisionBoundaries();
        plugin.ui_console_tab_window.renderNearestWalkableTile();
        plugin.ui_console_tab_window.renderObjectCollision();
    
        if (this.mainSprite && this.mainSprite.isVehicle) {
            plugin.ui_overlay_window.update(this.mainSprite.currentSpeed, this.mainSprite.maxSpeed);
        }
    
        plugin.debug.tracker('game.loop');
    
        requestAnimationFrame(this.loop.bind(this));
    },
    

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

    renderBackground: function() {
        this.backgroundTileCount = 0;
        const tileSize = 16;
    
        this.ctx.fillStyle = 'black';
        this.ctx.fillRect(
            this.viewportXStart * tileSize, 
            this.viewportYStart * tileSize, 
            (this.viewportXEnd - this.viewportXStart + 1) * tileSize, 
            (this.viewportYEnd - this.viewportYStart + 1) * tileSize
        );
    
        if (!this.sceneBg) {
            return;
        }
    
        const bgTileData = this.objectData[this.sceneBg][0];
        for (let y = Math.floor(this.viewportYStart); y <= Math.floor(this.viewportYEnd); y++) {
            for (let x = Math.floor(this.viewportXStart); x <= Math.floor(this.viewportXEnd); x++) {
                const posX = x * tileSize;
                const posY = y * tileSize;
    
                const tileFrameIndex = bgTileData.i;
                const srcX = (tileFrameIndex % 150) * tileSize;
                const srcY = Math.floor(tileFrameIndex / 150) * tileSize;
    
                this.ctx.drawImage(
                    assets.use(bgTileData.t),
                    srcX, srcY, tileSize, tileSize,
                    posX, posY, tileSize, tileSize
                );
    
                this.backgroundTileCount++;
                this.tileCount++;
            }
        }
        plugin.debug.tracker('render.renderBackground');
    },

    render: function() {
        this.renderQueue = [];
        this.backgroundTileCount = 0;
        this.tileCount = 0;
        this.spriteCount = 0;
    
        plugin.hook('onRenderAll');
    
        const expandedObjectData = Object.keys(this.objectData).reduce((acc, key) => {
            acc[key] = this.objectData[key].map(this.expandTileData.bind(this));
            return acc;
        }, {});
    
        const itemsToAdd = [];
    
        if (this.roomData && this.roomData.items) {
            this.roomData.items.forEach(roomItem => {
                if (roomItem.visible === false) {
                    return;
                }
    
                const itemData = expandedObjectData[roomItem.id];
                if (!itemData || itemData.length === 0) return;
    
                const tileData = itemData[0];
                const xCoordinates = roomItem.x || [];
                const yCoordinates = roomItem.y || [];
                const bottomYTile = Math.max(...yCoordinates);
                const bottomYPixel = bottomYTile * 16;
    
                let rotation = tileData.rotation || 0;
                if (roomItem.isRotating) {
                    actions.handleRotation(roomItem);
                    rotation = roomItem.rotation;
                }
                if (tileData.sway === true) {
                    rotation += this.handleSway(roomItem);
                }
    
                const objectTiles = {
                    zIndex: 0,
                    type: 'object',
                    id: roomItem.id,
                    visible: true,
                    layer_id: "item_" + roomItem.layer_id,
                    data: {
                        tileData,
                        rotation,
                        tiles: []
                    },
                    draw: () => {
                        this.ctx.save();
                        const centerX = (Math.min(...xCoordinates) + Math.max(...xCoordinates)) / 2;
                        const maxY = Math.max(...yCoordinates);
                        const centerXPixel = centerX * 16;
                        const bottomYPixelVal = maxY * 16;
                        this.ctx.translate(centerXPixel, bottomYPixelVal);
                        this.ctx.rotate(rotation);
    
                        objectTiles.data.tiles.forEach(tile => {
                            this.ctx.drawImage(
                                assets.use(tileData.t),
                                tile.srcX, tile.srcY, 16, 16,
                                tile.posX, tile.posY, 16, 16
                            );
                        });
    
                        this.ctx.restore();
                    }
                };
    
                let index = 0;
                for (let i = 0; i < yCoordinates.length; i++) {
                    const tileY = yCoordinates[i];
                    for (let j = 0; j < xCoordinates.length; j++) {
                        const tileX = xCoordinates[j];
    
                        if (
                            (tileX * 16 + 16) >= (this.viewportXStart * 16) &&
                            (tileX * 16)       <  (this.viewportXEnd   * 16) &&
                            (tileY * 16 + 16) >= (this.viewportYStart * 16) &&
                            (tileY * 16)       <  (this.viewportYEnd   * 16)
                        ) {
                            const centerX = (Math.min(...xCoordinates) + Math.max(...xCoordinates)) / 2;
                            const maxY = Math.max(...yCoordinates);
                            const posX = (tileX - centerX) * 16;
                            const posY = (tileY - maxY) * 16;
    
                            let tileFrameIndex;
                            if (Array.isArray(tileData.i[0])) {
                                const animationData = tileData.i;
                                const currentFrame = tileData.currentFrame || 0;
                                tileFrameIndex = animationData[currentFrame][ index % animationData[currentFrame].length ];
                            } else {
                                tileFrameIndex = tileData.i[index];
                            }
    
                            if (tileFrameIndex !== undefined) {
                                let z = tileData.z;
                                let isZUndefined = false;
                                if (Array.isArray(tileData.z)) {
                                    z = tileData.z[index % tileData.z.length];
                                }
                                if (z === undefined) {
                                    isZUndefined = true;
                                }
    
                                let objectZIndex;
                                if (z === 0 && !isZUndefined) {
                                    objectZIndex = 0;
                                } else if (z === 1) {
                                    objectZIndex = 1;
                                } else if (isZUndefined) {
                                    objectZIndex = bottomYPixel;
                                } else {
                                    objectZIndex = bottomYPixel + z;
                                }
    
                                objectTiles.zIndex = Math.max(objectTiles.zIndex, objectZIndex);
    
                                const srcX = (tileFrameIndex % 150) * 16;
                                const srcY = Math.floor(tileFrameIndex / 150) * 16;
    
                                objectTiles.data.tiles.push({
                                    srcX,
                                    srcY,
                                    posX,
                                    posY,
                                    zIndex: objectZIndex
                                });
                                this.tileCount++;
                            }
                        }
                        index++;
                    }
                }
    
                if (objectTiles.data.tiles.length > 0) {
                    this.renderQueue.push(objectTiles);
                }

                this.currentTileData = tileData;
                this.currentRoomItem = roomItem;
                this.handleLights();
                this.handleEffects();
    
                if (plugin.editor_layers.needsUpdate) {
                    const objectName = tileData.n || "Unnamed Object";
                    itemsToAdd.push({
                        name: objectName,
                        roomItemId: roomItem.id,
                        layer_id: roomItem.layer_id,
                        xCoordinates,
                        yCoordinates,
                    });
                }
            });
        }
    
        for (let id in this.sprites) {
            const sprite = this.sprites[id];
            const spriteRight = sprite.x + sprite.width;
            const spriteBottom = sprite.y + sprite.height;
    
            if (
                spriteRight >= this.viewportXStart * 16 &&
                sprite.x    <  this.viewportXEnd   * 16 &&
                spriteBottom >= this.viewportYStart * 16 &&
                sprite.y    <  this.viewportYEnd   * 16
            ) {
                const spriteZIndex = sprite.y + sprite.height;
    
                this.renderQueue.push({
                    zIndex: spriteZIndex,
                    type: 'sprite',
                    id: id,
                    data: { sprite },
                    draw: () => {
                        this.renderPathfinderLine();
                        sprite.drawShadow();
                        //plugin.effects.dirtCloudEffect.updateAndRender(this.deltaTime);
                        sprite.draw();
                        //plugin.effects.bubbleEffect.updateAndRender(this.deltaTime);
                    }
                });
                this.spriteCount++;
            }
        }
    
        this.renderQueue.sort((a, b) => a.zIndex - b.zIndex);
        this.renderQueue.forEach(item => item.draw());
    
        if (plugin.editor_layers.needsUpdate) {
            itemsToAdd.forEach(itemInfo => {
                plugin.editor_layers.addItemToLayer({
                    n: itemInfo.name,
                    layer_id: itemInfo.layer_id,
                });
            });
            plugin.editor_layers.needsUpdate = false;
        }    
    
        plugin.debug.tracker("render.renderAll");
    },

    handleSway: function(roomItem) { 
        if (!roomItem.swayInitialized) {
            roomItem.swayAngle = Math.PI / (160 + Math.random() * 40);
            roomItem.swaySpeed = 5000 + Math.random() * 2000;
            roomItem.swayInitialized = true;
        }
        if (roomItem.isInViewport) {
            const elapsedTime = roomItem.swayElapsed || 0;
            roomItem.swayElapsed = elapsedTime + this.deltaTime;
            const sway = Math.sin((roomItem.swayElapsed / roomItem.swaySpeed) * Math.PI * 2) * roomItem.swayAngle;
            return sway;
        }
        plugin.debug.tracker('render.handleSway');
        return 0;
    },

    initializeSway: function(roomItem) {
        roomItem.swayAngle = Math.PI / (160 + Math.random() * 40);
        roomItem.swaySpeed = 5000 + Math.random() * 2000;
        roomItem.swayElapsed = 0;
        roomItem.swayInitialized = true;
    },
       
    renderPathfinderLine: function () {
        if (this.mainSprite && this.mainSprite.path && this.mainSprite.path.length > 0) {
            const ctx = this.ctx;
            const lastPoint = this.mainSprite.path[this.mainSprite.path.length - 1];
            const elapsed = Date.now() % 1000;
            const progress1 = (elapsed % 1000) / 1000; 
            const progress2 = ((elapsed + 500) % 1000) / 1000; 
            const ring1Radius = 3 + progress1 * 10; 
            const ring2Radius = 3 + progress2 * 12; 
            const ringOpacity1 = 0.4 - progress1 * 0.4; 
            const ringOpacity2 = 0.4 - progress2 * 0.4; 
            const pixelSize = 2;
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
    
            // Second ring (pixelated)
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
        if (this.mainSprite && this.mainSprite.isCarrying) {
            const carriedItemId = this.mainSprite.carriedItem;
            const itemX = this.mainSprite.x - 8;
            const itemY = this.mainSprite.y - 32 - (this.objectData[carriedItemId][0].b.length);
            this.drawCarriedObject(this.ctx, carriedItemId, itemX, itemY);
        }
    },

    handleDebugUtilities: function () {
        if (typeof debug_window !== 'undefined') {
            if (this.showGrid && debug_window.grid) {
                debug_window.grid();
            }
            if (this.showCollision && debug_window.tiles) {
                debug_window.tiles();
            }
            if (this.showTiles && debug_window.tiles) {
                debug_window.tiles();
            }
        }
    },

    handleLights: function () {
        if (!plugin.exists('time', 'lighting')) return;
        const tileData = this.currentTileData;
        const roomItem = this.currentRoomItem;

        if (!tileData || !roomItem) return;

        const viewportXStart = this.viewportXStart;
        const viewportXEnd   = this.viewportXEnd;
        const viewportYStart = this.viewportYStart;
        const viewportYEnd   = this.viewportYEnd;

        if (tileData.l && Array.isArray(tileData.l)) {
            tileData.l.forEach((light) => {
                if (light.x !== undefined && light.y !== undefined) {
                    const objectTopLeftX = Math.min(...roomItem.x) * 16;
                    const objectTopLeftY = Math.min(...roomItem.y) * 16;
                    const posX = objectTopLeftX + light.x;
                    const posY = objectTopLeftY + light.y;
                    const radius = tileData.lr || 200;

                    const isInView = (
                        (posX + radius) >= (viewportXStart * 16) && 
                        (posX - radius) <  (viewportXEnd * 16) &&
                        (posY + radius) >= (viewportYStart * 16) && 
                        (posY - radius) <  (viewportYEnd * 16)
                    );

                    const lightId = `${roomItem.id}_${Math.round(posX)}_${Math.round(posY)}`;
                    const decimalHours = time.hours + (time.minutes / 60);
                    const isNightTime = (decimalHours >= 22) || (decimalHours < 7);

                    if (isInView && isNightTime) {
                        const existingLight = lighting.lights.find(l => l.id === lightId);
                        if (!existingLight) {
                            const color = tileData.lc || { r: 255, g: 255, b: 255 };
                            const intensity = tileData.li || 1;
                            const flickerSpeed = tileData.lfs || 0.03;
                            const flickerAmount = tileData.lfa || 0.04;
                            const lampType = tileData.lt || "lamp";
                            const shape = light.shape || null;
                            lighting.addLight(
                                lightId, posX, posY, radius, 
                                color, intensity, lampType, true, 
                                flickerSpeed, flickerAmount, shape
                            );
                        }
                    } else {
                        lighting.lights = lighting.lights.filter(l => l.id !== lightId);
                    }
                }
            });
        }
    },

    handleEffects: function () {
        const tileData = this.currentTileData;
        const roomItem = this.currentRoomItem;

        if (!tileData || !roomItem || !this.fxData || !tileData.fx) return;

        const fxData = this.fxData[tileData.fx];
        if (!fxData || !tileData.fxp) return;

        const viewportXStart = this.viewportXStart;
        const viewportXEnd   = this.viewportXEnd;
        const viewportYStart = this.viewportYStart;
        const viewportYEnd   = this.viewportYEnd;

        tileData.fxp.forEach((fxPosition) => {
            const fxXIndex = fxPosition[0];
            const fxYIndex = fxPosition[1];
            if (fxXIndex >= 0 && fxXIndex < roomItem.x.length &&
                fxYIndex >= 0 && fxYIndex < roomItem.y.length) {
                const tileX = roomItem.x[fxXIndex];
                const tileY = roomItem.y[fxYIndex];
                const posX = tileX * 16 + 8;
                const posY = tileY * 16 + 8;
                const isInView = (
                    posX >= (viewportXStart * 16) && 
                    posX <  (viewportXEnd   * 16) &&
                    posY >= (viewportYStart * 16) && 
                    posY <  (viewportYEnd   * 16)
                );
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
    },

    renderBubbles: function(sprite, colorHex) {
        if (!sprite.bubbleEffect) {
            sprite.bubbleEffect = {
                bubbles: [],
                duration: 2000,
                startTime: Date.now(),
            };
        }
        const ctx = this.ctx;
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
            bubble.y += bubble.riseSpeed * this.deltaTime / 16;
            bubble.opacity -= 0.01;
            if (bubble.opacity <= 0 || bubbleY < sprite.y - 40) {
                sprite.bubbleEffect.bubbles.splice(index, 1);
            }
        });
    },

    updateAnimatedTiles: function() {
        if (!this.roomData || !this.roomData.items) return;
        this.roomData.items.forEach(roomItem => {
            const itemData = assets.use('objectData')[roomItem.id];
            if (itemData && itemData.length > 0) {
                if (!roomItem.animationState) {
                    roomItem.animationState = itemData.map(tileData => ({
                        currentFrame: 0,
                        elapsedTime: 0
                    }));
                }
                itemData.forEach((tileData, index) => {
                    if (tileData.i && Array.isArray(tileData.i[0]) && tileData.d) {
                        const animationData = tileData.i;
                        const animationState = roomItem.animationState[index];
                        animationState.elapsedTime += this.deltaTime;
                        if (animationState.elapsedTime >= tileData.d) {
                            animationState.elapsedTime -= tileData.d;
                            animationState.currentFrame =
                                (animationState.currentFrame + 1) % animationData.length;
                        }
                        tileData.currentFrame = animationState.currentFrame;
                    }
                });
            }
        });
    },

    getTileIdAt: function(x, y) {
        if (!game.roomData || !game.roomData.items) {
            return null;
        }
    
        for (const item of game.roomData.items) {
            const xCoordinates = item.x || [];
            const yCoordinates = item.y || [];
    
            if (xCoordinates.includes(x) && yCoordinates.includes(y)) {
                return item.id;
            }
        }
        return null;
    },

    findObjectAt: function(x, y) {
        if (!game.roomData || !game.roomData.items) return null;

        const renderQueue = [];

        game.roomData.items.forEach(roomItem => {
            const itemData = assets.use('objectData')[roomItem.id];
            if (itemData && itemData.length > 0) {
                const tileData = itemData[0];
                const xCoordinates = roomItem.x || [];
                const yCoordinates = roomItem.y || [];

                let index = 0;

                for (let tileY = Math.min(...yCoordinates); tileY <= Math.max(...yCoordinates); tileY++) {
                    for (let tileX = Math.min(...xCoordinates); tileX <= Math.max(...xCoordinates); tileX++) {
                        const posX = tileX * 16;
                        const posY = tileY * 16;

                        let tileFrameIndex;
                        if (tileData.d) {
                            const currentFrame = tileData.currentFrame || 0;
                            tileFrameIndex = Array.isArray(tileData.i)
                                ? tileData.i[(currentFrame + index) % tileData.i.length]
                                : tileData.i;
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

        renderQueue.sort((a, b) => a.z - b.z);

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

    setZoomLevel: function(newZoomLevel) {
        localStorage.setItem('zoomLevel', this.zoomLevel);
    }
};
