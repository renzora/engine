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
    sceneBg: '6737dcd37d05e',
    activeSpriteId: null,
    timeActive: true,
    inputMethod: 'keyboard',
    fpsHistory: [],
    maxFpsHistory: 60,

    reloadGameData: function() {
        const assetsToReload = ['objectData', 'roomData'];

        assets.reloadAssets(assetsToReload, () => {
            console.log("Game data reloaded");
            this.roomData = assets.use('roomData');
            console.log("Game elements updated");
        });
    },

    init: function() {
        this.playerid = network.getToken('renaccount') || `player_${Math.floor(Math.random() * 10000)}`;

        assets.preload([
            { name: 'character', path: 'img/sprites/character/test/character.png' },
            { name: 'head', path: 'img/sprites/character/test/head.png' },
            { name: 'eyes', path: 'img/sprites/character/test/eyes.png' },
            { name: 'hair', path: 'img/sprites/character/test/hair.png' },
            { name: 'hands', path: 'img/sprites/character/test/hands.png' },
            { name: 'hats', path: 'img/sprites/character/test/hats.png' },
            { name: 'glasses', path: 'img/sprites/character/test/glasses.png' },
            { name: 'facial', path: 'img/sprites/character/test/facial.png' },
            { name: 'outfit', path: 'img/sprites/character/test/outfit.png' },
            { name: 'horse', path: 'img/sprites/animals/horse/1.png' },
            { name: 'cow', path: 'img/sprites/animals/cow/1.png' },
            { name: 'chick', path: 'img/sprites/animals/chick/1.png' },
            { name: 'chicken', path: 'img/sprites/animals/chicken/1.gif' },
            { name: 'pig', path: 'img/sprites/animals/pig/1.png' },
            { name: 'female-01', path: 'img/sprites/character/female-01/1.png' },
            { name: 'gen1', path: 'img/tiles/gen1.png' },
            { name: 'gameplay_music_01', path: 'audio/music/gameplay_music_01.mp3' },
            { name: 'itemsImg', path: 'img/icons/items.png' },
            { name: 'objectData', path: 'json/objectData.json' },
            { name: 'itemsData', path: 'json/itemsData.json' },
            { name: 'spritesData', path: 'json/spritesData.json' },
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
            { name: 'door_open_01', path: 'audio/sfx/movement/door.mp3' },
            { name: 'nightAmbience', path: 'audio/sfx/weather/nightAmbience.mp3' },
            { name: 'grass1', path: 'audio/sfx/movement/grass1.mp3' },
            { name: 'footstep_01', path: 'audio/sfx/movement/footstep.wav' },
            { name: 'footsteps1', path: 'audio/sfx/movement/footsteps1.wav' },
            { name: 'speech_menu_01', path: 'audio/sfx/ui/speech_menu_01.wav' },
            { name: 'rain', path: 'audio/sfx/weather/rain.mp3' },
            { name: 'meta', path: 'json/meta.json' },
            { name: 'machinegun1', path: 'audio/sfx/fx/machineGun.mp3' },
            { name: 'reload_gun', path: 'audio/sfx/fx/reload_gun.mp3' },
            { name: 'empty_gun', path: 'audio/sfx/fx/empty_gun.mp3' },
            { name: 'electronic_readout_01', path: 'audio/sfx/ui/electronic_readout_01.wav' },
            { name: 'store_chime_01', path: 'audio/sfx/fx/store_chime_01.mp3' },
        ], () => {
            console.log("All assets loaded");
            this.canvas = document.createElement('canvas');
            this.ctx = this.canvas.getContext('2d');
            this.ctx.imageSmoothingEnabled = false;
            document.body.appendChild(this.canvas);
            
            this.resizeCanvas();
            this.itemsImg = assets.use('itemsImg');
            this.itemsData = assets.use('itemsData');
            this.objectData = assets.use('objectData');
            this.fxData = assets.use('fxData');


            game.canvas.addEventListener('click', function(event) {
                if (console_window.isOpen) {
                    console_window.toggleConsoleWindow();
                }
            });

            weather.createFireflys();
            weather.createRain(0.7);
            weather.createSnow(0.6);

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
                    x: null,
                    y: null,
                    isPlayer: true,
                    topSpeed: 100,
                    animalType: 'female-01',
                    targetAim: false,
                    maxRange: 200,
                    health: 100,
                    energy: 100
                  };
              
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

        modal.load({ id: 'ui_inventory_window', url: 'ui/inventory.php', name: 'ui window',drag: false, reload: false });
    
        modal.load({ id: 'console_window', url: 'console/index.php', name: 'console', drag: false, reload: true });
    
        modal.load({ id: 'click_menu_window', url: 'menus/click_menu/index.php', name: 'click menu', drag: true, reload: false });
    
        modal.load({ id: 'pie_menu_window', url: 'menus/pie/index.php', name: 'pie menu',drag: false, reload: false });
    
        modal.load({ id: 'ui_overlay_window', url: 'ui/overlay.php', name: 'overlay', drag: false, reload: false });

        modal.load({ id: 'speech_window', url: 'speech/index.php', name: 'speech', drag: false, reload: true });

        //modal.load({ id: 'joypad_window', url: 'joypad/index.php', name: 'joypad', drag: false, reload: true });

        //modal.load({ id: 'navigator_window', url: 'navigator/index.php', name: 'navigator', drag: true, reload: true });
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
                    lighting.clearLightsAndEffects();
                    game.roomData = data.roomData;
                    game.sceneid = data.sceneid;
                    game.serverid = data.server_id; // Store the server_id for later use
                    game.worldWidth = data.width || 1280;
                    game.worldHeight = data.height || 944;
                    game.sceneBg = data.bg || 'grass';
                    game.resizeCanvas();
                    game.mainSprite.x = data.startingX || 0;
                    game.mainSprite.y = data.startingY || 0;
                    game.mainSprite.direction = data.facing || 'S';
                    collision.walkableGridCache = null;
                    collision.createWalkableGrid();

                    this.overlappingTiles = [];
                    camera.update();
                    localStorage.setItem('sceneid', game.sceneid);
                    effects.transitions.start('fadeOut', 1000);
                    effects.transitions.start('fadeIn', 1000);
                    //ui.notif("scene_change_notif", data.name, true);
                    audio.playAudio('gameplay_music_01', assets.use('gameplay_music_01'), 'music', true, '0.5');
                    audio.stopLoopingAudio('gameplay_music_01', 0.5);
    
                } else {
                    console.log('Error: ' + data.message);
                    modal.load('console/tabs/servers/ajax/error.php', 'scene_load_error_window', null, "server error", true);
                }
            },
            error: function(data) {
                console.log(data);

                modal.load({
                    id: "scene_load_error_window",
                    url: "console/tabs/servers/ajax/error.php",
                    name: "Server Error",
                    showInList: true
                });
            }
        });
    },

