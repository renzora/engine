var game = {
    needsFilterUpdate: true,
    canvas: undefined,
    ctx: undefined,
    isEditMode: false,
    x: null,
    y: null,
    timestamp: 0,
    lastTime: 0,
    deltaTime: 0,
    worldWidth: 1280,
    worldHeight: 944,
    zoomLevel: localStorage.getItem('zoomLevel') ? parseInt(localStorage.getItem('zoomLevel')) : 4,
    targetX: 0,
    targetY: 0,
    roomData: undefined,
    sprites: {},
    playerid: null,
    sceneid: null,
    desiredFPS: 60,
    fixedDeltaTime: 1000 / 60,
    accumulatedTime: 0,
    lastTime: null,
    maxAccumulatedTime: 1000,
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
    activeSpriteId: null,
    timeActive: true,
    inputMethod: 'keyboard',
    fpsHistory: [],
    maxFpsHistory: 60,
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
        hours: 0,
        minutes: 0,
        seconds: 0,
        days: 0,
        speedMultiplier: 100,
        daysOfWeek: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        update: function(deltaTime) {
            if (!game.timeActive) return;  // Stop time updates if time is not active
            
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
        localStorage.setItem('zoomLevel', this.zoomLevel);
    
        // Adjust the canvas size based on the zoom level
        const baseWidth = window.innerWidth;
        const baseHeight = window.innerHeight;
    
        // Increase or decrease the canvas size depending on the zoom level
        const scaledWidth = baseWidth / this.zoomLevel;
        const scaledHeight = baseHeight / this.zoomLevel;
    
        // Update the canvas element size to reflect the zoom
        this.canvas.width = scaledWidth;
        this.canvas.height = scaledHeight;
    
        console.log('Zoom level set to:', this.zoomLevel);
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
            { name: 'pickUp', path: 'audio/sfx/fx/pickUp.mp3' },
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


            game.canvas.addEventListener('click', function(event) {
                if (console_window.isOpen) {
                    console_window.toggleConsoleWindow();
                }
            });
        

            if (game.sprites[this.playerid]) {
                this.mainSprite = game.sprites[this.playerid];
                this.setActiveSprite(this.playerid);
            } else {
                console.warn(`Sprite with player ID ${this.playerid} not found.`);
            }




            console.log(this.mainSprite);

            weather.createFireflys();
            weather.createRain(0.7);
            weather.createSnow(0.6);
            weather.createClouds();

            const storedSceneId = localStorage.getItem('sceneid') || '66c25a30091e7e9dd7040daf';
            this.loadScene(storedSceneId);

            if (localStorage.getItem('showMainTitle') === null || localStorage.getItem('showMainTitle') === 'true') {
                modal.load({
                    id: "main_title_window",
                    url: "menus/main_title/index.php",
                    name: "Main Tiles",
                    showInList: true
                });
            } else {
                const playerOptions = {
                    id: this.playerid,
                    x: 29,
                    y: 23,
                    isPlayer: true,
                    speed: 100,
                    head: 1,
                    eyes: 1,
                    body: 1,
                    hair: 1,
                    outfit: 1,
                    hands: 2,
                    hat: 0,
                    facial: 0,
                    glasses: 0,
                    targetAim: false,
                    maxRange: 200,
                    health: 100,
                    energy: 100
                  };
              
                  // Assuming `sprite` and `effects` are globally available
                  sprite.create(playerOptions);
                  this.mainSprite = this.sprites[this.playerid];
                  this.setActiveSprite(this.playerid);
                  this.modal_init();
            }
            
            //modal.load({ id: 'ui_objectives_window', url: 'ui/objectives.php', name: 'Objectives', drag: false, reload: false });

            console.log("Connected to Main renzora server");

            this.loop();

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

    modal_init: function() {
        modal.load({ id: 'auth_window', url: 'auth/index.php', name: 'SignIn', drag: true,reload: true });
    
        modal.load({ id: 'ui_footer_window', url: 'ui/footer.php', name: 'Footer', drag: false, reload: false });
    
        modal.load({ id: 'console_window', url: 'menus/console', name: 'console', drag: false, reload: true });
    
        modal.load({ id: 'click_menu_window', url: 'menus/click_menu/index.php', name: 'click menu', drag: true, reload: false });
    
        modal.load({ id: 'pie_menu_window', url: 'menus/pie/index.php', name: 'pie menu',drag: false, reload: false });
    
        modal.load({ id: 'ui_overlay_window', url: 'ui/overlay.php', name: 'overlay', drag: false, reload: false });

        modal.load({ id: 'ui_inventory_window', url: 'ui/inventory.php', name: 'ui window',drag: false, reload: false });
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
            url: 'modals/menus/console/tabs/servers/ajax/getSceneData.php',
            data: 'scene_id=' + encodeURIComponent(sceneId),
            success: function(data) {
                if (data.message === 'success') {
                    effects.lights = [];
                    lighting.clearLightsAndEffects();
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
                    //ui.notif("scene_change_notif", data.name, true);
                    audio.stopLoopingAudio('music', 0.5);
    
                    //game.spawnRandomItems(500);
    
                } else {
                    console.log('Error: ' + data.message);
                    modal.load('menus/console/tabs/servers/ajax/error.php', 'scene_load_error_window', null, "server error", true);
                }
            },
            error: function(data) {
                console.log(data);

                modal.load({
                    id: "scene_load_error_window",
                    url: "menus/console/tabs/servers/ajax/error.php",
                    name: "Server Error",
                    showInList: true
                });
            }
        });
    },

    spawnRandomItems: function(numberOfItems) {
        if (!this.roomData || !this.roomData.items) {
            console.error('Room data not loaded.');
            return;
        }
    
        const itemKeys = Object.keys(this.objectData).filter(key => {
            const item = this.objectData[key][0];
            return item.type === "item";
        });
    
        if (itemKeys.length === 0) {
            console.error('No items with type "item" found.');
            return;
        }
    
        const maxX = this.worldWidth / 16;
        const maxY = this.worldHeight / 16;
    
        for (let i = 0; i < numberOfItems; i++) {
            let randomItemKey;
            let randomX;
            let randomY;
            let tileOccupied;
    
            do {
                randomItemKey = itemKeys[Math.floor(Math.random() * itemKeys.length)];
                randomX = Math.floor(Math.random() * maxX);
                randomY = Math.floor(Math.random() * maxY);
                tileOccupied = utils.getTileIdAt(randomX, randomY) !== null || !collision.isTileWalkable(randomX, randomY);
            } while (tileOccupied);
    
            const newItem = {
                id: randomItemKey,
                x: [randomX],
                y: [randomY],
                animationState: [{
                    currentFrame: 0,
                    elapsedTime: 0
                }],
                zIndex: [2],  // Ensure zIndex is set to 2
                type: "item"  // Ensure type is set to "item"
            };
    
            this.roomData.items.push(newItem);
            collision.createWalkableGrid();
            console.log(`Spawned ${randomItemKey} at (${randomX}, ${randomY}) with zIndex of 2`);
        }
    },    

    resizeCanvas: function() {
        this.canvas.width = window.innerWidth;
        this.canvas.height = window.innerHeight;
        this.canvas.style.position = 'absolute';
        this.canvas.style.left = '50%';
        this.canvas.style.top = '50%';
        this.canvas.style.transform = 'translate(-50%, -50%)';
    },

      handleMouseDown: function(event) {
        if (this.isEditMode || (this.mainSprite && this.mainSprite.targetAim)) return; // Add this check for isEditMode
        console.log('Game handleMouseDown triggered');
        
        // Only handle walk movement, no tile selection
        if (event.button === 0 || event.button === 2) {
            const rect = this.canvas.getBoundingClientRect();
            const mouseX = (event.clientX - rect.left) / this.zoomLevel + camera.cameraX;
            const mouseY = (event.clientY - rect.top) / this.zoomLevel + camera.cameraY;
            this.x = Math.floor(mouseX / 16);
            this.y = Math.floor(mouseY / 16);
        }
    },

    handleMouseMove: function(event) {

    },
    
    handleMouseUp: function(event) {
        if (this.isEditMode || (this.mainSprite && this.mainSprite.targetAim)) return; // Add this check for isEditMode
        console.log('Game handleMouseUp triggered');
        
        const rect = this.canvas.getBoundingClientRect();
        const mouseX = (event.clientX - rect.left) / this.zoomLevel + camera.cameraX;
        const mouseY = (event.clientY - rect.top) / this.zoomLevel + camera.cameraY;
        this.x = Math.floor(mouseX / 16);
        this.y = Math.floor(mouseY / 16);
        
        if (collision.isTileWalkable(this.x, this.y)) {
            this.selectedObjects = [];
            this.selectedCache = [];
            this.render();
            this.mainSprite.walkToClickedTile(this.x, this.y);
            console.log('Tile is walkable, walking to clicked tile.');
        }
    },

    render: function () {
        // Clear the canvas and fill with the background color
        this.ctx.fillStyle = '#333';
        this.ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);
    
        // Reset transformations before rendering the map
        this.ctx.setTransform(1, 0, 0, 1, 0, 0);
        
        // Apply zoom and translate based on camera position
        this.ctx.scale(this.zoomLevel, this.zoomLevel);
        this.ctx.translate(-Math.round(camera.cameraX), -Math.round(camera.cameraY));
    
        // Calculate the viewport boundaries
        this.viewportXStart = Math.max(0, Math.floor(camera.cameraX / 16));
        this.viewportXEnd = Math.min(this.worldWidth / 16, Math.ceil((camera.cameraX + window.innerWidth / this.zoomLevel) / 16));
        this.viewportYStart = Math.max(0, Math.floor(camera.cameraY / 16));
        this.viewportYEnd = Math.min(this.worldHeight / 16, Math.ceil((camera.cameraY + window.innerHeight / this.zoomLevel) / 16));
    
        // Render all the tiles and sprites
        const { backgroundTileCount, tileCount, spriteCount, animationCount } = render.renderAll(this.viewportXStart, this.viewportXEnd, this.viewportYStart, this.viewportYEnd);
    
        // Render selected item following the mouse cursor if the inventory window exists and an item is selected
        if (utils.objExists('ui_console_editor_inventory') && ui_console_editor_inventory.selectedInventoryItem) {
            ui_console_editor_inventory.render();
        }
    
        // Render other game effects and utilities
        render.renderPathfinderLine();
        render.renderCarriedObjects();
        render.renderLightingEffects();
        render.renderWeatherEffects();
        render.handleDebugUtilities();
        particles.renderParticles();
        effects.transitions.render();
    
        // Update the UI with tile, sprite, and animation counts
        render.updateUI(backgroundTileCount + tileCount, spriteCount, animationCount);
        render.highlightOverlappingTiles();
    
        // Handle effects like letterbox
        effects.letterboxEffect.update();
        effects.letterboxEffect.render();
    
        // Render edit mode if the window exists
        if (utils.objExists('edit_mode_window')) {
            edit_mode_window.renderSelectionBox();
            edit_mode_window.renderBrush();
            edit_mode_window.renderSelectedTiles();
            edit_mode_window.renderLasso();
        }
    
        // Render additional utilities for the console tab window if they exist
        if (utils.objExists('ui_console_tab_window')) {
            if (utils.objExists('ui_console_tab_window.renderCollisionBoundaries')) {
                ui_console_tab_window.renderCollisionBoundaries();
            }
            if (utils.objExists('ui_console_tab_window.renderNearestWalkableTile')) {
                ui_console_tab_window.renderNearestWalkableTile();
            }
            if (utils.objExists('ui_console_tab_window.renderObjectCollision')) {
                ui_console_tab_window.renderObjectCollision();
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
    
        const fpsUpdateInterval = 100;  // Update FPS and chart every 100ms
        if (timestamp - this.lastFpsUpdateTime >= fpsUpdateInterval) {
            const debugFPS = document.getElementById('gameFps');
            const fps = 1000 / timeElapsed;
    
            // Update FPS display
            if (debugFPS) {
                debugFPS.innerHTML = "FPS: " + fps.toFixed(2);
            }
    
            // Update FPS chart if fps_monitor_window exists
            if (window.fps_monitor_window && typeof fps_monitor_window.renderChart === 'function') {
                fps_monitor_window.renderChart(fps);
            }
    
            this.lastFpsUpdateTime = timestamp;
        }
    
        const gameTimeDisplay = document.getElementById('game_time');
        if (gameTimeDisplay) {
            gameTimeDisplay.innerHTML = this.gameTime.display();
        }
    
        requestAnimationFrame(this.loop.bind(this));
    }
    
};
