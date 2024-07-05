var game = {
    lerpFactor: parseFloat(localStorage.getItem('lerpFactor')) || 0.1,
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
    worldWidth: 2560,
    worldHeight: 2560,
    zoomLevel: 4,
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
    selectedObjects: [],
    selectedCache: [],
    pathfinding: true,
    selectedTiles: [],
    particles: [],
    overlappingTiles: [],
    edgeScrollSpeed: 30, // Increased speed for more noticeable movement
    edgeScrollEasing: 0.2, // Slightly larger easing factor for smoother control
    edgeScrollBuffer: 150,
    isPaused: false,
    sceneBg: "grass",
    isEditorActive: false,
    selectionBounds: null,
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
        hours: 7,
        minutes: 0,
        seconds: 0,
        days: 0,
        speedMultiplier: 1000, // Game time progresses 10 times faster than real time
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
            { name: 'head', path: 'img/sprites/head.png' },
            { name: 'body', path: 'img/sprites/body.png' },
            { name: 'hair', path: 'img/sprites/hair.png' },
            { name: 'hats', path: 'img/sprites/hats.png' },
            { name: 'glasses', path: 'img/sprites/glasses.png' },
            { name: 'facial', path: 'img/sprites/facial.png' },
            { name: 'outfit', path: 'img/sprites/outfit.png' },
            { name: '1', path: 'img/tiles/1.png' },
            { name: 'itemsImg', path: 'img/icons/items.png' },
            { name: 'objectData', path: 'json/objectData.json' },
            { name: 'objectScript', path: 'json/objectScript.json' },
            { name: 'itemsData', path: 'json/itemsData.json' },
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
        ], () => {
            console.log("All assets loaded");
            this.canvas = document.createElement('canvas');
            this.ctx = this.canvas.getContext('2d');
            document.body.appendChild(this.canvas);
            this.resizeCanvas();
            this.itemsImg = assets.load('itemsImg');
            this.itemsData = assets.load('itemsData');
            this.objectData = assets.load('objectData');

            actions.loadObjectScript();

            // Create player sprite
            const playerOptions = {
                id: this.playerid,
                x: 240,
                y: 250,
                isPlayer: true,
                speed: 90,
                head: 1,
                body: 1,
                hairStyle: 1,
                outfit: 3,
                hat: 1,
                facial: 1,
                glasses: 0,
            };
            sprite.create(playerOptions);

            weather.fireflysActive = true;
            weather.fogActive = true;
            weather.rainActive = true;
            weather.snowActive = false;
            weather.nightActive = true;

            weather.createFireflys();
            weather.createFog(0.05);
            weather.createRain(0.7);
            weather.createSnow(0.2);

            const storedSceneId = localStorage.getItem('sceneid') || '66771b7e6c1c5b2f1708b75a';
            this.loadScene(storedSceneId);

            modal.load('ui/objectives.php', "ui_objectives_window", "Objectives", false);
            modal.load('ui/modals.php', "ui_modals_list_window", "Modals List", false);
            modal.load('ui/footer.php', "ui_footer_window", "Footer", false);
            modal.load('menus/click_menu/index.php', 'click_menu_window', "click menu", false);
            modal.load('menus/pie/index.php', 'pie_menu_window', "pie menu", false);
            modal.load('console', null, "console", true);
            modal.load('ui/inventory.php', "ui_inventory_window", "ui window", false);
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
        // Pause game loop
        cancelAnimationFrame(this.animationFrameId);
    
        // Pause sound effects
        audio.pauseAll();
    
        // Mark the game as paused
        this.isPaused = true;
    
        console.log("Game paused due to tab visibility change");
    },
    
    resume: function() {
        // Request the latest game state from the server
        network.send({
            command: 'requestGameState',
            playerId: this.playerid
        });
    
        // Resume sound effects
        audio.resumeAll();
    
        console.log("Game resumed and requesting the latest state from the server");
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
    
                    // Ensure the camera is positioned correctly
                    game.updateCamera();
    
                    // Store the scene id in local storage
                    localStorage.setItem('sceneid', game.sceneid);
    
                    effects.transitions.start('fadeOut', 1000);
                    effects.transitions.start('fadeIn', 1000);
                    ui.notif("scene_change_notif", data.name, true);
                    audio.stopLoopingAudio('music', 0.5);
                    //audio.playAudio("music1", assets.load('music1'), 'music', true);
    
                    console.log('Scene loaded. Room data:', game.roomData);
    
                } else {
                    console.log('Error: ' + data.message);
                    // If scene is not found, load error modal
                    modal.load('console/tabs/servers/ajax/error.php', 'scene_load_error_window', null, "server error", true);
                }
            },
            error: function(data) {
                console.log(data);
                // If scene is not found, load error modal
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
       
    updateCamera: function() {
        if (this.activeCamera) {
            let mainSprite = this.sprites[this.playerid];
            if (mainSprite) {
                var scaledWindowWidth = window.innerWidth / this.zoomLevel;
                var scaledWindowHeight = window.innerHeight / this.zoomLevel;
    
                this.targetCameraX = mainSprite.x + mainSprite.width / 2 - scaledWindowWidth / 2;
                this.targetCameraY = mainSprite.y + mainSprite.height / 2 - scaledWindowHeight / 2;
    
                this.targetCameraX = Math.max(0, Math.min(this.targetCameraX, this.worldWidth - scaledWindowWidth));
                this.targetCameraY = Math.max(0, Math.min(this.targetCameraY, this.worldHeight - scaledWindowHeight));
    
                this.cameraX = this.lerp(this.cameraX, this.targetCameraX, this.lerpFactor);
                this.cameraY = this.lerp(this.cameraY, this.targetCameraY, this.lerpFactor);
    
                // Center map if smaller than viewport
                if (this.worldWidth < scaledWindowWidth) {
                    this.cameraX = -(scaledWindowWidth - this.worldWidth) / 2;
                }
                if (this.worldHeight < scaledWindowHeight) {
                    this.cameraY = -(scaledWindowHeight - this.worldHeight) / 2;
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

      getTileIdAt: function(x, y) {
        if (!this.roomData || !this.roomData.items) {
            console.log('No room data available.');
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
        if (this.isEditorActive) return; // Do nothing if the editor is active
        console.log('Game handleMouseDown triggered');
        if (event.button === 0 || event.button === 2) { // Left or right mouse button
            const rect = this.canvas.getBoundingClientRect();
            const mouseX = (event.clientX - rect.left) / this.zoomLevel + this.cameraX;
            const mouseY = (event.clientY - rect.top) / this.zoomLevel + this.cameraY;
            this.isDragging = true;
            this.dragStart = { x: mouseX, y: mouseY };
            this.dragEnd = null; // Reset dragEnd
            this.selectedTiles = []; // Clear selected tiles on new drag
            this.selectionBounds = null; // Reset selection bounds

            // Disable text selection
            document.body.style.userSelect = 'none';
            document.body.style.webkitUserSelect = 'none'; /* Safari */
            document.body.style.msUserSelect = 'none'; /* IE 10 and IE 11 */

            // Disable camera centering
            this.activeCamera = false;

            // Initialize edge scrolling state
            this.currentMouseX = event.clientX;
            this.currentMouseY = event.clientY;
            this.isEdgeScrolling = false; // Initially set to false
        }
    },

    handleMouseMove: function(event) {
        if (this.isEditorActive) return; // Do nothing if the editor is active
        console.log('Game handleMouseMove triggered');
        if (this.isDragging) {
            const rect = this.canvas.getBoundingClientRect();
            const mouseX = (event.clientX - rect.left) / this.zoomLevel + this.cameraX;
            const mouseY = (event.clientY - rect.top) / this.zoomLevel + this.cameraY;
            this.dragEnd = { x: mouseX, y: mouseY };
            
            const deltaX = Math.abs(this.dragEnd.x - this.dragStart.x);
            const deltaY = Math.abs(this.dragEnd.y - this.dragStart.y);

            if (deltaX >= 8 || deltaY >= 8) {
                this.updateSelectedTiles(); // Ensure selected tiles are updated during drag
                
                this.currentMouseX = event.clientX;
                this.currentMouseY = event.clientY;
        
                // Start edge scrolling if not already started
                if (!this.isEdgeScrolling && (this.dragStart.x !== this.dragEnd.x || this.dragStart.y !== this.dragEnd.y)) {
                    this.isEdgeScrolling = true;
                    this.edgeScroll();
                }
            }
        }
    },

    handleMouseUp: function(event) {
        if (this.isEditorActive) return; // Do nothing if the editor is active
        console.log('Game handleMouseUp triggered');
        const rect = this.canvas.getBoundingClientRect();
        const mouseX = (event.clientX - rect.left) / this.zoomLevel + this.cameraX;
        const mouseY = (event.clientY - rect.top) / this.zoomLevel + this.cameraY;
        this.isDragging = false;
        this.dragEnd = { x: mouseX, y: mouseY };

        const deltaX = Math.abs(this.dragEnd.x - this.dragStart.x);
        const deltaY = Math.abs(this.dragEnd.y - this.dragStart.y);

        if (deltaX < this.dragThreshold && deltaY < this.dragThreshold) {
            // This is a click
            this.handleCanvasClick(event, event.shiftKey);
        } else if (deltaX >= 8 || deltaY >= 8) {
            // This is a drag
            this.handleCanvasDrag({ startX: this.dragStart.x, startY: this.dragStart.y, endX: this.dragEnd.x, endY: this.dragEnd.y }, event.shiftKey);
        }

        // Re-enable text selection
        document.body.style.userSelect = '';
        document.body.style.webkitUserSelect = ''; /* Safari */
        document.body.style.msUserSelect = ''; /* IE 10 and IE 11 */

        modal.showAll();

        // Re-enable camera centering
        this.activeCamera = true;

        // Stop edge scrolling
        this.isEdgeScrolling = false;

        // Clear the selection bounds and selected tiles
        this.selectedTiles = [];
        this.selectionBounds = null;
    },
    
    edgeScroll: function() {
        if (!this.isDragging || !this.isEdgeScrolling) {
            return;
        }
    
        const edgeThreshold = this.edgeScrollBuffer; // Buffer distance from the edge to start scrolling
        const easing = this.edgeScrollEasing; // Easing factor for smooth scrolling
        const maxSpeed = this.edgeScrollSpeed;
    
        let mouseX = this.currentMouseX;
        let mouseY = this.currentMouseY;
    
        // Calculate the scroll amount
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
    
        // Update the camera position
        this.cameraX = Math.max(0, Math.min(this.cameraX + scrollX, this.worldWidth - window.innerWidth / this.zoomLevel));
        this.cameraY = Math.max(0, Math.min(this.cameraY + scrollY, this.worldHeight - window.innerHeight / this.zoomLevel));
    
        // Continue scrolling if dragging
        requestAnimationFrame(this.edgeScroll.bind(this));
    },

    handleCanvasClick: function(event, isShiftKey) {
        console.log('Game handleCanvasClick triggered');
        if (isShiftKey) {
            console.log('Shift + Click detected');
        }
        const rect = this.canvas.getBoundingClientRect();
        const mouseX = (event.clientX - rect.left) / this.zoomLevel + this.cameraX;
        const mouseY = (event.clientY - rect.top) / this.zoomLevel + this.cameraY;

        console.log(`Mouse position: (${mouseX}, ${mouseY})`);

        // Calculate the grid position
        const gridX = Math.floor(mouseX / 16);
        const gridY = Math.floor(mouseY / 16);

        console.log(`Grid position: (${gridX}, ${gridY})`);

        // Store the grid coordinates in the new variables
        this.x = gridX;
        this.y = gridY;

        const selectedObject = this.findObjectAt(mouseX, mouseY);

        if (selectedObject) {
            console.log(`Selected object ID: ${selectedObject.id}`);
        } else {
            console.log('No object selected');
        }

        if (event.button === 2) { // Right-click
            if (selectedObject) {
                this.selectedObjects = [selectedObject];
                if (!this.selectedCache.some(cache => cache.id === selectedObject.id)) {
                    this.selectedCache.push({ id: selectedObject.id, image: this.drawAndOutlineObjectImage(selectedObject) });
                }
            } else {
                this.selectedObjects = [];
            }
        } else if (event.button === 0) { // Left-click
            if (this.isTileWalkable(gridX, gridY)) {
                // Walk to the tile
                const playerSprite = this.sprites[this.playerid];
                playerSprite.walkToClickedTile(gridX, gridY);

                // Update target coordinates for the main sprite
                this.targetX = mouseX;
                this.targetY = mouseY;

                // Deselect any selected object if clicked on a walkable tile
                if (!isShiftKey) {
                    this.selectedObjects = [];
                }

                // Check tile actions after the sprite moves
                playerSprite.checkTileActions();

            } else if (selectedObject) {
                const uniqueId = `${selectedObject.id}_${selectedObject.x}_${selectedObject.y}`;
                if (isShiftKey) {
                    const index = this.selectedObjects.findIndex(obj => `${obj.id}_${obj.x}_${obj.y}` === uniqueId);
                    if (index === -1) {
                        this.selectedObjects.push(selectedObject);
                    } else {
                        this.selectedObjects.splice(index, 1);
                    }
                } else {
                    this.selectedObjects = [selectedObject];
                }

                if (!this.selectedCache.some(cache => cache.id === selectedObject.id)) {
                    this.selectedCache.push({ id: selectedObject.id, image: this.drawAndOutlineObjectImage(selectedObject) });
                }

                // Check if the selected object has a click action
                const objectId = selectedObject.id;
                const objectScript = actions.objectScript[objectId];

                if (objectScript && objectScript.click) {
                    const clickAction = objectScript.click;

                    // Check for required item
                    if (clickAction.requiredItem) {
                        const playerSprite = this.sprites[this.playerid];
                        if (playerSprite.currentItem === clickAction.requiredItem) {
                            console.log(`Action result: ${clickAction.result}`);
                            // Handle the result of the action (e.g., add item to inventory)
                        } else {
                            console.log(`You need a ${clickAction.requiredItem} to perform this action.`);
                        }
                    } else {
                        // Handle click action without required item
                        console.log(`Action result: ${clickAction.result}`);
                    }
                }
            } else {
                this.selectedObjects = [];
            }
        }

        console.log('Current selected objects:', this.selectedObjects);

        // Update the visual selection state
        this.updateSelectedTiles();
        this.render();
    },

    isTileWalkable: function(gridX, gridY) {
        const grid = this.createWalkableGrid(); // Create or fetch the walkable grid
        return grid[gridX] && grid[gridX][gridY] === 1; // Check if the tile is walkable
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

        // Check if the object is already in the cache
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
                        { x: x - 1, y: y },     // Left
                        { x: x + 1, y: y },     // Right
                        { x: x, y: y - 1 },     // Top
                        { x: x, y: y + 1 },     // Bottom
                        { x: x - 1, y: y - 1 }, // Top-left
                        { x: x + 1, y: y - 1 }, // Top-right
                        { x: x - 1, y: y + 1 }, // Bottom-left
                        { x: x + 1, y: y + 1 }  // Bottom-right
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

        // Add the new object image to the cache
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

        console.log('Current selected objects after drag:', this.selectedObjects);
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

        console.log('Current selected objects after drag:', this.selectedObjects);
    },

    isTileSelected: function(tileX, tileY) {
        return this.selectedTiles.some(tile => tile.x === tileX && tile.y === tileY);
    },

    renderSelectedTiles: function() {
        if (this.selectedTiles.length > 0) {
            const { startX, startY, endX, endY } = this.selectionBounds;
            this.ctx.strokeStyle = 'white';
            this.ctx.lineWidth = 8 / this.zoomLevel; // Adjust line width based on zoom level
            this.ctx.setLineDash([10, 10]); // Set line dash for a dashed border
    
            // Add shadow properties
            this.ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
            this.ctx.shadowBlur = 10;
            this.ctx.shadowOffsetX = 4;
            this.ctx.shadowOffsetY = 4;
    
            this.ctx.lineDashOffset -= 0.5; // Slower animation speed by using a smaller decrement value
            this.ctx.strokeRect(startX, startY, endX - startX, endY - startY);
            this.ctx.setLineDash([]); // Reset line dash to default
    
            // Reset shadow properties to avoid affecting other drawings
            this.ctx.shadowColor = 'transparent';
            this.ctx.shadowBlur = 0;
            this.ctx.shadowOffsetX = 0;
            this.ctx.shadowOffsetY = 0;
        }
    },
 
    render: function() {
        this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
        this.ctx.setTransform(1, 0, 0, 1, 0, 0);
        this.ctx.scale(this.zoomLevel, this.zoomLevel);
        this.ctx.translate(-Math.round(this.cameraX), -Math.round(this.cameraY));
    
        const mainSprite = this.sprites[this.playerid];
    
        const renderQueue = [];
        let tileCount = 0;
    
        // Calculate the boundaries of the viewport in world coordinates
        this.viewportXStart = Math.max(0, Math.floor(this.cameraX / 16));
        this.viewportXEnd = Math.min(this.worldWidth / 16, Math.ceil((this.cameraX + window.innerWidth / this.zoomLevel) / 16));
        this.viewportYStart = Math.max(0, Math.floor(this.cameraY / 16));
        this.viewportYEnd = Math.min(this.worldHeight / 16, Math.ceil((this.cameraY + window.innerHeight / this.zoomLevel) / 16));

    // Render background tiles
    const bgTileData = this.objectData[this.sceneBg][0];
    for (let y = this.viewportYStart; y < this.viewportYEnd; y++) {
        for (let x = this.viewportXStart; x < this.viewportXEnd; x++) {
            const posX = x * 16;
            const posY = y * 16;
            const tileFrameIndex = bgTileData.i;
            const srcX = (tileFrameIndex % 150) * 16;
            const srcY = Math.floor(tileFrameIndex / 150) * 16;

            this.ctx.drawImage(assets.load(bgTileData.t), srcX, srcY, 16, 16, posX, posY, 16, 16);
            tileCount++;
        }
    }
    
        //game.grid();
    
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
                                if (Array.isArray(tileData.i[0])) { // Check if there are animation frames
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
                            }
    
                            index++;
                        }
                    }
    
                    // Render lights directly in the render function
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
    
                                    const lightId = `${roomItem.id}_${tileX}_${tileY}`;

                                    // Check if light already exists
                                    const existingLight = effects.lights.find(light => light.id === lightId);
    
                                    if (!existingLight) {
                                        const radius = tileData.lr || 200;
                                        const color = tileData.lc || { r: 255, g: 255, b: 255 };
                                        const intensity = tileData.li || 1;
                                        const flickerSpeed = tileData.lfs || 0.03;
                                        const flickerAmount = tileData.lfa || 0.04;
                                        const lampType = tileData.lt || "lamp";
    
                                        effects.addLight(lightId, posX, posY, radius, color, intensity, lampType, true, flickerSpeed, flickerAmount);
                                    }
                                }
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
                // Add shadow to render queue with z-index of 0
                renderQueue.push({
                    z: 0,
                    draw: function() {
                        game.sprites[id].drawShadow();
                    }
                });
    
                // Add sprite to render queue with z-index of 2
                renderQueue.push({
                    z: 2, // Ensure sprites are drawn above tiles and pathfinder lines
                    draw: function() {
                        game.sprites[id].draw();
                    }
                });
                spriteCount++;
            }
        }
    
        // Sort renderQueue by z-index and render order
        renderQueue.sort((a, b) => a.z - b.z);
    
        // Draw the items in the renderQueue
        renderQueue.forEach(item => {
            item.draw();
        });
    
        // Draw the pathfinder line if available
        if (mainSprite && mainSprite.path && mainSprite.path.length > 0) {
            game.ctx.strokeStyle = 'rgba(255, 255, 0, 0.5)';
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
    
        // Draw the remaining sprites (z-index > 1)
        renderQueue.forEach(item => {
            if (item.z > 1) {
                item.draw();
            }
        });
    
        this.renderSelectedTiles();

        // Draw the selected objects
        if (this.selectedObjects && this.selectedObjects.length > 0) {
            this.selectedObjects.forEach(selectedObject => {
                if (selectedObject) {
                    const cachedObject = this.selectedCache.find(cache => cache.id === selectedObject.id);
                    if (cachedObject) {
                        const xCoordinates = selectedObject.x.map(x => parseInt(x, 10) * 16);
                        const yCoordinates = selectedObject.y.map(y => parseInt(y, 10) * 16);
                        const minX = Math.min(...xCoordinates);
                        const minY = Math.min(...yCoordinates);
                        game.ctx.drawImage(cachedObject.image, minX, minY);
                    }
                }
            });
        }

    // Add the night filter to the renderQueue with a high z-index
    game.ctx.save();
    game.ctx.fillStyle = `rgba(${effects.nightFilter.color.r}, ${effects.nightFilter.color.g}, ${effects.nightFilter.color.b}, ${effects.nightFilter.opacity})`;
    game.ctx.globalCompositeOperation = effects.nightFilter.compositeOperation;
    game.ctx.fillRect(0, 0, game.canvas.width, game.canvas.height);
    game.ctx.restore();

    this.ctx.globalCompositeOperation = effects.compositeOperation;
    this.ctx.drawImage(effects.createLightMask(), 0, 0);
    this.ctx.globalCompositeOperation = 'source-over';
    
        this.ctx.imageSmoothingEnabled = false;
    
        weather.drawSnow();
        weather.drawRain();
        //weather.drawFog();
        weather.drawFireflys();
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
            this.ctx.strokeRect(startX, startY, endX - startX, endY - endY);
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

        // Update the tiles rendered and sprites rendered display
var tilesRenderedDisplay = document.getElementById('tiles_rendered');
if (tilesRenderedDisplay) {
    tilesRenderedDisplay.innerHTML = `Tiles: ${tileCount} | Sprites: ${spriteCount}`;
}

    // Highlight overlapping tiles
    this.overlappingTiles.forEach(tile => {
        game.ctx.fillStyle = 'rgba(255, 0, 0, 0.5)';
        game.ctx.fillRect(tile.x * 16, tile.y * 16, 16, 16);
    });
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

    // Update game time display
    var gameTimeDisplay = document.getElementById('game_time');
    if (gameTimeDisplay) {
        gameTimeDisplay.innerHTML = this.gameTime.display();
    }

    // Continue the loop
    requestAnimationFrame(this.loop.bind(this));
},

    updateGameLogic: function(deltaTime) {
    
        for (let id in game.sprites) {
            const sprite = game.sprites[id];
            if (sprite.update) {
                sprite.update(deltaTime);
                sprite.checkTileActions();
            }
        }
    
        this.gameTime.update(deltaTime);
        effects.updateDayNightCycle();
        this.updateAnimatedTiles(deltaTime);
        weather.updateSnow(deltaTime);
        weather.updateRain(deltaTime);
        weather.updateFog(deltaTime);
        weather.updateFireflys(deltaTime);
        weather.updateLightning(deltaTime);
        this.updateCamera();
        effects.updateParticles(deltaTime);
        effects.transitions.update();
        effects.updateLights(deltaTime);

            // Rain sound effect handling
    if (weather.rainActive) {
        audio.playAudio("rain", assets.load('rain'), 'ambience', true);
    } else {
        audio.stopLoopingAudio('rain', 'ambience', 0.5);
    }

        if(typeof ui_window !== 'undefined' && ui_window.checkAndUpdateUIPositions) {
            ui_window.checkAndUpdateUIPositions();
        }
    },
    
    updateAnimatedTiles: function(deltaTime) {
        if (!this.roomData || !this.roomData.items) return;
    
        // Iterate over each item in the room data
        this.roomData.items.forEach(roomItem => {
            const itemData = assets.load('objectData')[roomItem.id];
            if (itemData && itemData.length > 0) {
                // Initialize roomItem's animation state if not already present
                if (!roomItem.animationState) {
                    roomItem.animationState = itemData.map(tileData => ({
                        currentFrame: 0,
                        elapsedTime: 0
                    }));
                }
    
                // Update each tile's animation state
                itemData.forEach((tileData, index) => {
                    if (tileData.i && Array.isArray(tileData.i[0])) {
                        const animationData = tileData.i;
                        const animationState = roomItem.animationState[index];
    
                        animationState.elapsedTime += deltaTime;
    
                        // Ensure that the frame only advances once per elapsed time period
                        if (animationState.elapsedTime >= tileData.d) {
                            animationState.elapsedTime -= tileData.d;
                            animationState.currentFrame = (animationState.currentFrame + 1) % animationData.length;
                        }
    
                        // Apply the current frame to the tileData for rendering
                        tileData.currentFrame = animationState.currentFrame;
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

                            audio.playAudio("bump1", assets.load('bump1'), 'sfx');
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