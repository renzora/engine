var game = {
    needsFilterUpdate: true,
    canvas: undefined,
    ctx: undefined,
    isDragging: false,
    dragStart: null,
    dragEnd: null,
    dragThreshold: 50,
    isEditMode: false,
    x: null,
    y: null,
    timestamp: 0,
    lastTime: 0,
    deltaTime: 0,
    worldWidth: 1280,
    worldHeight: 1280,
    zoomLevel: localStorage.getItem('zoomLevel') ? parseInt(localStorage.getItem('zoomLevel')) : 6,
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
    edgeScrollSpeed: 30,
    edgeScrollEasing: 0.2,
    edgeScrollBuffer: 150,
    isPaused: false,
    sceneBg: "66ae0ee7bbf2d",
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
        hours: 10,
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
        this.zoomLevel = newZoomLevel;
        localStorage.setItem('zoomLevel', newZoomLevel);
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
            { name: 'objectScript', path: 'json/objectScript.json' },
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

            actions.loadObjectScript();

            const playerOptions = {
                id: this.playerid,
                x: 29,
                y: 23,
                isPlayer: true,
                speed: 70,
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

            const storedSceneId = localStorage.getItem('sceneid') || '66771b7e6c1c5b2f1708b75a';
            this.loadScene(storedSceneId);

            modal.load('ui/footer.php', "ui_footer_window", "Footer", false);
            modal.load('menus/click_menu/index.php', 'click_menu_window', "click menu", false);
            modal.load('menus/pie/index.php', 'pie_menu_window', "pie menu", false);
            modal.load('console', null, "console", true);
            modal.load('ui/inventory.php', "ui_inventory_window", "ui window", false);
            //modal.load('ui/objectives.php', "ui_objectives_window", "Objectives", false);
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
        input.cancelPathfinding(game.sprites[game.playerid]);
        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'modals/console/tabs/servers/ajax/getSceneData.php',
            data: 'scene_id=' + encodeURIComponent(sceneId),
            success: function(data) {
                if (data.message === 'success') {
                    effects.lights = [];
                    game.roomData = data.roomData;
                    game.sceneid = data.sceneid;
                    this.overlappingTiles = [];
                    camera.update();
                    localStorage.setItem('sceneid', game.sceneid);
                    game.selectedObjects = [];
                    game.selectedCache = [];
                    effects.transitions.start('fadeOut', 1000);
                    effects.transitions.start('fadeIn', 1000);
                    ui.notif("scene_change_notif", data.name, true);
                    audio.stopLoopingAudio('music', 0.5);
                    //audio.playAudio("music1", assets.load('music1'), 'music', true);
                } else {
                    console.log('Error: ' + data.message);
                    modal.load('console/tabs/servers/ajax/error.php', 'scene_load_error_window', null, "server error", true);
                }
            },
            error: function(data) {
                console.log(data);
                modal.load('console/tabs/servers/ajax/error.php', 'scene_load_error_window', "server error", true);
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
            this.isDragging = true;
            this.dragStart = { x: mouseX, y: mouseY };
            this.dragEnd = null;
            this.selectedTiles = [];
            this.selectionBounds = null;
    
            document.body.style.userSelect = 'none';
            document.body.style.webkitUserSelect = 'none'; /* Safari */
            document.body.style.msUserSelect = 'none'; /* IE 10 and IE 11 */
            camera.activeCamera = false;
            this.currentMouseX = event.clientX;
            this.currentMouseY = event.clientY;
            this.isEdgeScrolling = false;
        }
    },

    handleMouseMove: function(event) {
        if (this.isEditorActive || (this.mainSprite && this.mainSprite.targetAim)) return;
        if (this.isDragging) {
            const rect = this.canvas.getBoundingClientRect();
            const mouseX = (event.clientX - rect.left) / this.zoomLevel + camera.cameraX;
            const mouseY = (event.clientY - rect.top) / this.zoomLevel + camera.cameraY;
            this.dragEnd = { x: mouseX, y: mouseY };
            
            const deltaX = Math.abs(this.dragEnd.x - this.dragStart.x);
            const deltaY = Math.abs(this.dragEnd.y - this.dragStart.y);
    
            if(deltaX >= 8 || deltaY >= 8) {
                this.updateSelectedTiles();
                this.currentMouseX = event.clientX;
                this.currentMouseY = event.clientY;
        
                if(!this.isEdgeScrolling && (this.dragStart.x !== this.dragEnd.x || this.dragStart.y !== this.dragEnd.y)) {
                    this.isEdgeScrolling = true;
                    this.edgeScroll();
                }
            }
        }
    },
    
    handleMouseUp: function(event) {
        if (this.isEditorActive || (this.mainSprite && this.mainSprite.targetAim)) return;
        console.log('Game handleMouseUp triggered');
        const rect = this.canvas.getBoundingClientRect();
        const mouseX = (event.clientX - rect.left) / this.zoomLevel + camera.cameraX;
        const mouseY = (event.clientY - rect.top) / this.zoomLevel + camera.cameraY;
        this.isDragging = false;
        this.dragEnd = { x: mouseX, y: mouseY };
    
        const deltaX = Math.abs(this.dragEnd.x - this.dragStart.x);
        const deltaY = Math.abs(this.dragEnd.y - this.dragStart.y);
    
        if (deltaX < this.dragThreshold && deltaY < this.dragThreshold) {
            this.handleCanvasClick(event, event.shiftKey);
        } else if (deltaX >= 8 || deltaY >= 8) {
            this.handleCanvasDrag({ startX: this.dragStart.x, startY: this.dragStart.y, endX: this.dragEnd.x, endY: this.dragEnd.y }, event.shiftKey);
            if(this.selectedObjects.length > 0) {
                setTimeout(() => {
                    click_menu_window.showContextMenu(event.clientX, event.clientY, true);
                }, 0);
            }
        }
    
        document.body.style.userSelect = '';
        document.body.style.webkitUserSelect = ''; /* Safari */
        document.body.style.msUserSelect = ''; /* IE 10 and IE 11 */
        camera.activeCamera = true;
        this.isEdgeScrolling = false;
        this.selectedTiles = [];
        this.selectionBounds = null;
    },
    
    edgeScroll: function() {
        if (!this.isDragging || !this.isEdgeScrolling) {
            return;
        }
    
        const edgeThreshold = this.edgeScrollBuffer;
        const easing = this.edgeScrollEasing;
        const maxSpeed = this.edgeScrollSpeed;
        let mouseX = this.currentMouseX;
        let mouseY = this.currentMouseY;
        let scrollX = 0;
        let scrollY = 0;
    
        if (mouseX < edgeThreshold) {
            scrollX = -maxSpeed * (1 - (mouseX / edgeThreshold)) * easing;
        } else if (mouseX > window.innerWidth - edgeThreshold) {
            scrollX = maxSpeed * (1 - ((window.innerWidth - mouseX) / edgeThreshold)) * easing;
        }
    
        if (mouseY < edgeThreshold) {
            scrollY = -maxSpeed * (1 - (mouseY / edgeThreshold)) * easing;
        } else if (mouseY > window.innerHeight - edgeThreshold) {
            scrollY = maxSpeed * (1 - ((window.innerHeight - mouseY) / edgeThreshold)) * easing;
        }

        camera.cameraX = Math.max(0, Math.min(camera.cameraX + scrollX, this.worldWidth - window.innerWidth / this.zoomLevel));
        camera.cameraY = Math.max(0, Math.min(camera.cameraY + scrollY, this.worldHeight - window.innerHeight / this.zoomLevel));

        requestAnimationFrame(this.edgeScroll.bind(this));
    },

    handleCanvasClick: function(event, isShiftKey) {
        console.log('Game handleCanvasClick triggered');

        console_window.toggleConsoleWindow();
        
        const rect = this.canvas.getBoundingClientRect();
        const mouseX = (event.clientX - rect.left) / this.zoomLevel + camera.cameraX;
        const mouseY = (event.clientY - rect.top) / this.zoomLevel + camera.cameraY;
        const gridX = Math.floor(mouseX / 16);
        const gridY = Math.floor(mouseY / 16);
        this.x = gridX;
        this.y = gridY;
        
        const selectedObject = this.findObjectAt(mouseX, mouseY);
        
        if (selectedObject) {
            console.log(`Selected object ID: ${selectedObject.id}`);
            if (isShiftKey) {
                const uniqueId = `${selectedObject.id}_${selectedObject.x}_${selectedObject.y}`;
                const index = this.selectedObjects.findIndex(obj => `${obj.id}_${obj.x}_${obj.y}` === uniqueId);
                if (index === -1) {
                    this.selectedObjects.push(selectedObject);
                } else {
                    this.selectedObjects.splice(index, 1);
                }
    
                if (!this.selectedCache.some(cache => cache.id === selectedObject.id)) {
                    this.selectedCache.push({ id: selectedObject.id, image: this.drawAndOutlineObjectImage(selectedObject) });
                }
            } else {
                this.selectedObjects = [selectedObject];
                if (!this.selectedCache.some(cache => cache.id === selectedObject.id)) {
                    this.selectedCache.push({ id: selectedObject.id, image: this.drawAndOutlineObjectImage(selectedObject) });
                }
            }
        }
    
        const spriteGridX = Math.floor(this.mainSprite.x / 16);
        const spriteGridY = Math.floor(this.mainSprite.y / 16);
        const distanceX = Math.abs(gridX - spriteGridX);
        const distanceY = Math.abs(gridY - spriteGridY);

        if (distanceX <= 2 && distanceY <= 2) {
            const deltaX = gridX - spriteGridX;
            const deltaY = gridY - spriteGridY;
    
            if (Math.abs(deltaX) > Math.abs(deltaY)) {
                if (deltaX > 0) {
                    this.mainSprite.direction = 'E';
                } else {
                    this.mainSprite.direction = 'W';
                }
            } else {
                if (deltaY > 0) {
                    this.mainSprite.direction = 'S';
                } else {
                    this.mainSprite.direction = 'N';
                }
            }
        }
    
        if(this.isTileWalkable(gridX, gridY)) {
            this.selectedObjects = [];
            this.selectedCache = [];
            this.updateSelectedTiles();
            this.render();
            this.mainSprite.walkToClickedTile(gridX, gridY);
            console.log('Tile is walkable, no context menu should be shown.');
        } else if (this.selectedObjects.length > 0) {
            setTimeout(() => {
                click_menu_window.showContextMenu(event.clientX, event.clientY, true);
            }, 0);
        }
    
        console.log('Current selected objects:', this.selectedObjects);
        this.updateSelectedTiles();
    },
    

    isTileWalkable: function(gridX, gridY) {
        const grid = this.createWalkableGrid();
        return grid[gridX] && grid[gridX][gridY] === 1;
    },

    handleCanvasDrag: function(dragArea, isShiftKey) {
        console.log('Game handleCanvasDrag triggered');
        this.updateSelectedTiles();
        this.selectItemsInSelectedTiles(isShiftKey);
        this.selectedObjects.forEach(selectedObject => {
            const cachedObject = this.selectedCache.find(cache => cache.id === selectedObject.id);
            if (!cachedObject) {
                this.selectedCache.push({ id: selectedObject.id, image: this.drawAndOutlineObjectImage(selectedObject) });
            }
        });
    
        if (this.selectedObjects.length > 0) {
            setTimeout(() => {
                click_menu_window.showContextMenu(event.clientX, event.clientY, true);
            }, 0);
        }
    },

    createWalkableGrid: function() {
        const width = this.worldWidth / 16;
        const height = this.worldHeight / 16;
        const grid = Array.from({ length: width }, () => Array(height).fill(1));
    
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
                                    const [north, east, south, west] = walkableData;
                                    if (north < 16 || east < 16 || south < 16 || west < 16) {
                                        grid[tileX][tileY] = 0;
                                    }
                                } else if (walkableData === 0) {
                                    grid[tileX][tileY] = 0;
                                } else if (walkableData === 1 && grid[tileX][tileY] !== 0) {
                                    grid[tileX][tileY] = 1;
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

    updateSelectedTiles: function() {
        if (this.dragStart && this.dragEnd) {
            const deltaX = Math.abs(this.dragEnd.x - this.dragStart.x);
            const deltaY = Math.abs(this.dragEnd.y - this.dragStart.y);

            if (deltaX >= 8 || deltaY >= 8) {
                const startX = Math.min(this.dragStart.x, this.dragEnd.x);
                const startY = Math.min(this.dragStart.y, this.dragEnd.y);
                const endX = Math.max(this.dragStart.x, this.dragEnd.x);
                const endY = Math.max(this.dragStart.y, this.dragEnd.y);

                const startTileX = Math.floor(startX / 16);
                const startTileY = Math.floor(startY / 16);
                const endTileX = Math.floor(endX / 16);
                const endTileY = Math.floor(endY / 16);

                this.selectedTiles = [];

                for (let x = startTileX; x <= endTileX; x++) {
                    for (let y = startTileY; y <= endTileY; y++) {
                        this.selectedTiles.push({ x: x * 16, y: y * 16 });
                    }
                }

                this.selectionBounds = {
                    startX: startTileX * 16,
                    startY: startTileY * 16,
                    endX: (endTileX + 1) * 16,
                    endY: (endTileY + 1) * 16
                };
            }
        }
    },

    selectItemsInSelectedTiles: function(isShiftKey) {
        const foundItems = [];
    
        if (this.roomData && this.roomData.items) {
            this.roomData.items.forEach(roomItem => {
                const itemData = this.objectData[roomItem.id];
                if (itemData && itemData.length > 0) {
                    const tileData = itemData[0];
                    const xCoordinates = roomItem.x || [];
                    const yCoordinates = roomItem.y || [];
    
                    for (let y = Math.min(...yCoordinates); y <= Math.max(...yCoordinates); y++) {
                        for (let x = Math.min(...xCoordinates); x <= Math.max(...xCoordinates); x++) {
                            if (this.isTileSelected(x * 16, y * 16)) {
                                foundItems.push(roomItem);
                                break;
                            }
                        }
                    }
                }
            });
        }
    
        if (isShiftKey) {
            foundItems.forEach(foundItem => {
                const uniqueId = `${foundItem.id}_${foundItem.x}_${foundItem.y}`;
                const index = this.selectedObjects.findIndex(obj => `${obj.id}_${obj.x}_${obj.y}` === uniqueId);
                if (index === -1) {
                    this.selectedObjects.push(foundItem);
                } else {
                    this.selectedObjects.splice(index, 1);
                }
            });
        } else {
            this.selectedObjects = foundItems;
        }
    },

    isTileSelected: function(tileX, tileY) {
        return this.selectedTiles.some(tile => tile.x === tileX && tile.y === tileY);
    },

    renderSelectedTiles: function() {
        if (this.selectedTiles.length > 0) {
            const { startX, startY, endX, endY } = this.selectionBounds;
            this.ctx.strokeStyle = 'white';
            this.ctx.lineWidth = 8 / this.zoomLevel;
            this.ctx.setLineDash([10, 10]);
            this.ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
            this.ctx.shadowBlur = 10;
            this.ctx.shadowOffsetX = 4;
            this.ctx.shadowOffsetY = 4;
            this.ctx.lineDashOffset -= 0.5;
            this.ctx.strokeRect(startX, startY, endX - startX, endY - startY);
            this.ctx.setLineDash([]);
            this.ctx.shadowColor = 'transparent';
            this.ctx.shadowBlur = 0;
            this.ctx.shadowOffsetX = 0;
            this.ctx.shadowOffsetY = 0;
        }
    },

    drawCarriedObject: function(ctx, carriedItemId, baseX, baseY) {
        const itemData = game.objectData[carriedItemId];
        if (!itemData || itemData.length === 0) {
            console.error("Invalid item data for carried item ID:", carriedItemId);
            return;
        }
    
        const tileData = itemData[0];
    
        const tileIndices = tileData.i;
        const xCoordinates = tileData.a || [];
        const yCoordinates = tileData.b || [];
        const tileWidth = 16;
        const tileHeight = 16;
        const imageSource = assets.load(tileData.t);
        const tilesPerRow = 150;
    
        if (xCoordinates.length === 0 || yCoordinates.length === 0) {
            console.error("Invalid coordinates for carried item ID:", carriedItemId);
            return;
        }
    
        let index = 0;
    
        for (let y = Math.min(...yCoordinates); y <= Math.max(...yCoordinates); y++) {
            for (let x = Math.min(...xCoordinates); x <= Math.max(...xCoordinates); x++) {
                const itemX = x * tileWidth + baseX;
                const itemY = y * tileHeight + baseY;
    
                let tileFrameIndex;
                if (Array.isArray(tileIndices[0])) {
                    const animationData = tileIndices;
                    const currentFrame = Math.floor(Date.now() / 100) % animationData.length;
                    tileFrameIndex = animationData[currentFrame][index % animationData[currentFrame].length];
                } else {
                    tileFrameIndex = tileIndices[index];
                }
    
                if (tileFrameIndex !== undefined) {
                    const srcX = (tileFrameIndex % tilesPerRow) * tileWidth;
                    const srcY = Math.floor(tileFrameIndex / tilesPerRow) * tileHeight;
    
                    ctx.drawImage(imageSource, srcX, srcY, tileWidth, tileHeight, itemX, itemY, tileWidth, tileHeight);
                }
    
                index++;
            }
        }
    },    
    
 
    render: function () {
        this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
        this.ctx.setTransform(1, 0, 0, 1, 0, 0);
        this.ctx.scale(this.zoomLevel, this.zoomLevel);
        this.ctx.translate(-Math.round(camera.cameraX), -Math.round(camera.cameraY));
        let tileCount = 0;
        let spriteCount = 0;
        this.viewportXStart = Math.max(0, Math.floor(camera.cameraX / 16));
        this.viewportXEnd = Math.min(this.worldWidth / 16, Math.ceil((camera.cameraX + window.innerWidth / this.zoomLevel) / 16));
        this.viewportYStart = Math.max(0, Math.floor(camera.cameraY / 16));
        this.viewportYEnd = Math.min(this.worldHeight / 16, Math.ceil((camera.cameraY + window.innerHeight / this.zoomLevel) / 16));

        tileCount = render.renderBackground(this.viewportXStart, this.viewportXEnd, this.viewportYStart, this.viewportYEnd);

        const { itemTileCount } = render.renderRoomItems(this.viewportXStart, this.viewportXEnd, this.viewportYStart, this.viewportYEnd);
        tileCount += itemTileCount;

        spriteCount = render.renderSprites(this.viewportXStart, this.viewportXEnd, this.viewportYStart, this.viewportYEnd);
    
        render.renderPathfinderLine();
        render.renderCarriedObjects();
        render.renderSelectedTiles();
        render.renderLightingEffects();
        render.renderWeatherEffects();
        render.handleDebugUtilities();
        particles.renderParticles();
        effects.transitions.render();
        render.updateUI(tileCount, spriteCount);
        render.highlightOverlappingTiles();
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