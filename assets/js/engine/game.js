var game = {
    needsFilterUpdate: true,
    canvas: undefined,
    ctx: undefined,
    isDragging: false,
    isEditMode: false,
    x: null,
    y: null,
    timestamp: 0,
    lastTime: 0,
    deltaTime: 0,
    worldWidth: 1280,
    worldHeight: 1280,
    zoomLevel: localStorage.getItem('zoomLevel') ? parseInt(localStorage.getItem('zoomLevel')) : 4,
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
    fixedDeltaTime: 1000 / 60,
    accumulatedTime: 0,
    lastTime: null,
    maxAccumulatedTime: 1000,
    displayUsernames: false,
    displayChat: false,
    displaySprite: true,
    allowControls: true,
    selectedObjects: [],
    selectedCache: [],
    pathfinding: true,
    selectedTiles: [],
    particles: [],
    overlappingTiles: [],
    isPaused: false,
    sceneBg: "grass",
    isEditorActive: false,
    editorMode: null,
    selectionBounds: null,
    tooltips: [],
    activeSpriteId: null,
    inputMethod: 'keyboard',
    objectives: [
        { name: "Find the hidden sword", status: false },
        { name: "Plant the apple seeds in renzora Garden", status: false },
        { name: "Sell gold at oakenbridge Market", status: false },
        { name: "Find the hidden sword", status: true },
        { name: "Find the hidden sword", status: true },
        { name: "Defeat the dragon", status: true },
        { name: "Collect 100 coins from merchant", status: false }
    ],
    gameTime: {
        hours: 21,
        minutes: 0,
        seconds: 0,
        days: 0,
        speedMultiplier: 100,
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
        const assetsToReload = ['objectData', 'roomData'];

        assets.reloadAssets(assetsToReload, () => {
            console.log("Game data reloaded");
            this.roomData = assets.load('roomData');
            this.updateGameElements();
        });
    },

    updateInputMethod: function(method, name = '') {
        const inputMethodDisplay = document.getElementById('input_method');
        if (inputMethodDisplay) {
            inputMethodDisplay.innerText = `Input: ${method}${name ? ' (' + name + ')' : ''}`;
        }
    },

    updateGameElements: function() {
        console.log("Game elements updated");
    },

    setZoomLevel: function(newZoomLevel) {
        this.zoomLevel = Math.max(2, Math.min(newZoomLevel, 10)); 
        localStorage.setItem('zoomLevel', game.zoomLevel);
        console.log('setting zoom level');
    },

    init: function() {
        this.playerid = network.getToken('renaccount') || `player_${Math.floor(Math.random() * 10000)}`;

        assets.preload([
            { name: 'head', path: 'img/sprites/character/test/head.png' },
            { name: 'eyes', path: 'img/sprites/character/test/eyes.png' },
            { name: 'hair', path: 'img/sprites/character/test/hair.png' },
            { name: 'hands', path: 'img/sprites/character/test/hands.png' },
            { name: 'hats', path: 'img/sprites/character/test/hats.png' },
            { name: 'glasses', path: 'img/sprites/character/test/glasses.png' },
            { name: 'facial', path: 'img/sprites/character/test/facial.png' },
            { name: 'outfit', path: 'img/sprites/character/test/outfit.png' },
            { name: 'gen1', path: 'img/tiles/gen1.png' },
            { name: 'itemsImg', path: 'img/icons/items.png' },
            { name: 'objectData', path: 'json/objectData.json' },
            { name: 'itemsData', path: 'json/itemsData.json' },
            { name: 'fxData', path: 'json/fxData.json' },
            { name: 'walkGrass', path: 'audio/sfx/movement/footstep.wav' },
            { name: 'closeModal', path: 'audio/sfx/ui/closeModal.mp3' },
            { name: 'menuDrop', path: 'audio/sfx/ui/menuDrop.mp3' },
            { name: 'objectDrop', path: 'audio/sfx/ui/dropObject.mp3' },
            { name: 'notification', path: 'audio/sfx/ui/notification.mp3' },
            { name: 'music1', path: 'audio/music/music1.mp3' },
            { name: 'bump1', path: 'audio/sfx/movement/bump.mp3' },
            { name: 'click', path: 'audio/sfx/ui/click.mp3' },
            { name: 'slotDrop', path: 'audio/sfx/ui/slotDrop.mp3' },
            { name: 'sceneDrop', path: 'audio/sfx/ui/sceneDrop.mp3' },
            { name: 'itemEquip', path: 'audio/sfx/ui/itemEquip.mp3' },
            { name: 'nightAmbience', path: 'audio/sfx/weather/nightAmbience.mp3' },
            { name: 'rain', path: 'audio/sfx/weather/rain.mp3' },
            { name: 'meta', path: 'json/meta.json' },
            { name: 'machinegun1', path: 'audio/sfx/fx/machineGun.mp3' },
            { name: 'reload_gun', path: 'audio/sfx/fx/reload_gun.mp3' },
            { name: 'empty_gun', path: 'audio/sfx/fx/empty_gun.mp3' },
        ], () => {
            console.log("All assets loaded");
            this.canvas = document.createElement('canvas');
            this.ctx = this.canvas.getContext('2d');
            document.body.appendChild(this.canvas);
            this.resizeCanvas();
            this.itemsImg = assets.load('itemsImg');
            this.itemsData = assets.load('itemsData');
            this.objectData = assets.load('objectData');
            this.fxData = assets.load('fxData');
            
            const playerOptions = {
                id: this.playerid,
                x: 29,
                y: 23,
                isPlayer: true,
                speed: 85,
                head: 1,
                eyes: 1,
                body: 1,
                hair: 1,
                outfit: 1,
                hands: 1,
                hat: 0,
                facial: 0,
                glasses: 0,
                targetAim: false,
                maxRange: 200
            };
            sprite.create(playerOptions);

            this.mainSprite = game.sprites[this.playerid];
            this.setActiveSprite(this.playerid);



            weather.createFireflys();
            weather.createRain(0.7);
            weather.createSnow(0.2);

            const storedSceneId = localStorage.getItem('sceneid') || '66afd5880a19134bf10959d3';
            this.loadScene(storedSceneId);

            //modal.load({ id: 'ui_footer_window', url: 'ui/footer.php', name: 'Footer', drag: false, reload: false });
            modal.load({ id: 'console_window', url: 'menus/console', name: 'console', drag: false, reload: true });
            modal.load({ id: 'click_menu_window', url: 'menus/click_menu/index.php', name: 'click menu', drag: true, reload: false });
            modal.load({ id: 'pie_menu_window', url: 'menus/pie/index.php', name: 'pie menu', drag: false, reload: false });
            modal.load({ id: 'ui_inventory_window', url: 'ui/inventory.php', name: 'ui window', drag: false, reload: false });
            modal.load({ id: 'ui_overlay_window', url: 'ui/overlay.php', name: 'overlay', drag: false, reload: false });
            //modal.load({ id: 'ui_objectives_window', url: 'ui/objectives.php', name: 'Objectives', drag: false, reload: false });

            console.log("Connected to Main renzora server");
            //modal.load('inventory_items', "inventory_items_window", "Inventory Items", true);

            this.loop();

            if (this.mainSprite) {
                this.mainSprite.updateHealth(0);
                this.mainSprite.updateHealth(this.mainSprite.health);  
                this.mainSprite.updateEnergy(this.mainSprite.energy);  
            }

            // Send initial player state to the server
            network.send({
                command: 'playerStateUpdate',
                data: {
                    id: this.playerid,
                    ...this.sprites[this.playerid]
                }
            });

        window.reloadGameData = this.reloadGameData.bind(this);
        this.canvas.addEventListener('mousedown', this.handleMouseDown.bind(this));
        this.canvas.addEventListener('mousemove', this.handleMouseMove.bind(this));
        this.canvas.addEventListener('mouseup', this.handleMouseUp.bind(this));
        this.canvas.addEventListener('contextmenu', (e) => e.preventDefault());

        document.addEventListener('visibilitychange', () => {
            if (document.hidden) {
                this.pause();
            } else {
                this.resume();
            }
        });

        });
    },

    pause: function() {
        cancelAnimationFrame(this.animationFrameId);
        audio.pauseAll();
        this.isPaused = true;
    },
    
    resume: function() {
        network.send({
            command: 'requestGameState',
            playerId: this.playerid
        });
        audio.resumeAll();
    },

    setActiveSprite: function(spriteId) {
        if (this.sprites[spriteId]) {
            this.activeSpriteId = spriteId;
        } else {
            console.error(`Sprite with ID ${spriteId} does not exist.`);
        }
    },

    loadScene: function(sceneId) {
        lighting.clearLightsAndEffects();
        input.cancelPathfinding(game.sprites[game.playerid]);
        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'modals/menus/console/tabs/servers/ajax/getSceneData.php',
            data: 'scene_id=' + encodeURIComponent(sceneId),
            success: function(data) {
                if (data.message === 'success') {
                    effects.lights = [];
                    game.roomData = data.roomData;
                    game.sceneid = data.sceneid;
                    game.serverid = data.server_id; // Store the server_id for later use

                    this.overlappingTiles = [];
                    camera.update();
                    localStorage.setItem('sceneid', game.sceneid);
                    game.selectedObjects = [];
                    game.selectedCache = [];
                    effects.transitions.start('fadeOut', 1000);
                    effects.transitions.start('fadeIn', 1000);
                    ui.notif("scene_change_notif", data.name, true);
                    audio.stopLoopingAudio('music', 0.5);
    
                    // Recalculate the walkable grid for the new scene
                    collision.createWalkableGrid();
    
                } else {
                    console.log('Error: ' + data.message);
                    modal.load('menus/console/tabs/servers/ajax/error.php', 'scene_load_error_window', null, "server error", true);
                }
            },
            error: function(data) {
                console.log(data);
                modal.load('menus/console/tabs/servers/ajax/error.php', 'scene_load_error_window', "server error", true);
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

    getTileIdAt: function(x, y) {
        if (!this.roomData || !this.roomData.items) {
            return null;
        }
    
        for (const item of this.roomData.items) {
            const xCoordinates = item.x || [];
            const yCoordinates = item.y || [];
    
            if (xCoordinates.includes(x) && yCoordinates.includes(y)) {
                return item.id;
            }
        }
        return null;
    },

    handleMouseDown: function(event) {
        if (this.isEditorActive || (this.mainSprite && this.mainSprite.targetAim)) return;
        console.log('Game handleMouseDown triggered');
        if (event.button === 0 || event.button === 2) {
            const rect = this.canvas.getBoundingClientRect();
            const mouseX = (event.clientX - rect.left) / this.zoomLevel + camera.cameraX;
            const mouseY = (event.clientY - rect.top) / this.zoomLevel + camera.cameraY;
            this.x = Math.floor(mouseX / 16);
            this.y = Math.floor(mouseY / 16);
        }
    },

    handleMouseMove: function(event) {
        if (this.isEditorActive || (this.mainSprite && this.mainSprite.targetAim)) return;
    },
    
    handleMouseUp: function(event) {
        if (this.isEditorActive || (this.mainSprite && this.mainSprite.targetAim)) return;
        console.log('Game handleMouseUp triggered');
    
        // Check if the console window or another menu is active
        if (console_window.isMenuActive()) {
            console.log('Menu is active, preventing canvas click actions.');
            console_window.toggleConsoleWindow();
            return;
        }
    
        const rect = this.canvas.getBoundingClientRect();
        const mouseX = (event.clientX - rect.left) / this.zoomLevel + camera.cameraX;
        const mouseY = (event.clientY - rect.top) / this.zoomLevel + camera.cameraY;
        this.x = Math.floor(mouseX / 16);
        this.y = Math.floor(mouseY / 16);
        
        const selectedObject = this.findObjectAt(mouseX, mouseY);
        
        if (selectedObject) {
            console.log(`Selected object ID: ${selectedObject.id}`);
            this.selectedObjects = [selectedObject];
            if (!this.selectedCache.some(cache => cache.id === selectedObject.id)) {
                this.selectedCache.push({ id: selectedObject.id, image: this.drawAndOutlineObjectImage(selectedObject) });
            }
        }
    
        if(collision.isTileWalkable(this.x, this.y)) {
            this.selectedObjects = [];
            this.selectedCache = [];
            this.render();
            this.mainSprite.walkToClickedTile(this.x, this.y);
            console.log('Tile is walkable, no context menu should be shown.');
        } else if (this.selectedObjects.length > 0) {
            setTimeout(() => {
                click_menu_window.showContextMenu(event.clientX, event.clientY, true);
            }, 0);
        }
    
        console.log('Current selected objects:', this.selectedObjects);
    },  

    findObjectAt: function(x, y) {
        if (!this.roomData || !this.roomData.items) return null;

        const renderQueue = [];

        this.roomData.items.forEach(roomItem => {
            const itemData = assets.load('objectData')[roomItem.id];
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

        renderQueue.sort((a, b) => a.z - b.z || a.renderOrder - b.renderOrder);

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

        const cachedObject = this.selectedCache.find(cache => cache.id === object.id);
        if (cachedObject) {
            return cachedObject.image;
        }

        const itemData = assets.load('objectData')[object.id];
        if (!itemData) return null;

        const xCoordinates = object.x.map(x => parseInt(x, 10) * 16);
        const yCoordinates = object.y.map(y => parseInt(y, 10) * 16);

        const minX = Math.min(...xCoordinates);
        const maxX = Math.max(...xCoordinates) + 16;
        const minY = Math.min(...yCoordinates);
        const maxY = Math.max(...yCoordinates) + 16;

        const offscreenCanvas = document.createElement('canvas');
        const offscreenCtx = offscreenCanvas.getContext('2d');
        offscreenCanvas.width = maxX - minX;
        offscreenCanvas.height = maxY - minY;

        for (let i = 0; i < xCoordinates.length; i++) {
            for (let j = 0; j < yCoordinates.length; j++) {
                const itemX = xCoordinates[i] - minX;
                const itemY = yCoordinates[j] - minY;

                const tileIndex = j * xCoordinates.length + i;
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
        const imageData = offscreenCtx.getImageData(0, 0, width, height);
        const data = imageData.data;
        const outlineData = new Uint8ClampedArray(data);
        const outlinePixel = (x, y) => {
            const index = (y * width + x) * 4;
            outlineData[index] = 255;
            outlineData[index + 1] = 255;
            outlineData[index + 2] = 255;
            outlineData[index + 3] = 255;
        };

        for (let y = 0; y < height; y++) {
            for (let x = 0; x < width; x++) {
                const index = (y * width + x) * 4;
                if (data[index + 3] === 0) {
                    const neighbors = [
                        { x: x - 1, y: y },
                        { x: x + 1, y: y },
                        { x: x, y: y - 1 },
                        { x: x, y: y + 1 },
                        { x: x - 1, y: y - 1 },
                        { x: x + 1, y: y - 1 },
                        { x: x - 1, y: y + 1 },
                        { x: x + 1, y: y + 1 }
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

        for (let x = 0; x < width; x++) {
            if (data[x * 4 + 3] !== 0) outlinePixel(x, 0);
            if (data[((height - 1) * width + x) * 4 + 3] !== 0) outlinePixel(x, height - 1);
        }
        for (let y = 0; y < height; y++) {
            if (data[(y * width) * 4 + 3] !== 0) outlinePixel(0, y);
            if (data[(y * width + (width - 1)) * 4 + 3] !== 0) outlinePixel(width - 1, y);
        }

        const outlinedImageData = new ImageData(outlineData, width, height);
        offscreenCtx.putImageData(outlinedImageData, 0, 0);
        this.selectedCache.push({ id: object.id, image: offscreenCanvas });

        return offscreenCanvas;
    },

    render: function () {
        this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
        this.ctx.setTransform(1, 0, 0, 1, 0, 0);
        this.ctx.scale(this.zoomLevel, this.zoomLevel);
        this.ctx.translate(-Math.round(camera.cameraX), -Math.round(camera.cameraY));
        
        this.viewportXStart = Math.max(0, Math.floor(camera.cameraX / 16));
        this.viewportXEnd = Math.min(this.worldWidth / 16, Math.ceil((camera.cameraX + window.innerWidth / this.zoomLevel) / 16));
        this.viewportYStart = Math.max(0, Math.floor(camera.cameraY / 16));
        this.viewportYEnd = Math.min(this.worldHeight / 16, Math.ceil((camera.cameraY + window.innerHeight / this.zoomLevel) / 16));
    
        const { backgroundTileCount, tileCount, spriteCount } = render.renderAll(this.viewportXStart, this.viewportXEnd, this.viewportYStart, this.viewportYEnd);
    
        render.renderPathfinderLine();
        render.renderCarriedObjects();
        render.renderLightingEffects();
        render.renderWeatherEffects();
        render.handleDebugUtilities();
        particles.renderParticles();
        effects.transitions.render();
    
        render.updateUI(backgroundTileCount + tileCount, spriteCount);
        render.highlightOverlappingTiles();
    
        if (typeof debug_utils_window !== 'undefined') {
            if (debug_utils_window.showCollisionBoundaries && typeof debug_utils_window.renderCollisionBoundaries === 'function') {
                debug_utils_window.renderCollisionBoundaries();
            }
            if (debug_utils_window.showWalkableTiles && typeof debug_utils_window.renderNearestWalkableTile === 'function') {
                debug_utils_window.renderNearestWalkableTile();
            }
            if (debug_utils_window.showObjectCollision && typeof debug_utils_window.renderObjectCollision === 'function') {
                debug_utils_window.renderObjectCollision();
            }
        }
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
            render.updateGameLogic(this.fixedDeltaTime);
            this.accumulatedTime -= this.fixedDeltaTime;
        }
    
        this.render();
    
        const fpsUpdateInterval = 100;
        if (timestamp - this.lastFpsUpdateTime >= fpsUpdateInterval) {
            var debugFPS = document.getElementById('gameFps');
            var fps = 1000 / timeElapsed;
            if (debugFPS) {
                debugFPS.innerHTML = "FPS: " + fps.toFixed(2);
            }
            this.lastFpsUpdateTime = timestamp;
        }
    
        var gameTimeDisplay = document.getElementById('game_time');
        if (gameTimeDisplay) {
            gameTimeDisplay.innerHTML = this.gameTime.display();
        }
    
        requestAnimationFrame(this.loop.bind(this));
    }
};
