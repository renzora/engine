game = {
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
    zoomLevel: localStorage.getItem('zoomLevel') ? parseInt(localStorage.getItem('zoomLevel')) : 5,
    targetX: 0,
    targetY: 0,
    roomData: undefined,
    sprites: {},
    playerid: null,
    sceneid: localStorage.getItem('sceneid') || '6741ff4e81ea9fd1700704f3',
    desiredFPS: 60,
    fixedDeltaTime: 1000 / 60,
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

    create: function(e) {
        this.canvas = document.createElement('canvas');
        this.ctx = this.canvas.getContext('2d');
        this.ctx.imageSmoothingEnabled = false;
        document.body.appendChild(this.canvas);
        this.resizeCanvas();
        this.loop();
        gamepad.init(e);
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
    
        // Skip console window width calculation if in edit mode
        if (!game.isEditMode && consoleElement && console_window.isOpen) {
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
        input.cancelPathfinding(game.sprites[game.playerid]);
        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'plugins/editor/console/tabs/servers/ajax/getSceneData.php',
            data: 'scene_id=' + encodeURIComponent(sceneId),
            success: function(data) {
                if (data.message === 'success') {
                    effects.lights = [];
                    lighting.clearLightsAndEffects();
                    game.roomData = data.roomData;
                    game.sceneid = data.sceneid;
                    game.serverid = data.server_id;
                    game.worldWidth = data.width || 1280;
                    game.worldHeight = data.height || 944;
                    game.x = data.startingX || 0; // Actual top-left pixel X
                    game.y = data.startingY || 0; // Actual top-left pixel Y
    
                    // Update sprite position if player exists
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
                    localStorage.setItem('sceneid', game.sceneid);
                    effects.transitions.start('fadeOut', 1000);
                    effects.transitions.start('fadeIn', 1000);
                    ui.notif("scene_change_notif", data.name, true);
                    audio.playAudio('gameplay_music_01', assets.use('gameplay_music_01'), 'music', true, '0.5');
                    audio.stopLoopingAudio('gameplay_music_01', 0.5);
    
                } else {
                    console.log('Error: ' + data.message);
                    plugin.load({
                        id: "scene_load_error_window",
                        url: "editor/console/tabs/servers/ajax/error.php",
                    });
                }
            },
            error: function(data) {
                console.log(data);

                plugin.load({
                    id: "scene_load_error_window",
                    url: "editor/console/tabs/servers/ajax/error.php",
                });
            }
        });
    },

render: function () {
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

    if(utils.pluginExists('weather_plugin')) {
        weather_plugin.rain.draw();
        weather_plugin.snow.draw();
        weather_plugin.fireflys.draw();
    }

    this.ctx.save();
    this.ctx.setTransform(1, 0, 0, 1, 0, 0);

    const { maskCanvas, maskCtx } = lighting.createBaseNightFilter();
    lighting.renderLightsOnFilter(maskCtx);
    lighting.renderFinalOverlay(this.ctx, maskCanvas, maskCtx);
    
    particles.render();
    effects.transitions.render();
    render.renderCarriedObjects();
    render.handleDebugUtilities();
    render.aimTool();

    if(utils.pluginExists('ui_footer_window.updateUi')) {
        ui_footer_window.updateUI();
    }

    effects.letterbox.update();

    if (utils.pluginExists('ui_console_editor_inventory') && ui_console_editor_inventory.selectedInventoryItem) {
        ui_console_editor_inventory.render();
    }

    if (utils.pluginExists('edit_mode_window')) {
        if (typeof edit_mode_window.renderSelectionBox === 'function') {
            edit_mode_window.renderSelectionBox();
        }
        if (typeof edit_mode_window.renderBrush === 'function') {
            edit_mode_window.renderBrush();
        }
        if (typeof edit_mode_window.renderSelectedTiles === 'function') {
            edit_mode_window.renderSelectedTiles();
        }
        if (typeof edit_mode_window.renderLasso === 'function') {
            edit_mode_window.renderLasso();
        }
    }
    

    if(utils.pluginExists('ui_console_tab_window')) {
        if (utils.pluginExists('ui_console_tab_window.renderCollisionBoundaries')) {
            ui_console_tab_window.renderCollisionBoundaries();
        }
        if (utils.pluginExists('ui_console_tab_window.renderNearestWalkableTile')) {
            ui_console_tab_window.renderNearestWalkableTile();
        }
        if (utils.pluginExists('ui_console_tab_window.renderObjectCollision')) {
            ui_console_tab_window.renderObjectCollision();
        }
    }

    if (utils.pluginExists("ui_overlay_window.update") && game.mainSprite.isVehicle) {
        ui_overlay_window.update(game.mainSprite.currentSpeed, game.mainSprite.maxSpeed);
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
