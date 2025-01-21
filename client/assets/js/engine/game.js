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
    particles: [],
    overlappingTiles: [],
    isPaused: false,
    timeActive: true,
    inputMethod: 'keyboard',
    fpsHistory: [],
    maxFpsHistory: 60,

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

            this.resizeCanvas();
            this.loop();
            input.init();
            gamepad.init(config);
            audio.start();

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

    resizeCanvas: function() {
        utils.setZoomLevel();
    
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
        
        if (collision.isTileWalkable(this.x, this.y)) {
            this.selectedObjects = [];
            this.selectedCache = [];
            this.render();
            this.mainSprite.walkToClickedTile(this.x, this.y);
            console.log('Tile is walkable, walking to clicked tile.');
        }
    },

    scene: function(sceneId) {
        // Cancel any existing pathfinding for the player.
        input.cancelPathfinding(this.sprites[this.playerid]);
    
        // Use fetch to request the scene data.
        fetch(`/api/scenes/${encodeURIComponent(sceneId)}`)
            .then(response => {
                if (!response.ok) {
                    // If the response isn't OK, throw an error so it goes to the catch block.
                    throw new Error(`Network response was not ok: ${response.statusText}`);
                }
                return response.json();
            })
            .then(data => {
                console.log('Scene response:', data);
    
                if (data.message === 'success') {
                    effects.lights = [];
                    lighting.clearLightsAndEffects();
    
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
    
                    render.sceneBg = data.bg || null;
                    game.resizeCanvas();
    
                    collision.walkableGridCache = null;
                    collision.createWalkableGrid();
    
                    game.overlappingTiles = [];
                    camera.update();
                    effects.transitions.start('fadeOut', 1000);
                    effects.transitions.start('fadeIn', 1000);
    
                } else {
                    console.log('Scene load error:', data.message);
                    plugin.load({
                        id: "scene_load_error_window",
                        url: "plugins/error/index.njk"
                    });
                }
            })
            .catch(error => {
                console.error('Scene load error:', error);
                plugin.load({
                    id: "scene_load_error_window",
                    url: "plugins/error/index.njk"
                });
            });
    },    

    render: function() {
        this.ctx.imageSmoothingEnabled = false;
        this.ctx.setTransform(1, 0, 0, 1, 0, 0);
        
        this.ctx.scale(this.zoomLevel, this.zoomLevel);
        this.ctx.translate(-Math.round(camera.cameraX), -Math.round(camera.cameraY));

        this.viewportXStart = Math.max(0, Math.floor(camera.cameraX / 16));
        this.viewportXEnd = Math.min(this.worldWidth / 16, Math.ceil((camera.cameraX + window.innerWidth / this.zoomLevel) / 16));
        this.viewportYStart = Math.max(0, Math.floor(camera.cameraY / 16));
        this.viewportYEnd = Math.min(this.worldHeight / 16, Math.ceil((camera.cameraY + window.innerHeight / this.zoomLevel) / 16));

        render.renderBackground(this.viewportXStart, this.viewportXEnd, this.viewportYStart, this.viewportYEnd);
        render.renderAll(this.viewportXStart, this.viewportXEnd, this.viewportYStart, this.viewportYEnd);

        plugin.hook('onRender');

        this.ctx.save();
        this.ctx.setTransform(1, 0, 0, 1, 0, 0);

        const { maskCanvas, maskCtx } = lighting.createBaseNightFilter();
        lighting.renderLightsOnFilter(maskCtx);
        lighting.renderFinalOverlay(this.ctx, maskCanvas, maskCtx);
        
        particles.render();
        effects.transitions.render();
        render.renderCarriedObjects();
        render.handleDebugUtilities();

        effects.letterbox.update();

        if (ui.pluginExists('ui_console_editor_inventory') && ui_console_editor_inventory.selectedInventoryItem) {
            ui_console_editor_inventory.render();
        }

        if (ui.pluginExists('ui_console_tab_window')) {
            if (ui.pluginExists('ui_console_tab_window.renderCollisionBoundaries')) {
                ui_console_tab_window.renderCollisionBoundaries();
            }
            if (ui.pluginExists('ui_console_tab_window.renderNearestWalkableTile')) {
                ui_console_tab_window.renderNearestWalkableTile();
            }
            if (ui.pluginExists('ui_console_tab_window.renderObjectCollision')) {
                ui_console_tab_window.renderObjectCollision();
            }
        }

        if (ui.pluginExists("ui_overlay_window.update") && this.mainSprite && this.mainSprite.isVehicle) {
            ui_overlay_window.update(this.mainSprite.currentSpeed, this.mainSprite.maxSpeed);
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

        // Update logic in fixed steps
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

        plugin.hook('onLoop');

        requestAnimationFrame(this.loop.bind(this));
    }
};