resizeCanvas: function() {
    const consoleElement = document.getElementById('console_window');
    const adjacentMenu = document.getElementById('tabs');
    let consoleWidth = 0;
    let menuWidth = 0;

    if (consoleElement && console_window.isOpen) {
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
        
        if (collision.isTileWalkable(this.x, this.y)) {
            this.selectedObjects = [];
            this.selectedCache = [];
            this.render();
            this.mainSprite.walkToClickedTile(this.x, this.y);
            console.log('Tile is walkable, walking to clicked tile.');
        }
    },

// game.js
render: function () {
    this.ctx.imageSmoothingEnabled = false;
    this.ctx.setTransform(1, 0, 0, 1, 0, 0);
    
    this.ctx.scale(this.zoomLevel, this.zoomLevel);
    this.ctx.translate(-Math.round(camera.cameraX), -Math.round(camera.cameraY));

    this.viewportXStart = Math.max(0, Math.floor(camera.cameraX / 16));
    this.viewportXEnd = Math.min(this.worldWidth / 16, Math.ceil((camera.cameraX + window.innerWidth / this.zoomLevel) / 16));
    this.viewportYStart = Math.max(0, Math.floor(camera.cameraY / 16));
    this.viewportYEnd = Math.min(this.worldHeight / 16, Math.ceil((camera.cameraY + window.innerHeight / this.zoomLevel) / 16));

    // Render background and sprites
    render.renderBackground(this.viewportXStart, this.viewportXEnd, this.viewportYStart, this.viewportYEnd);
    render.renderAll(this.viewportXStart, this.viewportXEnd, this.viewportYStart, this.viewportYEnd);

    // Apply the night filter
    lighting.renderNightFilter();

    
    weather.render();
    particles.render();
    
    effects.transitions.render();
    render.renderPathfinderLine();
    render.renderCarriedObjects();
    render.handleDebugUtilities();
    render.aimTool();

    if(utils.objExists('ui_footer_window')) {
        ui_footer_window.updateUI();
    }

    effects.letterbox.update();

    if (utils.objExists('ui_console_editor_inventory') && ui_console_editor_inventory.selectedInventoryItem) {
        ui_console_editor_inventory.render();
    }

    if(utils.objExists('edit_mode_window')) {
        edit_mode_window.renderSelectionBox();
        edit_mode_window.renderBrush();
        edit_mode_window.renderSelectedTiles();
        edit_mode_window.renderLasso();
    }

    if(utils.objExists('ui_console_tab_window')) {
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

    const fps = 1000 / timeElapsed;
    utils.tracker('fps', fps);
  
    if (window.fps_monitor_window && typeof fps_monitor_window.renderChart === 'function') {
        utils.finalizeFrame();
        fps_monitor_window.renderChart();
    }

    const debugFPS = document.getElementById('gameFps');
    if (debugFPS) {
        debugFPS.innerHTML = "FPS: " + fps.toFixed(2);
    }

    const gameTimeDisplay = document.getElementById('game_time');
    if (gameTimeDisplay) {
        gameTimeDisplay.innerHTML = utils.gameTime.display();
    }

    requestAnimationFrame(this.loop.bind(this));
    }
    
};
