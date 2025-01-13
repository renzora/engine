<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config/db.php';
if ($auth) {
?>

<div
      id="editor_toolbar_buttons"
      class="fixed top-2 bg-black/80 text-white rounded-lg shadow-lg p-2 flex gap-2 overflow-x-auto"
      style="margin-bottom: 10px;">
      <button type="button" id="select_button" class="mode-button shadow flex items-center justify-center hover:bg-gray-700 hover:rounded transition hint--top" onclick="edit_mode_window.changeMode('select')" aria-label="Select">
        <div class="ui_icon ui_select"></div>
      </button>
      <button type="button" id="brush_button" class="mode-button shadow flex items-center justify-center hover:bg-gray-700 hover:rounded transition" onclick="edit_mode_window.changeMode('brush')">
        <div class="ui_icon ui_brush"></div>
      </button>
      <button type="button" id="pan_button" class="mode-button shadow flex items-center justify-center hover:bg-gray-700 hover:rounded transition" onclick="edit_mode_window.changeMode('pan')">
        <div class="ui_icon ui_pan"></div>
      </button>
      <button type="button" id="lasso_button" class="mode-button shadow flex items-center justify-center hover:bg-gray-700 hover:rounded transition" onclick="edit_mode_window.changeMode('lasso')">
        <div class="ui_icon ui_lasso"></div>
      </button>
      <button type="button" id="move_button" class="mode-button shadow flex items-center justify-center hover:bg-gray-700 hover:rounded transition" onclick="edit_mode_window.changeMode('move')">
        <div class="ui_icon ui_move"></div>
      </button>
      <button type="button" id="save_button" class="mode-button shadow flex items-center justify-center hover:bg-gray-700 hover:rounded transition" onclick="edit_mode_window.saveRoomData()">
        <div class="ui_icon ui_save"></div>
      </button>
      <button type="button" id="close_button" class="mode-button shadow flex items-center justify-center hover:bg-gray-700 hover:rounded transition" onclick="plugin.close('edit_mode_window');">
        <div class="ui_icon ui_close"></div>
      </button>
    </div>

<style>
    body.move-cursor {
      cursor: move !important;
    }
  </style>

  <script>
window[id] = {
    id: id,
    renderMode: '2d',
    originalRoomData: JSON.parse(JSON.stringify(game.roomData)),
    modeButtons: {},
    brushRadius: 16,
    isBrushModeActive: false,
    isPanning: false,
    isMiddleClickPanning: false,
    mouseX: 0,
    mouseY: 0,
    lastMouseX: 0,
    lastMouseY: 0,
    defaultCursor: 'default',
    previousMode: null,
    modes: ['select', 'brush', 'zoom', 'delete', 'pan', 'lasso', 'move'],
    lassoPath: [],
    isLassoActive: false,
    boundMouseMoveHandler: null,
    boundMouseDownHandler: null,
    boundMouseUpHandler: null,
    boundMouseScrollHandler: null,
    boundKeyDownHandler: null,
    clipboard: [],
    isAddingNewObject: false,
    isDragging: false,
    selectionStart: { x: 0, y: 0 },
    selectionEnd: { x: 0, y: 0 },
    selectedObjects: [],
    undoStack: [],
    redoStack: [],
    isMovingObjects: false,
    moveOffsetX: 0,
    moveOffsetY: 0,
    currentHour: null,
    currentMinute: null,
    currentDay: null,
    isSnapEnabled: false,
    isGroupObjectsEnabled: false,
    dragStartAxis: null,
    hoveredAxis: null,

    start: function () {
    this.modeButtons = {
        brush: document.getElementById('brush_button'),
        select: document.getElementById('select_button'),
        zoom: document.getElementById('zoom_button'),
        delete: document.getElementById('delete_button'),
        pan: document.getElementById('pan_button'),
        lasso: document.getElementById('lasso_button'),
        move: document.getElementById('move_button')
    };

    game.timeActive = true;
    game.isEditMode = true;
    game.pathfinding = false;
    game.allowControls = true;
    camera.lerpEnabled = false;
    game.zoomLevel = 4;
    actions.tooltipActive = false;
    //game.mainSprite.stopPathfinding();
    actions.hideTooltip();

    this.updateCurrentTimeAndDay();
    this.boundMouseMoveHandler = this.handleMouseMove.bind(this);
    this.boundMouseDownHandler = this.handleMouseDown.bind(this);
    this.boundMouseUpHandler = this.handleMouseUp.bind(this);
    this.boundMouseScrollHandler = this.handleMouseScroll.bind(this);
    this.boundKeyDownHandler = this.handleKeyDown.bind(this);

    game.canvas.addEventListener('mousemove', this.boundMouseMoveHandler);
    game.canvas.addEventListener('mousedown', this.boundMouseDownHandler);
    game.canvas.addEventListener('mouseup', this.boundMouseUpHandler);
    game.canvas.addEventListener('wheel', this.boundMouseScrollHandler);
    window.addEventListener('keyup', this.handleKeyUp.bind(this));
    window.addEventListener('keydown', this.boundKeyDownHandler);

    plugin.minimize('ui_inventory_window');
    plugin.minimize('ui_overlay_window');
    plugin.close('context_menu_window');
    
    console_window.load_tab_buttons('editor');
    console_window.toggleConsoleWindow('editor_inventory');
    console_window.allowToggle = false;

    plugin.preload([
        { priority: 0, options: { id: 'editor_layers', url: 'editor/layers/index.html', drag: false, reload: false } },
        { priority: 1, options: { id: 'editor_context_menu_window', url: 'editor/context_menu/index.php', drag: false, reload: true } },
        { priority: 2, options: { id: 'ui_footer_window', url: 'ui/dev/index.html', drag: false, reload: false } }
    ]);

    setTimeout(() => { camera.manual = true; }, 0);
    this.changeMode('select');
},

unmount: function () {
    game.isEditMode = false;
    game.pathfinding = true;
    game.allowControls = true;
    game.timeActive = true;
    utils.gameTime.hours = this.currentHour;
    utils.gameTime.minutes = this.currentMinute;
    game.displaySprite = true;
    camera.lerpEnabled = true;
    camera.manual = false;
    game.zoomLevel = 4;

    game.canvas.removeEventListener('mousemove', this.boundMouseMoveHandler);
    game.canvas.removeEventListener('mousedown', this.boundMouseDownHandler);
    game.canvas.removeEventListener('mouseup', this.boundMouseUpHandler);
    game.canvas.removeEventListener('wheel', this.boundMouseScrollHandler);
    window.removeEventListener('keydown', this.boundKeyDownHandler);
    window.removeEventListener('keyup', this.boundKeyUpHandler);

    this.isDragging = false;
    this.isLassoActive = false;
    this.selectedObjects = [];
    this.lassoPath = [];
    this.selectionStart = { x: 0, y: 0 };
    this.selectionEnd = { x: 0, y: 0 };
    this.isMovingObjects = false;
    this.isMiddleClickPanning = false;
    actions.tooltipActive = true;

    this.clearSelectionBox();
    this.clearLassoPath();

    document.body.style.cursor = 'default';

    plugin.load({ id: 'context_menu_window', url: 'ui/menus/context_menu/index.html', name: 'Context Menu', drag: false,reload: true });
    plugin.close('editor_context_menu_window');
    plugin.close('console_window');
    plugin.close('ui_footer_window');
    plugin.close('editor_layers');
    plugin.show('ui_inventory_window');
    plugin.show('ui_overlay_window');
    game.resizeCanvas();
},

updateCurrentTimeAndDay: function () {
        const gameTime = utils.gameTime;
        this.currentHour = Math.floor(gameTime.hours);
        this.currentMinute = Math.floor(gameTime.minutes);
        this.currentDay = gameTime.daysOfWeek[gameTime.days % 7];
    },

    changeMode: function (newMode) {
    const contextMenu = document.getElementById('editor_context_menu_window');
    if (contextMenu) {
        contextMenu.classList.add('hidden');
    }

    if (this.isDragging) {
        return;
    }

    if (game.editorMode === newMode) return;

    document.body.style.cursor = 'default';
    game.editorMode = newMode;
    this.isBrushModeActive = false;
    this.isLassoActive = false;
    this.isPanning = false;

    switch (newMode) {
        case 'select':
            this.defaultCursor = 'pointer';
            break;
        case 'brush':
            this.defaultCursor = 'crosshair';
            this.isBrushModeActive = true;
            break;
        case 'pan':
            this.defaultCursor = 'grab';
            this.isPanning = true;
            break;
        case 'lasso':
            this.defaultCursor = 'crosshair';
            this.isLassoActive = true;
            break;
        default:
            this.defaultCursor = 'default';
    }

    document.body.style.cursor = this.defaultCursor;

    const buttons = document.querySelectorAll('.mode-button');
    buttons.forEach(button => {
        button.classList.remove('active');
        if (button.id === `${newMode}_button`) {
            button.classList.add('active');
        }
    });
},

handleMouseMove: function (event) {
    const rect = game.canvas.getBoundingClientRect();
    this.mouseX = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
    this.mouseY = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;

    if (event.buttons === 4 && this.isMiddleClickPanning) {
        const deltaX = event.clientX - this.lastMouseX;
        const deltaY = event.clientY - this.lastMouseY;
        const canvasLeft = parseInt(game.canvas.style.left || '0', 10);
        const canvasTop = parseInt(game.canvas.style.top || '0', 10);
        const atLeftEdge = camera.cameraX <= 0;
        const atRightEdge = camera.cameraX >= game.worldWidth - (window.innerWidth / game.zoomLevel);
        const atTopEdge = camera.cameraY <= 0;
        const atBottomEdge = camera.cameraY >= game.worldHeight - (window.innerHeight / game.zoomLevel);
        const isAtHorizontalEdge = atLeftEdge || atRightEdge;
        const isAtVerticalEdge = atTopEdge || atBottomEdge;

        if (!isAtHorizontalEdge && !isAtVerticalEdge) {
            const cameraDeltaX = deltaX / game.zoomLevel;
            const cameraDeltaY = deltaY / game.zoomLevel;

            camera.cameraX = Math.max(
                0,
                Math.min(camera.cameraX - cameraDeltaX, game.worldWidth - (window.innerWidth / game.zoomLevel))
            );
            camera.cameraY = Math.max(
                0,
                Math.min(camera.cameraY - cameraDeltaY, game.worldHeight - (window.innerHeight / game.zoomLevel))
            );

            this.lastMouseX = event.clientX;
            this.lastMouseY = event.clientY;
            return;
        }

        game.canvas.style.left = `${canvasLeft + deltaX}px`;
        game.canvas.style.top = `${canvasTop + deltaY}px`;

        this.lastMouseX = event.clientX;
        this.lastMouseY = event.clientY;
        return;
    }

    if (this.isDragging && event.buttons === 1) {
        if (game.editorMode === 'move') {
            this.handleObjectMovement();
            return;
        }
        
        if (game.editorMode === 'brush' || game.editorMode === 'lasso') {
            this.lassoPath.push({ x: this.mouseX, y: this.mouseY });
            this.renderLasso();
            return;
        }
    }

    if (game.editorMode === 'move' && this.selectedObjects.length > 0 && !this.isDragging) {
        let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
        this.selectedObjects.forEach(obj => {
            minX = Math.min(minX, Math.min(...obj.x) * 16);
            minY = Math.min(minY, Math.min(...obj.y) * 16);
            maxX = Math.max(maxX, Math.max(...obj.x) * 16 + 16);
            maxY = Math.max(maxY, Math.max(...obj.y) * 16 + 16);
        });

        const centerX = (minX + maxX) / 2;
        const centerY = (minY + maxY) / 2;
        const axisLength = 30;
        const hoverTolerance = 5;
        const centerCircleRadius = 10;
        const distToCenter = Math.sqrt(
            Math.pow(this.mouseX - centerX, 2) + 
            Math.pow(this.mouseY - centerY, 2)
        );
        
        if (distToCenter <= centerCircleRadius) {
            this.hoveredAxis = 'center';
            document.body.style.cursor = 'move';
            return;
        }

        const axes = [{ 
                name: 'x',
                mainLine: { dx: axisLength, dy: axisLength/2 },
                oppositeLine: { dx: -axisLength, dy: -axisLength/2 }
            },
            { 
                name: 'z',
                mainLine: { dx: -axisLength, dy: axisLength/2 },
                oppositeLine: { dx: axisLength, dy: -axisLength/2 }
            },
            { 
                name: 'y',
                mainLine: { dx: 0, dy: -axisLength },
                oppositeLine: { dx: 0, dy: axisLength }
            }];

        this.hoveredAxis = null;
        for (const axis of axes) {
            const mainDist = this.pointToLineDistance(
                this.mouseX, this.mouseY,
                centerX + Math.cos(Math.atan2(axis.mainLine.dy, axis.mainLine.dx)) * centerCircleRadius,
                centerY + Math.sin(Math.atan2(axis.mainLine.dy, axis.mainLine.dx)) * centerCircleRadius,
                centerX + axis.mainLine.dx,
                centerY + axis.mainLine.dy
            );

            const oppositeDist = this.pointToLineDistance(
                this.mouseX, this.mouseY,
                centerX + Math.cos(Math.atan2(axis.oppositeLine.dy, axis.oppositeLine.dx)) * centerCircleRadius,
                centerY + Math.sin(Math.atan2(axis.oppositeLine.dy, axis.oppositeLine.dx)) * centerCircleRadius,
                centerX + axis.oppositeLine.dx,
                centerY + axis.oppositeLine.dy
            );

            if (Math.min(mainDist, oppositeDist) < hoverTolerance) {
                this.hoveredAxis = axis.name;
                document.body.style.cursor = 'pointer';
                return;
            }
        }

        if (!this.isDragging) {
            document.body.style.cursor = this.defaultCursor;
        }
    }

    if (game.editorMode === 'pan' && this.isPanning && event.buttons === 1) {
        const deltaX = event.clientX - this.lastMouseX;
        const deltaY = event.clientY - this.lastMouseY;

        camera.cameraX -= deltaX / game.zoomLevel;
        camera.cameraY -= deltaY / game.zoomLevel;
        this.constrainCamera();

        this.lastMouseX = event.clientX;
        this.lastMouseY = event.clientY;
        return;
    }

    if (this.isDragging && event.buttons === 1) {
        if (game.editorMode === 'select') {
            this.handleSelectionBox(event, rect);
        }
    }
},

handleMouseDown: function (event) {
    if (edit_mode_window.isAddingNewObject) return;
    if (game.editorMode === 'pan') {
        this.isPanning = true;
        this.lastMouseX = event.clientX;
        this.lastMouseY = event.clientY;
        document.body.style.cursor = 'grabbing';
        return;
    }

    this.updateMousePosition(event);

    if (event.button === 0 && game.editorMode === 'move') {
        if (this.selectedObjects.length > 0) {
            let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
            this.selectedObjects.forEach(obj => {
                minX = Math.min(minX, Math.min(...obj.x) * 16);
                minY = Math.min(minY, Math.min(...obj.y) * 16);
                maxX = Math.max(maxX, Math.max(...obj.x) * 16 + 16);
                maxY = Math.max(maxY, Math.max(...obj.y) * 16 + 16);
            });
            
            const centerX = (minX + maxX) / 2;
            const centerY = (minY + maxY) / 2;
            const centerCircleRadius = 6;
            
            const distToCenter = Math.sqrt(
                Math.pow(this.mouseX - centerX, 2) + 
                Math.pow(this.mouseY - centerY, 2)
            );
            
            if (distToCenter <= centerCircleRadius) {
                this.dragStartAxis = 'center';
                this.startObjectMove(event);
                return;
            }

            if (this.hoveredAxis) {
                this.dragStartAxis = this.hoveredAxis;
                this.startObjectMove(event);
                return;
            }
        }
    }

    if (event.button === 0 && this.selectedObjects.length > 0 && !event.shiftKey) {
        if (this.hoveredAxis) {
            return;
        }

        const clickedOnSelected = this.selectedObjects.some(obj => {
            const objRect = {
                x: Math.min(...obj.x) * 16,
                y: Math.min(...obj.y) * 16,
                width: (Math.max(...obj.x) - Math.min(...obj.x) + 1) * 16,
                height: (Math.max(...obj.y) - Math.min(...obj.y) + 1) * 16
            };

            return (
                this.mouseX >= objRect.x &&
                this.mouseX <= objRect.x + objRect.width &&
                this.mouseY >= objRect.y &&
                this.mouseY <= objRect.y + objRect.height
            );
        });

        if (!clickedOnSelected) {
            const clickedObject = game.roomData.items.find(obj => {
                if (this.isObjectLocked(obj)) return false;
                
                const objRect = {
                    x: Math.min(...obj.x) * 16,
                    y: Math.min(...obj.y) * 16,
                    width: (Math.max(...obj.x) - Math.min(...obj.x) + 1) * 16,
                    height: (Math.max(...obj.y) - Math.min(...obj.y) + 1) * 16
                };

                return (
                    this.mouseX >= objRect.x &&
                    this.mouseX <= objRect.x + objRect.width &&
                    this.mouseY >= objRect.y &&
                    this.mouseY <= objRect.y + objRect.height
                );
            });

            this.selectedObjects = clickedObject ? [clickedObject] : [];
            this.clearSelectionBox();
            if (clickedObject) {
                this.changeMode('move');
            } else {
                this.changeMode('select');
            }
            return;
        }
    }

    if (event.button === 2) {
        const isClickInsideSelection = this.isCursorInsideSelectedArea();
        if (!isClickInsideSelection) {
            this.selectedObjects = [];
            this.clearSelectionBox();
            this.clearLassoPath();
            this.changeMode('select');
        }
        return;
    }

    if (event.button === 1) {
        this.previousMode = game.editorMode;
        this.changeMode('pan');
        this.isMiddleClickPanning = true;
        this.isPanning = true;
        this.lastMouseX = event.clientX;
        this.lastMouseY = event.clientY;
        document.body.style.cursor = 'grabbing';
        return;
    }

    if (event.button === 0) {
        if (game.editorMode === 'brush' || game.editorMode === 'lasso') {
            this.isDragging = true;
            this.lassoPath = [{ x: this.mouseX, y: this.mouseY }];
            return;
        }

        if (event.shiftKey && game.editorMode === 'move') {
            const clickedObject = this.selectedObjects.find(obj => {
                const objRect = {
                    x: Math.min(...obj.x) * 16,
                    y: Math.min(...obj.y) * 16,
                    width: (Math.max(...obj.x) - Math.min(...obj.x) + 1) * 16,
                    height: (Math.max(...obj.y) - Math.min(...obj.y) + 1) * 16
                };

                return (
                    this.mouseX >= objRect.x &&
                    this.mouseX <= objRect.x + objRect.width &&
                    this.mouseY >= objRect.y &&
                    this.mouseY <= objRect.y + objRect.height
                );
            });

            if (clickedObject && this.isObjectLocked(clickedObject)) {
                return;
            }

            if (clickedObject) {
                this.selectedObjects = this.selectedObjects.filter(obj => obj !== clickedObject);
                if (this.selectedObjects.length === 0) {
                    this.changeMode('select');
                }
                this.renderSelectedTiles();
                return;
            }
        }

        if (game.editorMode === 'move') {
            if (this.selectedObjects.some(obj => this.isObjectLocked(obj))) {
                return;
            }
            this.handleMoveMode(event);
        } else if (game.editorMode === 'select') {
            this.handleSelectionStart(event);
        }
    }
},

handleMouseUp: function (event) {
    if (event.button === 1 && this.isMiddleClickPanning) {
        this.isMiddleClickPanning = false;
        this.isPanning = false;
        this.changeMode(this.previousMode);
        document.body.style.cursor = this.defaultCursor;
        return;
    }

    if (event.button === 0) {
        this.isDragging = false;
        this.dragStartAxis = null;
        
        if (this.isPanning) {
            this.isPanning = false;
            document.body.style.cursor = this.defaultCursor;
        }
        
        if (game.editorMode === 'brush' || game.editorMode === 'lasso') {
            if (this.lassoPath.length > 2) {
                this.updateSelectedObjectsWithLasso(event.shiftKey);
            }
            this.clearLassoPath();
            return;
        }
        
        if (this.handleSelectionEnd(event)) return;
        
        if (game.editorMode === 'move') {
            this.finalizeObjectMovement();
        }
    }
},

handleMouseScroll: function (event) {
    if (!game.isEditMode) return;
    const zoomFactor = event.deltaY < 0 ? 1 : -1;
    const newZoomLevel = Math.max(2, Math.min(game.zoomLevel + zoomFactor, 10));
    const rect = game.canvas.getBoundingClientRect();
    const mouseXOnCanvas = event.clientX - rect.left;
    const mouseYOnCanvas = event.clientY - rect.top;
    const worldMouseX = mouseXOnCanvas / game.zoomLevel + camera.cameraX;
    const worldMouseY = mouseYOnCanvas / game.zoomLevel + camera.cameraY;
    game.zoomLevel = newZoomLevel;
    const newWorldMouseX = mouseXOnCanvas / game.zoomLevel + camera.cameraX;
    const newWorldMouseY = mouseYOnCanvas / game.zoomLevel + camera.cameraY;
    camera.cameraX += worldMouseX - newWorldMouseX;
    camera.cameraY += worldMouseY - newWorldMouseY;
    this.constrainCamera();
},

isCursorInsideSelectedArea: function () {
    return this.selectedObjects.some(obj => {
        const objRect = {
            x: Math.min(...obj.x) * 16,
            y: Math.min(...obj.y) * 16,
            width: (Math.max(...obj.x) - Math.min(...obj.x) + 1) * 16,
            height: (Math.max(...obj.y) - Math.min(...obj.y) + 1) * 16
        };

        return (
            this.mouseX >= objRect.x &&
            this.mouseX <= objRect.x + objRect.width &&
            this.mouseY >= objRect.y &&
            this.mouseY <= objRect.y + objRect.height
        );
    });
},

handlePanning: function (event) {
    if (event.button === 1 || game.editorMode === 'pan') {
        this.isDragging = true;
        this.isPanning = true;
        this.lastMouseX = event.clientX;
        this.lastMouseY = event.clientY;
        document.body.style.cursor = 'grabbing';
        return true;
    }
    return false;
},

handleCameraPanning: function(event) {
    const deltaX = event.clientX - this.lastMouseX;
    const deltaY = event.clientY - this.lastMouseY;
    camera.cameraX -= deltaX / game.zoomLevel;
    camera.cameraY -= deltaY / game.zoomLevel;
    this.constrainCamera();
    this.lastMouseX = event.clientX;
    this.lastMouseY = event.clientY;
},

handleZoomDrag: function (event) {
    if (event.button === 0) {
        this.isDragging = true;
        this.lastMouseY = event.clientY;
        return true;
    }
    return false;
},

handleObjectMovement: function () {
    if (this.selectedObjects.length === 0) return;
    let totalDeltaX = this.mouseX - this.lastMouseX;
    let totalDeltaY = this.mouseY - this.lastMouseY;

    if (this.dragStartAxis && this.dragStartAxis !== 'center') {
        switch (this.dragStartAxis) {
            case 'x':
                const xProjection = (totalDeltaX + totalDeltaY) / 2;
                totalDeltaX = xProjection;
                totalDeltaY = xProjection;
                break;
            case 'z':
                const zProjection = (-totalDeltaX + totalDeltaY) / 2;
                totalDeltaX = -zProjection;
                totalDeltaY = zProjection;
                break;
            case 'y': 
                totalDeltaX = 0;
                break;
        }
    }

    this.selectedObjects.forEach((obj) => {
        obj.x = obj.x.map(coord => coord + totalDeltaX / 16);
        obj.y = obj.y.map(coord => coord + totalDeltaY / 16);
    });

    this.lastMouseX = this.mouseX;
    this.lastMouseY = this.mouseY;
},

handleSelectionStart: function (event) {
    if (edit_mode_window.isAddingNewObject) return;
    this.isDragging = true;
    const rect = game.canvas.getBoundingClientRect();
    this.selectionStart = {
        x: (event.clientX - rect.left) / game.zoomLevel + camera.cameraX,
        y: (event.clientY - rect.top) / game.zoomLevel + camera.cameraY
    };
    this.selectionEnd = { ...this.selectionStart };

    if (!event.shiftKey) {
        this.selectedObjects = [];
    }
},

handleSelectionBox: function(event, rect) {
    if (game.editorMode === 'select' || (game.editorMode === 'move' && event.shiftKey)) {
        this.selectionEnd = {
            x: (event.clientX - rect.left) / game.zoomLevel + camera.cameraX,
            y: (event.clientY - rect.top) / game.zoomLevel + camera.cameraY
        };
        this.renderSelectionBox();
        const edgeThreshold = 50;
        const viewportWidth = window.innerWidth;
        const viewportHeight = window.innerHeight;
        const canvasStyle = game.canvas.style;
        const canvasLeft = parseInt(canvasStyle.left || '0', 10);
        const canvasTop = parseInt(canvasStyle.top || '0', 10);

        if (event.clientX < edgeThreshold) {
            if (camera.cameraX > 0) {
                camera.cameraX -= 5;
            } else {
                game.canvas.style.left = `${Math.min(canvasLeft + 5, 0)}px`;
            }
        } else if (event.clientX > viewportWidth - edgeThreshold) {
            const maxCameraX = game.worldWidth - viewportWidth / game.zoomLevel;
            if (camera.cameraX < maxCameraX) {
                camera.cameraX += 5;
            } else {
                game.canvas.style.left = `${Math.max(canvasLeft - 5, viewportWidth - rect.width)}px`;
            }
        }

        if (event.clientY < edgeThreshold) {
            if (camera.cameraY > 0) {
                camera.cameraY -= 5;
            } else {
                game.canvas.style.top = `${Math.min(canvasTop + 5, 0)}px`;
            }
        } else if (event.clientY > viewportHeight - edgeThreshold) {
            const maxCameraY = game.worldHeight - viewportHeight / game.zoomLevel;
            if (camera.cameraY < maxCameraY) {
                camera.cameraY += 5;
            } else {
                game.canvas.style.top = `${Math.max(canvasTop - 5, viewportHeight - rect.height)}px`;
            }
        }

        this.constrainCamera();
    }
},

handleLassoStart: function (event) {
    if (event.button === 0) {
        this.isDragging = true;
        this.lassoPath = [{ x: this.mouseX, y: this.mouseY }];
        return true;
    }
    return false;
},

handleLassoDragging: function() {
    this.lassoPath.push({ x: this.mouseX, y: this.mouseY });
    this.renderLasso();
},

handleMoveMode: function (event) {
    const clickedOnSelectedObject = this.checkIfClickedOnSelectedObject();

    if (!clickedOnSelectedObject) {
        this.changeToSelectModeAndStart(event);
    } else {
        this.startObjectMove(event);
    }
},

checkIfClickedOnSelectedObject: function () {
    return this.selectedObjects.some(obj => {
        const objRect = {
            x: Math.min(...obj.x) * 16,
            y: Math.min(...obj.y) * 16,
            width: (Math.max(...obj.x) - Math.min(...obj.x) + 1) * 16,
            height: (Math.max(...obj.y) - Math.min(...obj.y) + 1) * 16
        };

        return this.mouseX >= objRect.x &&
            this.mouseX <= objRect.x + objRect.width &&
            this.mouseY >= objRect.y &&
            this.mouseY <= objRect.y + objRect.height;
    });
},

startObjectMove: function (event) {
    this.isDragging = true;
    this.initialOffsets = [];

    this.selectedObjects.forEach(obj => {
        const offsetX = this.mouseX - (obj.x[0] * 16);
        const offsetY = this.mouseY - (obj.y[0] * 16);

        this.initialOffsets.push({
            obj: obj,
            offsetX: offsetX,
            offsetY: offsetY
        });
    });

    this.lastMouseX = this.mouseX;
    this.lastMouseY = this.mouseY;
    this.clearSelectionBox();
},

changeToSelectModeAndStart: function (event) {
    this.changeMode('select');
    this.isDragging = true;
    const rect = game.canvas.getBoundingClientRect();
    this.selectionStart = {
        x: (event.clientX - rect.left) / game.zoomLevel + camera.cameraX,
        y: (event.clientY - rect.top) / game.zoomLevel + camera.cameraY
    };
    this.selectionEnd = { ...this.selectionStart };
},

handleSelectionEnd: function (event) {
    if (game.editorMode === 'select') {
        this.isDragging = false;
        this.updateSelectedObjects(event.shiftKey);
        this.clearSelectionBox();
        return true;
    }

    if (game.editorMode === 'lasso') {
        this.isDragging = false;
        this.updateSelectedObjectsWithLasso(event.shiftKey);
        this.clearLassoPath();
        return true;
    }
    return false;
},

finalizeObjectMovement: function () {
    this.isDragging = false;
    if (this.selectedObjects.length > 0) {
        this.selectedObjects.forEach(obj => {
            obj.x = obj.x.map(coord => {
                const pixelCoordX = coord * 16;
                const snappedX = Math.round(pixelCoordX) / 16;
                return snappedX;
            });
            obj.y = obj.y.map(coord => {
                const pixelCoordY = coord * 16;
                const snappedY = Math.round(pixelCoordY) / 16;
                return snappedY;
            });
        });
        this.pushToUndoStack();
    }
},

handlePanningEnd: function (event) {
    if (event.button === 1 || game.editorMode === 'pan') {
        this.isPanning = false;
        this.isDragging = false;
        document.body.style.cursor = this.defaultCursor;
        this.stopSlidingCamera();
        return true;
    }
    return false;
},

moveSelectedObjectsWithArrowKeys: function (direction) {
    if (this.selectedObjects.length === 0) {
        return;
    }

    const step = this.isSnapEnabled ? 16 : 1;
    let deltaX = 0, deltaY = 0;
    switch (direction) {
        case 'ArrowUp':
            deltaY = -step;
            break;
        case 'ArrowDown':
            deltaY = step;
            break;
        case 'ArrowLeft':
            deltaX = -step;
            break;
        case 'ArrowRight':
            deltaX = step;
            break;
    }

    this.selectedObjects.forEach(obj => {
        obj.x = obj.x.map(coord => coord + deltaX / 16);
        obj.y = obj.y.map(coord => coord + deltaY / 16);
    });

    console.log(`Objects moved ${direction} by ${step} pixels.`);

},

updateMousePosition: function (event) {
    const rect = game.canvas.getBoundingClientRect();
    this.mouseX = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
    this.mouseY = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;
},

handleBrushModeScroll: function(deltaY, ctrlKeyPressed, event) {
    if (ctrlKeyPressed) {
        this.zoomInBrushMode(deltaY, event);
    } else {
        this.adjustBrushSize(deltaY);
    }
},

renderSelectedTiles: function () {
    if (this.selectedObjects.length > 0) {
        game.ctx.save();

        const animationSpeed = 300;
        const markerLength = 4;
        const lineWidth = 1;
        const shadowOffset = 1;
        const paddingFactor = 0.15;
        const axisLength = 30;
        const centerCircleRadius = 5;

        game.ctx.lineWidth = lineWidth;

        const vibrantColors = [
            'rgb(255, 0, 0)', 'rgb(0, 255, 0)', 'rgb(0, 0, 255)',
            'rgb(255, 255, 0)', 'rgb(255, 0, 255)', 'rgb(0, 255, 255)',
            'rgb(255, 165, 0)', 'rgb(128, 0, 128)'
        ];

        if (!this.objectColors || this.objectColors.length !== this.selectedObjects.length) {
            this.objectColors = this.selectedObjects.map((_, index) => vibrantColors[index % vibrantColors.length]);
        }

        let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
        this.selectedObjects.forEach(obj => {
            minX = Math.min(minX, Math.min(...obj.x) * 16);
            minY = Math.min(minY, Math.min(...obj.y) * 16);
            maxX = Math.max(maxX, Math.max(...obj.x) * 16 + 16);
            maxY = Math.max(maxY, Math.max(...obj.y) * 16 + 16);
        });

        const centerX = (minX + maxX) / 2;
        const centerY = (minY + maxY) / 2;

        this.selectedObjects.forEach((obj, index) => {
            const objectColor = this.objectColors[index];
            const objWidth = Math.max(...obj.x) - Math.min(...obj.x) + 1;
            const objHeight = Math.max(...obj.y) - Math.min(...obj.y) + 1;
            const paddingX = objWidth * 16 * paddingFactor;
            const paddingY = objHeight * 16 * paddingFactor;
            const minObjX = Math.min(...obj.x) * 16 + paddingX;
            const minObjY = Math.min(...obj.y) * 16 + paddingY;
            const maxObjX = Math.max(...obj.x) * 16 + 16 - paddingX;
            const maxObjY = Math.max(...obj.y) * 16 + 16 - paddingY;
            const corners = [
                { x1: minObjX, y1: minObjY, x2: minObjX + markerLength, y2: minObjY, x3: minObjX, y3: minObjY + markerLength, dx: 1, dy: 1 },
                { x1: maxObjX, y1: minObjY, x2: maxObjX - markerLength, y2: minObjY, x3: maxObjX, y3: minObjY + markerLength, dx: -1, dy: 1 },
                { x1: minObjX, y1: maxObjY, x2: minObjX + markerLength, y2: maxObjY, x3: minObjX, y3: maxObjY - markerLength, dx: 1, dy: -1 },
                { x1: maxObjX, y1: maxObjY, x2: maxObjX - markerLength, y2: maxObjY, x3: maxObjX, y3: maxObjY - markerLength, dx: -1, dy: -1 }
            ];

            const timeOffset = performance.now() / animationSpeed;
            const offset = Math.sin(timeOffset) * 2;

            corners.forEach(corner => {
                const animatedX = corner.dx * offset;
                const animatedY = corner.dy * offset;
                game.ctx.strokeStyle = 'rgba(0, 0, 0, 0.5)';
                game.ctx.beginPath();
                game.ctx.moveTo(corner.x1 + animatedX + shadowOffset, corner.y1 + animatedY + shadowOffset);
                game.ctx.lineTo(corner.x2 + animatedX + shadowOffset, corner.y2 + animatedY + shadowOffset);
                game.ctx.moveTo(corner.x1 + animatedX + shadowOffset, corner.y1 + animatedY + shadowOffset);
                game.ctx.lineTo(corner.x3 + animatedX + shadowOffset, corner.y3 + animatedY + shadowOffset);
                game.ctx.stroke();
                game.ctx.strokeStyle = objectColor;
                game.ctx.beginPath();
                game.ctx.moveTo(corner.x1 + animatedX, corner.y1 + animatedY);
                game.ctx.lineTo(corner.x2 + animatedX, corner.y2 + animatedY);
                game.ctx.moveTo(corner.x1 + animatedX, corner.y1 + animatedY);
                game.ctx.lineTo(corner.x3 + animatedX, corner.y3 + animatedY);
                game.ctx.stroke();
            });
        });

        if (game.editorMode === 'move' && this.selectedObjects.length > 0) {
            game.ctx.beginPath();
            game.ctx.arc(centerX, centerY, centerCircleRadius, 0, Math.PI * 2);
            if (this.hoveredAxis === 'center' || this.dragStartAxis === 'center') {
                game.ctx.fillStyle = 'rgba(200, 200, 200, 0.8)';
                game.ctx.strokeStyle = 'rgba(255, 255, 255, 0.8)';
            } else {
                game.ctx.fillStyle = 'rgba(150, 150, 150, 0.5)';
                game.ctx.strokeStyle = 'rgba(255, 255, 255, 0.5)';
            }
            game.ctx.fill();
            game.ctx.stroke();

            const axes = [
                { name: 'x', color: 'rgb(255, 50, 50)', x: axisLength, y: axisLength/2 },
                { name: 'z', color: 'rgb(50, 50, 255)', x: -axisLength, y: axisLength/2 },
                { name: 'y', color: 'rgb(50, 255, 50)', x: 0, y: -axisLength }
            ];

            axes.forEach(axis => {
                const isAxisHovered = this.hoveredAxis === axis.name;
                const isSelected = this.dragStartAxis === axis.name;
                const color = isAxisHovered || isSelected ? axis.color : axis.color.replace('rgb', 'rgba').replace(')', ', 0.5)');
                game.ctx.beginPath();
                const angle = Math.atan2(axis.y, axis.x);
                const startX = centerX + Math.cos(angle) * centerCircleRadius;
                const startY = centerY + Math.sin(angle) * centerCircleRadius;
                game.ctx.moveTo(startX, startY);
                game.ctx.lineTo(centerX + axis.x, centerY + axis.y);
                game.ctx.strokeStyle = color;
                game.ctx.lineWidth = isAxisHovered || isSelected ? 3 : 2;
                game.ctx.stroke();
                game.ctx.beginPath();
                const oppositeAngle = Math.atan2(-axis.y, -axis.x);
                const oppositeStartX = centerX + Math.cos(oppositeAngle) * centerCircleRadius;
                const oppositeStartY = centerY + Math.sin(oppositeAngle) * centerCircleRadius;
                game.ctx.moveTo(oppositeStartX, oppositeStartY);
                game.ctx.lineTo(centerX - axis.x, centerY - axis.y);
                game.ctx.strokeStyle = color;
                game.ctx.stroke();
            });
        }

        game.ctx.restore();
    }
},

startObjectMove: function (event) {
    this.isDragging = true;
    this.initialOffsets = [];
    const axisClicked = this.checkAxisClick(event);
    if (axisClicked) {
        this.currentDragAxis = axisClicked;
        console.log('Starting drag on axis:', axisClicked);
    } else {
        this.currentDragAxis = null;
    }

    this.selectedObjects.forEach(obj => {
        this.initialOffsets.push({
            obj: obj,
            offsetX: this.mouseX - obj.x[0] * 16,
            offsetY: this.mouseY - obj.y[0] * 16
        });
    });

    this.lastMouseX = this.mouseX;
    this.lastMouseY = this.mouseY;
    this.clearSelectionBox();
},

checkAxisClick: function(event) {
    if (this.selectedObjects.length === 0) return null;

    const obj = this.selectedObjects[0];
    const centerX = (Math.min(...obj.x) + Math.max(...obj.x)) / 2 * 16;
    const bottomY = Math.max(...obj.y) * 16;
    const axisLength = 30;
    const clickRadius = 8;
    const axes = {
        x: { dx: axisLength, dy: axisLength/2 },
        z: { dx: -axisLength, dy: axisLength/2 },
        y: { dx: 0, dy: -axisLength }
    };

    for (const [axis, vector] of Object.entries(axes)) {
        const endX = centerX + vector.dx;
        const endY = bottomY + vector.dy;
        const dist = this.pointToLineDistance(
            this.mouseX, this.mouseY,
            centerX, bottomY,
            endX, endY
        );

        if (dist < clickRadius) {
            return axis;
        }
    }

    return null;
},

pointToLineDistance: function(px, py, x1, y1, x2, y2) {
    const A = px - x1;
    const B = py - y1;
    const C = x2 - x1;
    const D = y2 - y1;

    const dot = A * C + B * D;
    const lenSq = C * C + D * D;
    let param = -1;

    if (lenSq !== 0) {
        param = dot / lenSq;
    }

    let xx, yy;

    if (param < 0) {
        xx = x1;
        yy = y1;
    } else if (param > 1) {
        xx = x2;
        yy = y2;
    } else {
        xx = x1 + param * C;
        yy = y1 + param * D;
    }

    const dx = px - xx;
    const dy = py - yy;
    const distance = Math.sqrt(dx * dx + dy * dy);
    game.ctx.save();
    game.ctx.fillStyle = 'rgba(255, 0, 0, 0.3)';
    game.ctx.beginPath();
    game.ctx.arc(xx, yy, 2, 0, Math.PI * 2);
    game.ctx.fill();
    game.ctx.restore();

    return distance;
},

isObjectLocked: function(obj) {
    const layerId = `item_${obj.layer_id}`;
    const layerNode = editor_layers.findNodeById(layerId);
    return layerNode && layerNode.node.locked;
},

isTopmostObject: function (obj, selectedObjects) {
    const objIndex = game.roomData.items.indexOf(obj);

    return selectedObjects.every(otherObj => {
        if (obj === otherObj) return true;
        const otherIndex = game.roomData.items.indexOf(otherObj);

        if (otherIndex > objIndex) {
            const objRect = {
                x: Math.min(...obj.x) * 16,
                y: Math.min(...obj.y) * 16,
                width: (Math.max(...obj.x) - Math.min(...obj.x) + 1) * 16,
                height: (Math.max(...obj.y) - Math.min(...obj.y) + 1) * 16,
            };

            const otherRect = {
                x: Math.min(...otherObj.x) * 16,
                y: Math.min(...otherObj.y) * 16,
                width: (Math.max(...otherObj.x) - Math.min(...otherObj.x) + 1) * 16,
                height: (Math.max(...otherObj.y) - Math.min(...otherObj.y) + 1) * 16,
            };

            return !(
                objRect.x < otherRect.x + otherRect.width &&
                objRect.x + objRect.width > otherRect.x &&
                objRect.y < otherRect.y + otherRect.height &&
                objRect.y + objRect.height > otherRect.y
            );
        }

        return true;
    });
},

renderSelectionBox: function () {
    if (this.isDragging) {
        const rect = {
            x: Math.min(this.selectionStart.x, this.selectionEnd.x),
            y: Math.min(this.selectionStart.y, this.selectionEnd.y),
            width: Math.abs(this.selectionEnd.x - this.selectionStart.x),
            height: Math.abs(this.selectionEnd.y - this.selectionStart.y)
        };

        game.ctx.save();
        game.ctx.fillStyle = 'rgba(255, 255, 255, 0.2)';
        game.ctx.fillRect(rect.x, rect.y, rect.width, rect.height);
        game.ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
        game.ctx.shadowBlur = 8;
        game.ctx.shadowOffsetX = 4;
        game.ctx.shadowOffsetY = 4;
        const dashSpeed = performance.now() / 100;
        game.ctx.lineDashOffset = -dashSpeed;
        game.ctx.strokeStyle = 'rgba(255, 255, 255, 0.9)';
        game.ctx.lineWidth = 2;
        game.ctx.setLineDash([6, 3]);
        game.ctx.strokeRect(rect.x, rect.y, rect.width, rect.height);
        game.ctx.restore();
    }
},

renderLasso: function () {
    if (this.lassoPath.length > 1) {
        const dashSpeed = performance.now() / 100;
        game.ctx.strokeStyle = 'rgba(255, 255, 255, 0.9)';
        game.ctx.lineWidth = 2;
        game.ctx.setLineDash([6, 3]);
        game.ctx.lineDashOffset = -dashSpeed;
        game.ctx.beginPath();
        game.ctx.moveTo(this.lassoPath[0].x, this.lassoPath[0].y);

        for (let i = 1; i < this.lassoPath.length; i++) {
            game.ctx.lineTo(this.lassoPath[i].x, this.lassoPath[i].y);
        }

        game.ctx.stroke();
        game.ctx.setLineDash([]);
        game.ctx.lineDashOffset = 0;
    }
},

renderBrush: function() {
    if (this.isBrushModeActive) {
        const halfBrushSize = this.brushRadius / 2;
        const topLeftX = this.mouseX - halfBrushSize;
        const topLeftY = this.mouseY - halfBrushSize;
        game.ctx.fillStyle = 'rgba(0, 0, 255, 0.5)';
        game.ctx.fillRect(topLeftX, topLeftY, this.brushRadius, this.brushRadius); 
    }
},

deleteSelectedObjects: function () {
    if (this.selectedObjects.length === 0) {
        return;
    }

    this.pushToUndoStack();

    game.roomData.items = game.roomData.items.filter(item => {
        return !this.selectedObjects.includes(item);
    });

    this.selectedObjects.forEach(obj => {
        const layerId = `item_${obj.layer_id}`;
        editor_layers.removeLayerById(layerId);
    });

    this.selectedObjects = [];

    if (this.previousMode === 'lasso') {
        this.changeMode('lasso');
    } else {
        this.changeMode('select');
    }
},

updateSelectedObjects: function (shiftKeyHeld, isClick = false, clickedCoords = null) {
    if (game.editorMode === 'pan') {
        return;
    }

    const isSingleClick = isClick || (this.selectionStart.x === this.selectionEnd.x && this.selectionStart.y === this.selectionEnd.y);

    if (isSingleClick) {
        const clickedX = clickedCoords ? clickedCoords.x : this.selectionStart.x;
        const clickedY = clickedCoords ? clickedCoords.y : this.selectionStart.y;

        const affectedObjects = game.roomData.items.filter(roomItem => {
            if (this.isObjectLocked(roomItem)) return false;

            const itemData = game.objectData[roomItem.id];
            if (!itemData || itemData.length === 0) return false;

            const xCoordinates = roomItem.x || [];
            const yCoordinates = roomItem.y || [];

            const itemBounds = {
                x: Math.min(...xCoordinates) * 16,
                y: Math.min(...yCoordinates) * 16,
                width: (Math.max(...xCoordinates) - Math.min(...xCoordinates) + 1) * 16,
                height: (Math.max(...yCoordinates) - Math.min(...yCoordinates) + 1) * 16,
            };

            return (
                clickedX >= itemBounds.x &&
                clickedX < itemBounds.x + itemBounds.width &&
                clickedY >= itemBounds.y &&
                clickedY < itemBounds.y + itemBounds.height
            );
        });

        const topmostObject = affectedObjects.length > 0 ? affectedObjects[0] : null;

        if (topmostObject) {
            if (shiftKeyHeld) {
                const index = this.selectedObjects.indexOf(topmostObject);
                if (index === -1) {
                    this.selectedObjects.push(topmostObject);
                } else {
                    this.selectedObjects.splice(index, 1);
                }
            } else {
                this.selectedObjects = [topmostObject];
            }
        }
    } else {
        const selectionRect = {
            x: Math.min(this.selectionStart.x, this.selectionEnd.x),
            y: Math.min(this.selectionStart.y, this.selectionEnd.y),
            width: Math.abs(this.selectionEnd.x - this.selectionStart.x),
            height: Math.abs(this.selectionEnd.y - this.selectionStart.y),
        };

        const overlappingObjects = game.roomData.items.filter(item => {
            if (this.isObjectLocked(item)) return false;

            const itemRect = {
                x: Math.min(...item.x) * 16,
                y: Math.min(...item.y) * 16,
                width: (Math.max(...item.x) - Math.min(...item.x) + 1) * 16,
                height: (Math.max(...item.y) - Math.min(...item.y) + 1) * 16,
            };

            return (
                itemRect.x < selectionRect.x + selectionRect.width &&
                itemRect.x + itemRect.width > selectionRect.x &&
                itemRect.y < selectionRect.y + selectionRect.height &&
                itemRect.y + itemRect.height > selectionRect.y
            );
        });

        if (shiftKeyHeld) {
            overlappingObjects.forEach(obj => {
                const index = this.selectedObjects.indexOf(obj);
                if (index === -1) {
                    this.selectedObjects.push(obj);
                } else {
                    this.selectedObjects.splice(index, 1);
                }
            });
        } else {
            this.selectedObjects = overlappingObjects;
        }
    }

    if (this.selectedObjects.length > 0) {
        this.changeMode('move');
    }

    const layerIds = this.selectedObjects.map(obj => "item_" + obj.layer_id);
    editor_layers.selectLayersById(layerIds);
},


renderObjectToCanvas: function (ctx, roomItem, tileData, xCoordinates, yCoordinates) {
    const tileSize = 16;
    let frameData = tileData.i;
    const isAnimated = Array.isArray(frameData[0][0]);
    const currentFrame = tileData.currentFrame || 0;

    if (isAnimated) {
        frameData = frameData[currentFrame % frameData.length];
    }

    const tileIndices = frameData.flatMap(entry => {
        if (typeof entry === 'string' && entry.includes('-')) {
            const [start, end] = entry.split('-').map(Number);
            return Array.from({ length: end - start + 1 }, (_, i) => start + i);
        }
        return entry;
    });

    const gridWidth = tileData.a + 1;
    const gridHeight = tileData.b + 1;
    const topLeftX = Math.min(...xCoordinates) * tileSize;
    const topLeftY = Math.min(...yCoordinates) * tileSize;
    let currentIndex = 0;

    for (let row = 0; row < gridHeight; row++) {
        for (let col = 0; col < gridWidth; col++) {
            const tileFrameIndex = tileIndices[currentIndex % tileIndices.length];
            currentIndex++;
            const srcX = (tileFrameIndex % 150) * tileSize;
            const srcY = Math.floor(tileFrameIndex / 150) * tileSize;
            const posX = col * tileSize;
            const posY = row * tileSize;

            ctx.drawImage(
                assets.use(tileData.t),
                srcX, srcY, tileSize, tileSize,
                posX, posY, tileSize, tileSize
            );
        }
    }
},


updateSelectedObjectsWithLasso: function (shiftKeyHeld) {
    const affectedObjects = game.roomData.items.filter(item => {
        if (this.isObjectLocked(item)) return false;

        const itemCenter = {
            x: (Math.min(...item.x) + Math.max(...item.x)) / 2 * 16,
            y: (Math.min(...item.y) + Math.max(...item.y)) / 2 * 16
        };

        return this.isPointInLasso(itemCenter);
    });

    if (shiftKeyHeld) {
        affectedObjects.forEach(obj => {
            const index = this.selectedObjects.indexOf(obj);
            if (index === -1) {
                this.selectedObjects.push(obj);
            } else {
                this.selectedObjects.splice(index, 1);
            }
        });
    } else {
        this.selectedObjects = affectedObjects;
    }

    if (this.selectedObjects.length > 0) {
        this.changeMode('move');
    }

    const layerIds = this.selectedObjects.map(obj => "item_" + obj.layer_id);
    editor_layers.selectLayersById(layerIds);
},

isPointInLasso: function (point) {
    let inside = false;
    const { x, y } = point;

    for (let i = 0, j = this.lassoPath.length - 1; i < this.lassoPath.length; j = i++) {
        const xi = this.lassoPath[i].x, yi = this.lassoPath[i].y;
        const xj = this.lassoPath[j].x, yj = this.lassoPath[j].y;

        const intersect = ((yi > y) !== (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi);
        if (intersect) inside = !inside;
    }

    return inside;
},

pushSelectedObjectsToTop: function () {
    if (this.selectedObjects.length === 0) {
        return;
    }

    this.pushToUndoStack();
    const items = game.roomData.items;

    this.selectedObjects.forEach(obj => {
        const index = items.indexOf(obj);
        if (index > -1) {
            items.splice(index, 1);
        }
    });

    items.push(...this.selectedObjects);
},

pushSelectedObjectsToBottom: function () {
    if (this.selectedObjects.length === 0) {
        return;
    }

    this.pushToUndoStack();
    const items = game.roomData.items;

    this.selectedObjects.forEach(obj => {
        const index = items.indexOf(obj);
        if (index > -1) {
            items.splice(index, 1);
        }
    });

    items.unshift(...this.selectedObjects);
},

spaceOutSelectedObjects: function () {
    if (this.selectedObjects.length <= 1) {
        return;
    }

    const spacingDistance = 48;

    let centerX = 0;
    let centerY = 0;

    this.selectedObjects.forEach(obj => {
        centerX += Math.min(...obj.x) * 16;
        centerY += Math.min(...obj.y) * 16;
    });

    centerX /= this.selectedObjects.length;
    centerY /= this.selectedObjects.length;

    this.selectedObjects.forEach((obj, index) => {
        const angle = (index / this.selectedObjects.length) * Math.PI * 2;
        let newX = centerX + Math.cos(angle) * spacingDistance;
        let newY = centerY + Math.sin(angle) * spacingDistance;

        newX = Math.round(newX);
        newY = Math.round(newY);

        const offsetX = (newX / 16) - Math.min(...obj.x);
        const offsetY = (newY / 16) - Math.min(...obj.y);

        obj.x = obj.x.map(coord => Math.round(coord + offsetX));
        obj.y = obj.y.map(coord => Math.round(coord + offsetY));
    });
},

selectAllObjects: function () {
    this.selectedObjects = game.roomData.items.slice();

    if (this.selectedObjects.length > 0) {
        this.changeMode('move');
    }
},

copySelectedObjects: function () {
        if (this.selectedObjects.length > 0) {
            this.clipboard = this.selectedObjects.map(obj => JSON.parse(JSON.stringify(obj)));
            console.log("Objects copied:", this.clipboard);
        }
    },

pasteCopiedObjects: function () {
    if (this.clipboard.length > 0) {
        const mouseX = this.mouseX;
        const mouseY = this.mouseY;
        const clipboardCenterX = this.clipboard.reduce((sum, obj) => sum + Math.min(...obj.x) * 16, 0) / this.clipboard.length;
        const clipboardCenterY = this.clipboard.reduce((sum, obj) => sum + Math.min(...obj.y) * 16, 0) / this.clipboard.length;
        const offsetX = mouseX - clipboardCenterX;
        const offsetY = mouseY - clipboardCenterY;
        const offsetForX = this.isSnapEnabled ? Math.floor(offsetX / 16) * 16 : Math.round(offsetX);
        const offsetForY = this.isSnapEnabled ? Math.floor(offsetY / 16) * 16 : Math.round(offsetY);

        const pastedObjects = this.clipboard.map(obj => {
            const newObj = JSON.parse(JSON.stringify(obj));
            const newLayerId = utils.generateId();
            newObj.layer_id = newLayerId;

            newObj.x = newObj.x.map(coord => {
                const newCoordX = coord * 16 + offsetForX;
                return this.isSnapEnabled ? Math.floor(newCoordX / 16) : newCoordX / 16;
            });

            newObj.y = newObj.y.map(coord => {
                const newCoordY = coord * 16 + offsetForY;
                return this.isSnapEnabled ? Math.floor(newCoordY / 16) : newCoordY / 16;
            });

            editor_layers.addItemToLayer({
                layer_id: newLayerId,
                n: newObj.name || "Pasted Item"
            });

            return newObj;
        });

        game.roomData.items.push(...pastedObjects);
        this.selectedObjects = pastedObjects;
        const pastedLayerIds = pastedObjects.map(obj => "item_" + obj.layer_id);
        editor_layers.selectLayersById(pastedLayerIds);
        this.renderSelectedTiles();
        this.changeMode('move');
    }
},

undo: function () {
    if (this.undoStack.length === 0) {
        console.log("Nothing to undo.");
        return;
    }

    this.pushToRedoStack();
    const lastState = this.undoStack.pop();
    this.restoreRoomData(lastState);

    console.log("Undo completed.");
},

redo: function () {
    if (this.redoStack.length === 0) {
        console.log("Nothing to redo.");
        return;
    }

    this.pushToUndoStack();
    const lastState = this.redoStack.pop();
    this.restoreRoomData(lastState);
},

saveRoomData: function () {
    const data = {
        sceneid: game.sceneid,
        roomData: game.roomData
    };
    const dataToSend = JSON.stringify(data);
    console.log('Data being sent to server:', dataToSend);

    ui.ajax({
        outputType: 'json',
        method: 'POST',
        url: 'plugins/editor/main/ajax/save_scene.php',
        data: dataToSend,
        headers: { 'Content-Type': 'application/json' },
        success: function (data) {
            plugin.close('edit_mode_window');
            collision.createWalkableGrid();
        },
        error: function (data) {
            console.error('Error saving room data:', data);
        }
    });
},

revertToOriginalState: function () {
    if (this.originalRoomData) {
        game.roomData = JSON.parse(JSON.stringify(this.originalRoomData));
        collision.createWalkableGrid();
    } else {
        console.error("Original room data not found.");
    }
},

constrainCamera: function () {
        const scaledWindowWidth = window.innerWidth / game.zoomLevel;
        const scaledWindowHeight = window.innerHeight / game.zoomLevel;

        const maxCameraX = game.worldWidth - scaledWindowWidth;
        camera.cameraX = Math.max(0, Math.min(camera.cameraX, maxCameraX));

        const maxCameraY = game.worldHeight - scaledWindowHeight;
        camera.cameraY = Math.max(0, Math.min(camera.cameraY, maxCameraY));
    },

    pushToUndoStack: function () {
    const currentState = JSON.parse(JSON.stringify(game.roomData));
    this.undoStack.push(currentState);
    console.log("Undo stack pushed, current undo stack size:", this.undoStack.length);
    this.redoStack = [];
},

pushToRedoStack: function () {
    const currentState = JSON.parse(JSON.stringify(game.roomData));
    this.redoStack.push(currentState);
    console.log("Redo stack pushed, current redo stack size:", this.redoStack.length);
},

restoreRoomData: function (state) {
    if (!state) {
        console.error("Error: Attempting to restore an undefined state.");
        return;
    }

    try {
        game.roomData = JSON.parse(JSON.stringify(state));
        collision.createWalkableGrid();
    } catch (error) {
        console.error("Error restoring room data:", error);
    }
},

    clearSelectionBox: function () {
    this.selectionStart = { x: 0, y: 0 };
    this.selectionEnd = { x: 0, y: 0 };
    game.render();
},

clearLassoPath: function () {
    this.lassoPath = [];
    game.render();
},

panCameraHorizontally: function(deltaY) {
    const scrollDirection = deltaY < 0 ? -1 : 1;
    camera.cameraX += scrollDirection * 10;
    this.constrainCamera();
},

isModeWithZoomOrMovement: function() {
    return ['select', 'lasso', 'move', 'zoom', 'delete'].includes(game.editorMode);
},

handleZoomOrMovement: function(deltaY, event) {
    camera.lerpEnabled = false;
    camera.manual = true;

    const rect = game.canvas.getBoundingClientRect();
    const mouseXBeforeZoom = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
    const mouseYBeforeZoom = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;
    const zoomFactor = deltaY < 0 ? 1 : -1;
    const newZoomLevel = Math.max(1, Math.min(game.zoomLevel + zoomFactor, 10));

    game.zoomLevel = newZoomLevel;
    const mouseXAfterZoom = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
    const mouseYAfterZoom = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;

    camera.cameraX += mouseXBeforeZoom - mouseXAfterZoom;
    camera.cameraY += mouseYBeforeZoom - mouseYAfterZoom;

    this.constrainCamera();
    game.resizeCanvas();
},

panCameraVertically: function(deltaY) {
    const scrollDirection = deltaY < 0 ? -1 : 1;
    camera.cameraY += scrollDirection * 10;
    this.constrainCamera();
},

zoomInBrushMode: function(deltaY, event) {
    const scrollDirection = deltaY < 0 ? 1 : -1;
    const newZoomLevel = Math.max(2, Math.min(game.zoomLevel + scrollDirection, 10));

    const rect = game.canvas.getBoundingClientRect();
    const mouseXBeforeZoom = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
    const mouseYBeforeZoom = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;

    game.zoomLevel = newZoomLevel;
    const mouseXAfterZoom = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
    const mouseYAfterZoom = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;

    camera.cameraX += mouseXBeforeZoom - mouseXAfterZoom;
    camera.cameraY += mouseYBeforeZoom - mouseYAfterZoom;

    this.constrainCamera();
},

adjustBrushSize: function(deltaY) {
    const scrollDirection = deltaY < 0 ? 5 : -5;
    this.brushRadius = Math.max(16, Math.min(this.brushRadius + scrollDirection, 500));
},

findConnectedClusters: function() {
    const clusters = [];
    const visited = new Set();

    this.selectedObjects.forEach(object => {
        if (!visited.has(object)) {
            const cluster = [];
            this.dfsFindCluster(object, cluster, visited);
            clusters.push(cluster);
        }
    });

    return clusters;
},

dfsFindCluster: function(object, cluster, visited) {
    visited.add(object);
    cluster.push(object);

    this.selectedObjects.forEach(otherObject => {
        if (!visited.has(otherObject) && this.areObjectsConnected(object, otherObject)) {
            this.dfsFindCluster(otherObject, cluster, visited);
        }
    });
},

areObjectsConnected: function(obj1, obj2) {
    const obj1MinX = Math.min(...obj1.x) * 16;
    const obj1MinY = Math.min(...obj1.y) * 16;
    const obj1MaxX = Math.max(...obj1.x) * 16 + 16;
    const obj1MaxY = Math.max(...obj1.y) * 16 + 16;
    const obj2MinX = Math.min(...obj2.x) * 16;
    const obj2MinY = Math.min(...obj2.y) * 16;
    const obj2MaxX = Math.max(...obj2.x) * 16 + 16;
    const obj2MaxY = Math.max(...obj2.y) * 16 + 16;
    const connected = obj1MaxX >= obj2MinX && obj1MinX <= obj2MaxX && obj1MaxY >= obj2MinY && obj1MinY <= obj2MaxY;
    return connected;
},

handleKeyDown: function (event) {
    const key = event.key;
    const activeElement = document.activeElement;
    if (
        activeElement.tagName === 'INPUT' ||
        activeElement.tagName === 'TEXTAREA' ||
        activeElement.tagName === 'SELECT' ||
        activeElement.isContentEditable
    ) {
        return;
    }

    switch (key) {
        case '1':
            this.changeMode('select');
            break;
        case '2':
            this.changeMode('brush');
            break;
        case '3':
            this.changeMode('zoom');
            break;
        case '4':
            this.changeMode('delete');
            break;
        case '5':
            this.changeMode('pan');
            break;
        case '6':
            this.changeMode('lasso');
            break;
        case '7':
            this.changeMode('move');
            break;
        default:
            break;
    }

    if (key === 'Escape') {
        this.selectedObjects = [];
        this.clearSelectionBox();
        this.clearLassoPath();
        console.log('All selections cleared.');
        this.changeMode('select');
    } else if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(key)) {
        this.moveSelectedObjectsWithArrowKeys(key);
        event.preventDefault();
    } else if (key === 'Delete' || key === 'Backspace') {
        this.deleteSelectedObjects();
        event.preventDefault();
    }
},

handleKeyUp: function (event) {
    const key = event.key;
    const activeElement = document.activeElement;
    if (
        activeElement.tagName === 'INPUT' ||
        activeElement.tagName === 'TEXTAREA' ||
        activeElement.tagName === 'SELECT' ||
        activeElement.isContentEditable
    ) {
        return;
    }

    if (key === 'Shift' && (game.editorMode === 'select' || game.editorMode === 'lasso')) {
        this.changeMode('move');
    }
}
};
</script>

<?php
}
?>