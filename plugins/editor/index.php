<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>

<style>
    body.move-cursor {
      cursor: move !important;
    }
  </style>

  <script>
edit_mode_window = {
    renderMode: 'isometric',
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

    game.timeActive = false;
    game.isEditMode = true;
    game.pathfinding = false;
    game.allowControls = true;
    camera.lerpEnabled = false;
    game.zoomLevel = 4;
    actions.tooltipActive = false;
    game.mainSprite.stopPathfinding();
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
        { priority: 1, options: { id: 'editor_context_menu_window', url: 'editor/context_menu/index.php', drag: false, reload: true } },
        { priority: 2, options: { id: 'ui_footer_window', url: 'ui/dev/index.php', drag: false, reload: false } }
    ]);

    setTimeout(() => { camera.manual = true; }, 0);
    this.changeMode('select');

},

unmount: function () {
    console.log('Editor unmounted, game and weather restored, and scene reloaded.');

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

    plugin.load({ id: 'context_menu_window', url: 'ui/menus/context_menu/index.php', name: 'Context Menu', drag: false,reload: true });
    plugin.close('editor_context_menu_window');
    plugin.close('console_window');
    plugin.close('ui_footer_window');
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
        console.log("Mode change prevented while dragging objects.");
        return;
    }

    if (game.editorMode === newMode) return;

    document.body.style.cursor = 'default';
    game.editorMode = newMode;

    switch (newMode) {
        case 'select':
            this.defaultCursor = 'pointer';
            break;
        case 'move':
            this.defaultCursor = 'move';
            break;
        case 'lasso':
            this.defaultCursor = 'crosshair';
            break;
        default:
            this.defaultCursor = 'default';
    }

    document.body.style.cursor = this.defaultCursor;

    this.isBrushModeActive = (newMode === 'brush');
    this.isMovingObjects = (newMode === 'move');
    this.isPanning = (newMode === 'pan');

    const brushSizeInput = document.getElementById('brush_amount');
    if (brushSizeInput && brushSizeInput.parentElement) {
        if (this.isBrushModeActive) {
            brushSizeInput.parentElement.style.display = 'flex';
        } else {
            brushSizeInput.parentElement.style.display = 'none';
        }
    }

    if (newMode === 'lasso') {
        this.clearSelectionBox();
    }

    if (newMode === 'move' && (this.previousMode === 'select' || this.previousMode === 'lasso')) {
        this.previousMode = game.editorMode;
    }
},

handleMouseDown: function (event) {
    if (edit_mode_window.isAddingNewObject) return;

    this.updateMousePosition(event);

    if (event.button === 2) {
        const isClickInsideSelection = this.isCursorInsideSelectedArea();

        if (!isClickInsideSelection) {
            this.selectedObjects = [];
            this.clearSelectionBox();
            this.clearLassoPath();
            console.log('All selections cleared with right click.');
            this.changeMode('select');
        } else {
            console.log('Right-click on selected object. No deselection performed.');
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
        if (event.shiftKey && game.editorMode === 'move' && this.selectedObjects.length > 0) {
            const clickedObject = this.selectedObjects.find(obj => {
                const objRect = {
                    x: Math.min(...obj.x) * 16,
                    y: Math.min(...obj.y) * 16,
                    width: (Math.max(...obj.x) - Math.min(...obj.x) + 1) * 16,
                    height: (Math.max(...obj.y) - Math.min(...obj.y) + 1) * 16,
                };

                return (
                    this.mouseX >= objRect.x &&
                    this.mouseX <= objRect.x + objRect.width &&
                    this.mouseY >= objRect.y &&
                    this.mouseY <= objRect.y + objRect.height
                );
            });

            if (clickedObject) {
                this.selectedObjects = this.selectedObjects.filter(obj => obj !== clickedObject);
                console.log('Deselected object:', clickedObject);

                if (this.selectedObjects.length === 0) {
                    this.changeMode('select');
                }

                this.renderSelectedTiles();
                return;
            }
        }

        if (game.editorMode === 'pan') {
            this.isPanning = true;
            this.lastMouseX = event.clientX;
            this.lastMouseY = event.clientY;
        } else if (game.editorMode === 'zoom') {
            this.changeMode('select');
        } else if (game.editorMode === 'select') {
            this.handleSelectionStart(event);
        } else if (game.editorMode === 'move') {
            this.handleMoveMode(event);
        } else if (game.editorMode === 'lasso') {
            this.handleLassoStart(event);
        }
    }
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


handleMouseMove: function (event) {
    const rect = game.canvas.getBoundingClientRect();
    this.mouseX = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
    this.mouseY = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;

    // Handle middle mouse dragging
    if (event.buttons === 4 && this.isMiddleClickPanning) { // Middle mouse button
        const deltaX = event.clientX - this.lastMouseX;
        const deltaY = event.clientY - this.lastMouseY;

        // Current canvas position
        const canvasLeft = parseInt(game.canvas.style.left || '0', 10);
        const canvasTop = parseInt(game.canvas.style.top || '0', 10);

        // Check if any side of the camera is at the edge
        const atLeftEdge = camera.cameraX <= 0;
        const atRightEdge = camera.cameraX >= game.worldWidth - (window.innerWidth / game.zoomLevel);
        const atTopEdge = camera.cameraY <= 0;
        const atBottomEdge = camera.cameraY >= game.worldHeight - (window.innerHeight / game.zoomLevel);

        const isAtHorizontalEdge = atLeftEdge || atRightEdge;
        const isAtVerticalEdge = atTopEdge || atBottomEdge;

        // If the scene is not at an edge, pan the scene
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
            return; // Exit early to prioritize scene panning
        }

        // Otherwise, drag the canvas element
        game.canvas.style.left = `${canvasLeft + deltaX}px`;
        game.canvas.style.top = `${canvasTop + deltaY}px`;

        // If dragging back, check if the canvas is re-entering the viewport
        const isCanvasOutsideLeft = canvasLeft + deltaX < 0;
        const isCanvasOutsideTop = canvasTop + deltaY < 0;
        const isCanvasOutsideRight = canvasLeft + deltaX + rect.width > window.innerWidth;
        const isCanvasOutsideBottom = canvasTop + deltaY + rect.height > window.innerHeight;

        if (
            (isCanvasOutsideLeft && deltaX > 0) ||
            (isCanvasOutsideTop && deltaY > 0) ||
            (isCanvasOutsideRight && deltaX < 0) ||
            (isCanvasOutsideBottom && deltaY < 0)
        ) {
            // Allow the canvas to move back into the viewport
            this.lastMouseX = event.clientX;
            this.lastMouseY = event.clientY;
            return; // Do not resume scene panning yet
        }

        // Resume scene panning when the canvas is fully back in the viewport
        if (
            !isCanvasOutsideLeft &&
            !isCanvasOutsideTop &&
            !isCanvasOutsideRight &&
            !isCanvasOutsideBottom
        ) {
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
        }

        // Update last mouse position for smooth dragging
        this.lastMouseX = event.clientX;
        this.lastMouseY = event.clientY;
        return; // Exit after processing canvas dragging
    }

    // Handle pan mode (separate from middle mouse dragging)
    if (game.editorMode === 'pan' && this.isPanning && event.buttons === 1) {
        const deltaX = event.clientX - this.lastMouseX;
        const deltaY = event.clientY - this.lastMouseY;

        // Update camera position for panning
        camera.cameraX -= deltaX / game.zoomLevel;
        camera.cameraY -= deltaY / game.zoomLevel;
        this.constrainCamera(); // Ensure camera stays within bounds

        // Update last mouse position for smooth panning
        this.lastMouseX = event.clientX;
        this.lastMouseY = event.clientY;

        return; // Exit early to prevent triggering other actions
    }

    // Dynamically determine the cursor and mode
    if (game.editorMode === 'move' || game.editorMode === 'select') {
        const cursorInsideSelection = this.isCursorInsideSelectedArea();
        if (cursorInsideSelection && game.editorMode !== 'move') {
            this.changeMode('move'); // Switch to 'move' mode
        } else if (!cursorInsideSelection && game.editorMode === 'move') {
            this.changeMode('select'); // Switch to 'select' mode
        }
    }

    // Handle dragging or other operations
    if (this.isDragging && event.buttons === 1) {
        if (game.editorMode === 'move' && !event.shiftKey) {
            this.handleObjectMovement();
        } else if (game.editorMode === 'select') {
            this.handleSelectionBox(event, rect);
        } else if (game.editorMode === 'lasso') {
            this.handleLassoDragging();
        }
    }
},



handleMouseUp: function (event) {
    // Handle middle mouse button release to stop panning and restore mode
    if (event.button === 1 && this.isMiddleClickPanning) {  // Middle mouse button
        this.isMiddleClickPanning = false;
        this.isPanning = false;  // Stop panning when middle mouse is released
        this.changeMode(this.previousMode);  // Restore the previous mode
        document.body.style.cursor = this.defaultCursor;  // Restore the cursor
        return;
    }

    if (event.button === 0 && this.isPanning) {
        this.isPanning = false;  // Stop panning when left mouse button is released
        document.body.style.cursor = this.defaultCursor;  // Restore the cursor
        return;
    }

    // Other button actions
    if (event.button !== 0) return;

    if (this.handleSelectionEnd(event)) return;
    if (game.editorMode === 'move') {
        this.finalizeObjectMovement();
    }
},

handleMouseScroll: function (event) {
    if (!game.isEditMode) return;

    // Determine zoom direction and new zoom level
    const zoomFactor = event.deltaY < 0 ? 1 : -1;
    const newZoomLevel = Math.max(2, Math.min(game.zoomLevel + zoomFactor, 10)); // Min 2, Max 10

    // Get the canvas position and cursor location
    const rect = game.canvas.getBoundingClientRect();
    const mouseXOnCanvas = event.clientX - rect.left;
    const mouseYOnCanvas = event.clientY - rect.top;

    // Calculate the world coordinates before zoom
    const worldMouseX = mouseXOnCanvas / game.zoomLevel + camera.cameraX;
    const worldMouseY = mouseYOnCanvas / game.zoomLevel + camera.cameraY;

    // Apply the new zoom level
    game.zoomLevel = newZoomLevel;

    // Calculate the world coordinates after zoom
    const newWorldMouseX = mouseXOnCanvas / game.zoomLevel + camera.cameraX;
    const newWorldMouseY = mouseYOnCanvas / game.zoomLevel + camera.cameraY;

    // Adjust the camera position to maintain focus on the cursor
    camera.cameraX += worldMouseX - newWorldMouseX;
    camera.cameraY += worldMouseY - newWorldMouseY;

    // Ensure the camera stays within bounds
    this.constrainCamera();

    // Trigger a re-render
    game.render();
},


handlePanning: function (event) {
    if (event.button === 1 || game.editorMode === 'pan') {
        this.isDragging = true;
        this.isPanning = true;
        this.lastMouseX = event.clientX;
        this.lastMouseY = event.clientY;
        document.body.style.cursor = 'grabbing';
        return true; // Exit early after setting panning mode
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
        return true; // Exit early to handle zoom dragging
    }
    return false;
},

handleObjectMovement: function () {
    const totalDeltaX = this.mouseX - this.lastMouseX;
    const totalDeltaY = this.mouseY - this.lastMouseY;

    if (this.selectedObjects.length > 0) {
        this.selectedObjects.forEach((obj) => {
            // Log and remove associated lights
            lighting.lights = lighting.lights.filter(light => {
                const objectLightIdPrefix = `${obj.id}_`;
                const isLightRelated = light.id.startsWith(objectLightIdPrefix);

                if (isLightRelated) {
                    console.log(`Removing light: ${light.id}`);
                }

                return !isLightRelated; // Keep only lights that are not related to this object
            });

            // Move the object positions
            obj.x = obj.x.map(coord => coord + totalDeltaX / 16);
            obj.y = obj.y.map(coord => coord + totalDeltaY / 16);

            // Lights will be re-added by handleLights after movement
        });

        // Update mouse position for continuous movement
        this.lastMouseX = this.mouseX;
        this.lastMouseY = this.mouseY;
    }
},

handleSelectionStart: function (event) {
    if (edit_mode_window.isAddingNewObject) return;  // Prevent selection when adding a new object

    // Continue with the usual selection logic
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

        // New panning code starts here
        const edgeThreshold = 50; // Distance from the edge of the viewport to trigger panning
        const viewportWidth = window.innerWidth;
        const viewportHeight = window.innerHeight;

        const canvasStyle = game.canvas.style;
        const canvasLeft = parseInt(canvasStyle.left || '0', 10);
        const canvasTop = parseInt(canvasStyle.top || '0', 10);

        // Check if the drag is near the edges of the window viewport
        if (event.clientX < edgeThreshold) {
            if (camera.cameraX > 0) {
                camera.cameraX -= 5; // Pan left
            } else {
                game.canvas.style.left = `${Math.min(canvasLeft + 5, 0)}px`; // Move canvas left
            }
        } else if (event.clientX > viewportWidth - edgeThreshold) {
            const maxCameraX = game.worldWidth - viewportWidth / game.zoomLevel;
            if (camera.cameraX < maxCameraX) {
                camera.cameraX += 5; // Pan right
            } else {
                game.canvas.style.left = `${Math.max(canvasLeft - 5, viewportWidth - rect.width)}px`; // Move canvas right
            }
        }

        if (event.clientY < edgeThreshold) {
            if (camera.cameraY > 0) {
                camera.cameraY -= 5; // Pan up
            } else {
                game.canvas.style.top = `${Math.min(canvasTop + 5, 0)}px`; // Move canvas up
            }
        } else if (event.clientY > viewportHeight - edgeThreshold) {
            const maxCameraY = game.worldHeight - viewportHeight / game.zoomLevel;
            if (camera.cameraY < maxCameraY) {
                camera.cameraY += 5; // Pan down
            } else {
                game.canvas.style.top = `${Math.max(canvasTop - 5, viewportHeight - rect.height)}px`; // Move canvas down
            }
        }

        this.constrainCamera(); // Ensure the camera stays within bounds
        // New panning code ends here
    }
},

handleLassoStart: function (event) {
    if (event.button === 0) {
        this.isDragging = true;
        this.lassoPath = [{ x: this.mouseX, y: this.mouseY }];
        return true; // Exit early after starting lasso path
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
                // Convert to pixel space, round to nearest pixel, and back to object space
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
        console.log("No objects selected to move.");
        return;
    }

    // Define the movement step size
    const step = this.isSnapEnabled ? 16 : 1;

    // Determine movement direction
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

    // Move the selected objects
    this.selectedObjects.forEach(obj => {
        obj.x = obj.x.map(coord => coord + deltaX / 16);  // Move on X axis
        obj.y = obj.y.map(coord => coord + deltaY / 16);  // Move on Y axis
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
        // Save canvas state before rendering
        game.ctx.save();

        const animationSpeed = 300; // Slower animation for subtle effect
        const markerLength = 5; // Length of the L shape
        const lineWidth = 2; // Width of the lines
        const shadowOffset = 1; // Offset for shadow effect
        const paddingFactor = 0.15; // Padding as a percentage of object size (10%)

        game.ctx.lineWidth = lineWidth;

        // Define a fixed palette of vibrant colors
        const vibrantColors = [
            'rgb(255, 0, 0)',    // Red
            'rgb(0, 255, 0)',    // Green
            'rgb(0, 0, 255)',    // Blue
            'rgb(255, 255, 0)',  // Yellow
            'rgb(255, 0, 255)',  // Magenta
            'rgb(0, 255, 255)',  // Cyan
            'rgb(255, 165, 0)',  // Orange
            'rgb(128, 0, 128)'   // Purple
        ];

        // Assign a color for each object
        if (!this.objectColors || this.objectColors.length !== this.selectedObjects.length) {
            this.objectColors = this.selectedObjects.map((_, index) => {
                return vibrantColors[index % vibrantColors.length]; // Cycle through the palette
            });
        }

        this.selectedObjects.forEach((obj, index) => {
            // Assign color for the current object
            const objectColor = this.objectColors[index];

            // Calculate object dimensions
            const objWidth = Math.max(...obj.x) - Math.min(...obj.x) + 1;
            const objHeight = Math.max(...obj.y) - Math.min(...obj.y) + 1;

            // Calculate padding relative to object size
            const paddingX = objWidth * 16 * paddingFactor;
            const paddingY = objHeight * 16 * paddingFactor;

            // Calculate padded boundaries
            const minX = Math.min(...obj.x) * 16 + paddingX;
            const minY = Math.min(...obj.y) * 16 + paddingY;
            const maxX = Math.max(...obj.x) * 16 + 16 - paddingX;
            const maxY = Math.max(...obj.y) * 16 + 16 - paddingY;

            // Define L-shape markers for each corner
            const corners = [
                { x1: minX, y1: minY, x2: minX + markerLength, y2: minY, x3: minX, y3: minY + markerLength, dx: 1, dy: 1 },
                { x1: maxX, y1: minY, x2: maxX - markerLength, y2: minY, x3: maxX, y3: minY + markerLength, dx: -1, dy: 1 },
                { x1: minX, y1: maxY, x2: minX + markerLength, y2: maxY, x3: minX, y3: maxY - markerLength, dx: 1, dy: -1 },
                { x1: maxX, y1: maxY, x2: maxX - markerLength, y2: maxY, x3: maxX, y3: maxY - markerLength, dx: -1, dy: -1 }
            ];

            // Calculate animation offset (subtle movement)
            const timeOffset = performance.now() / animationSpeed;
            const offset = Math.sin(timeOffset) * 2; // Smaller amplitude for subtle animation

            corners.forEach(corner => {
                // Apply diagonal animation offset based on direction (dx, dy)
                const animatedX = corner.dx * offset;
                const animatedY = corner.dy * offset;

                // Add shadow effect
                game.ctx.strokeStyle = 'rgba(0, 0, 0, 0.5)'; // Shadow color
                game.ctx.beginPath();
                // Horizontal shadow
                game.ctx.moveTo(corner.x1 + animatedX + shadowOffset, corner.y1 + animatedY + shadowOffset);
                game.ctx.lineTo(corner.x2 + animatedX + shadowOffset, corner.y2 + animatedY + shadowOffset);
                // Vertical shadow
                game.ctx.moveTo(corner.x1 + animatedX + shadowOffset, corner.y1 + animatedY + shadowOffset);
                game.ctx.lineTo(corner.x3 + animatedX + shadowOffset, corner.y3 + animatedY + shadowOffset);
                game.ctx.stroke();

                // Draw main L-shape with unique vibrant color
                game.ctx.strokeStyle = objectColor; // Use vibrant object-specific color
                game.ctx.beginPath();
                // Horizontal part of the L
                game.ctx.moveTo(corner.x1 + animatedX, corner.y1 + animatedY);
                game.ctx.lineTo(corner.x2 + animatedX, corner.y2 + animatedY);
                // Vertical part of the L
                game.ctx.moveTo(corner.x1 + animatedX, corner.y1 + animatedY);
                game.ctx.lineTo(corner.x3 + animatedX, corner.y3 + animatedY);
                game.ctx.stroke();
            });
        });

        // Restore canvas state
        game.ctx.restore();
    }
},




isTopmostObject: function (obj, selectedObjects) {
    const objIndex = game.roomData.items.indexOf(obj);

    // Check if this object is the topmost among overlapping objects
    return selectedObjects.every(otherObj => {
        if (obj === otherObj) return true; // Skip self
        const otherIndex = game.roomData.items.indexOf(otherObj);

        // If another object overlaps but is higher in the render stack, this object is not topmost
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

        // Save canvas state before applying styles
        game.ctx.save();

        // White semi-transparent fill for selection box
        game.ctx.fillStyle = 'rgba(255, 255, 255, 0.2)';
        game.ctx.fillRect(rect.x, rect.y, rect.width, rect.height);

        // Apply shadow to the selection box
        game.ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
        game.ctx.shadowBlur = 8;
        game.ctx.shadowOffsetX = 4;
        game.ctx.shadowOffsetY = 4;

        // Animate dashed lines for the selection box
        const dashSpeed = performance.now() / 100;  // Adjust speed by dividing time
        game.ctx.lineDashOffset = -dashSpeed;  // Move dashes forward for animation

        // White dashed border for selection box
        game.ctx.strokeStyle = 'rgba(255, 255, 255, 0.9)';
        game.ctx.lineWidth = 2;
        game.ctx.setLineDash([6, 3]);  // Dash pattern: 6px line, 3px gap
        game.ctx.strokeRect(rect.x, rect.y, rect.width, rect.height);

        // Restore canvas state after rendering
        game.ctx.restore();
    }
},

renderLasso: function () {
    if (this.lassoPath.length > 1) {
        const dashSpeed = performance.now() / 100;  // Adjust speed by dividing time

        game.ctx.strokeStyle = 'rgba(255, 255, 255, 0.9)';  // White dashed line
        game.ctx.lineWidth = 2;
        game.ctx.setLineDash([6, 3]);  // Dash pattern: 6px line, 3px gap
        game.ctx.lineDashOffset = -dashSpeed;  // Animate dashes by offsetting

        game.ctx.beginPath();
        game.ctx.moveTo(this.lassoPath[0].x, this.lassoPath[0].y);

        for (let i = 1; i < this.lassoPath.length; i++) {
            game.ctx.lineTo(this.lassoPath[i].x, this.lassoPath[i].y);
        }

        game.ctx.stroke();

        // Reset line dash settings for other elements
        game.ctx.setLineDash([]);
        game.ctx.lineDashOffset = 0;
    }
},

renderBrush: function() {
    if (this.isBrushModeActive) {
        const halfBrushSize = this.brushRadius / 2;
        const topLeftX = this.mouseX - halfBrushSize;
        const topLeftY = this.mouseY - halfBrushSize;

        game.ctx.fillStyle = 'rgba(0, 0, 255, 0.5)';  // Set brush color and transparency
        game.ctx.fillRect(topLeftX, topLeftY, this.brushRadius, this.brushRadius);  // Draw square brush
    }
},

deleteSelectedObjects: function () {
    if (this.selectedObjects.length === 0) {
        console.log("No objects selected to delete.");
        return;
    }

    // Add current room state to undo stack before deleting
    this.pushToUndoStack();

    // Filter out the selected objects from the room data
    game.roomData.items = game.roomData.items.filter(item => {
        return !this.selectedObjects.includes(item);
    });

    // Clear the selected objects array
    this.selectedObjects = [];

    console.log("Selected objects deleted.");

    // Switch back to select or lasso mode based on the previous mode
    if (this.previousMode === 'lasso') {
        this.changeMode('lasso');
    } else {
        this.changeMode('select'); // Default to select mode if not in lasso
    }
},

updateSelectedObjects: function (shiftKeyHeld) {
    const isSingleClick = this.selectionStart.x === this.selectionEnd.x && this.selectionStart.y === this.selectionEnd.y;

    if (isSingleClick) {
        const clickedX = this.selectionStart.x;
        const clickedY = this.selectionStart.y;

        const affectedObjects = game.roomData.items.filter(roomItem => {
            const itemData = game.objectData[roomItem.id];
            if (!itemData || itemData.length === 0) return false;

            const tileData = itemData[0];
            const xCoordinates = roomItem.x || [];
            const yCoordinates = roomItem.y || [];

            const itemBounds = {
                x: Math.min(...xCoordinates) * 16,
                y: Math.min(...yCoordinates) * 16,
                width: (Math.max(...xCoordinates) - Math.min(...xCoordinates) + 1) * 16,
                height: (Math.max(...yCoordinates) - Math.min(...yCoordinates) + 1) * 16,
            };

            // Check if the click is within the object bounds
            if (
                clickedX < itemBounds.x ||
                clickedX >= itemBounds.x + itemBounds.width ||
                clickedY < itemBounds.y ||
                clickedY >= itemBounds.y + itemBounds.height
            ) {
                return false;
            }

            // Create an offscreen canvas
            const offscreenCanvas = document.createElement('canvas');
            offscreenCanvas.width = itemBounds.width;
            offscreenCanvas.height = itemBounds.height;
            const ctx = offscreenCanvas.getContext('2d');

            // Render the object on the offscreen canvas
            this.renderObjectToCanvas(ctx, roomItem, tileData, xCoordinates, yCoordinates);

            // Log the rendered object as a Base64 URL for debugging
            console.log('Rendered Object Base64 URL:', offscreenCanvas.toDataURL());

            // Translate click position to local coordinates
            const localX = clickedX - itemBounds.x;
            const localY = clickedY - itemBounds.y;

            // Get the pixel data at the clicked position
            const pixelData = ctx.getImageData(localX, localY, 1, 1).data;

            // Check if the clicked pixel is not transparent
            return pixelData[3] > 0;
        });

        // Sort by rendering order (topmost last in the array)
        affectedObjects.sort((a, b) => {
            return game.roomData.items.indexOf(b) - game.roomData.items.indexOf(a);
        });

        // Select the topmost object only (or add it with Shift key)
        const topmostObject = affectedObjects.length > 0 ? affectedObjects[0] : null;

        if (!shiftKeyHeld) {
            this.selectedObjects = topmostObject ? [topmostObject] : [];
        } else if (topmostObject && !this.selectedObjects.includes(topmostObject)) {
            this.selectedObjects.push(topmostObject);
        }
    } else {
        // Handle drag selection (unchanged)
        const selectionRect = {
            x: Math.min(this.selectionStart.x, this.selectionEnd.x),
            y: Math.min(this.selectionStart.y, this.selectionEnd.y),
            width: Math.abs(this.selectionEnd.x - this.selectionStart.x),
            height: Math.abs(this.selectionEnd.y - this.selectionStart.y),
        };

        const overlappingObjects = game.roomData.items.filter(item => {
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

        this.selectedObjects = shiftKeyHeld
            ? [...new Set([...this.selectedObjects, ...overlappingObjects])]
            : overlappingObjects;
    }

    if (this.selectedObjects.length > 0) {
        this.changeMode('move');
    }
},

renderObjectToCanvas: function (ctx, roomItem, tileData, xCoordinates, yCoordinates) {
    const tileSize = 16;

    // Parse the tile indices from the `i` field
    let frameData = tileData.i;
    const isAnimated = Array.isArray(frameData[0][0]); // Check if the object is animated
    const currentFrame = tileData.currentFrame || 0;

    // If animated, get the current frame's tile indices
    if (isAnimated) {
        frameData = frameData[currentFrame % frameData.length]; // Loop through frames
    }

    // Flatten the frame data to get all tile indices
    const tileIndices = frameData.flatMap(entry => {
        if (typeof entry === 'string' && entry.includes('-')) {
            const [start, end] = entry.split('-').map(Number);
            return Array.from({ length: end - start + 1 }, (_, i) => start + i);
        }
        return entry;
    });

    // Correct the grid dimensions (add 1 to `a` and `b` to account for 0-based indexing)
    const gridWidth = tileData.a + 1;
    const gridHeight = tileData.b + 1;

    // Calculate the top-left corner of the object
    const topLeftX = Math.min(...xCoordinates) * tileSize;
    const topLeftY = Math.min(...yCoordinates) * tileSize;

    // Log debugging information
    console.log("Object Data:", tileData);
    console.log("Tile Indices:", tileIndices);
    console.log("Corrected Grid Dimensions (Width x Height):", gridWidth, "x", gridHeight);
    console.log("Object Coordinates (Top Left X, Y):", topLeftX, topLeftY);
    console.log("Animation Frame Data:", isAnimated ? `Frame ${currentFrame}` : "Not animated");

    // Track the current index in the tileIndices array
    let currentIndex = 0;

    for (let row = 0; row < gridHeight; row++) {
        for (let col = 0; col < gridWidth; col++) {
            // Get the tile index for the current grid position
            const tileFrameIndex = tileIndices[currentIndex % tileIndices.length];
            currentIndex++;

            // Calculate source coordinates in the sprite sheet
            const srcX = (tileFrameIndex % 150) * tileSize;
            const srcY = Math.floor(tileFrameIndex / 150) * tileSize;

            // Calculate the position in the object's local grid
            const posX = col * tileSize;
            const posY = row * tileSize;

            // Log detailed mapping for debugging
            console.log(`Rendering Tile: Index ${tileFrameIndex}, Source (X: ${srcX}, Y: ${srcY}), Position (X: ${posX}, Y: ${posY})`);

            // Draw the tile on the canvas
            ctx.drawImage(
                assets.use(tileData.t), // Ensure this is the correct sprite sheet
                srcX, srcY, tileSize, tileSize,
                posX, posY, tileSize, tileSize
            );
        }
    }

    // Log the Base64 output for debugging
    console.log('Rendered Object Base64 URL:', ctx.canvas.toDataURL());
},


updateSelectedObjectsWithLasso: function (shiftKeyHeld) {
    const affectedObjects = game.roomData.items.filter(item => {
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

    // Show or hide the bring buttons based on whether any objects are selected
    //editor_utils_window.toggleBringButtons(this.selectedObjects.length > 0);

    if (this.selectedObjects.length > 0 && !shiftKeyHeld) {
        this.changeMode('move');
    }

    console.log('Selected objects:', this.selectedObjects);
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
        console.log('No objects selected to move.');
        return;
    }

    // Save the current state to the undo stack before making changes
    this.pushToUndoStack();

    const items = game.roomData.items;

    // Remove the selected objects from the array
    this.selectedObjects.forEach(obj => {
        const index = items.indexOf(obj);
        if (index > -1) {
            items.splice(index, 1); // Remove object from current position
        }
    });

    // Add the selected objects to the top of the array
    items.push(...this.selectedObjects);

    console.log('Selected objects moved to the top of the render queue.');
},

pushSelectedObjectsToBottom: function () {
    if (this.selectedObjects.length === 0) {
        console.log('No objects selected to move.');
        return;
    }

    // Save the current state to the undo stack before making changes
    this.pushToUndoStack();

    const items = game.roomData.items;

    // Remove the selected objects from the array
    this.selectedObjects.forEach(obj => {
        const index = items.indexOf(obj);
        if (index > -1) {
            items.splice(index, 1); // Remove object from current position
        }
    });

    // Add the selected objects to the bottom of the array
    items.unshift(...this.selectedObjects);

    console.log('Selected objects moved to the bottom of the render queue.');
},

spaceOutSelectedObjects: function () {
    if (this.selectedObjects.length <= 1) {
        console.log('Need more than one object to space out.');
        return;
    }

    // Set the spacing distance in pixels, slightly increased for more separation
    const spacingDistance = 48; // Increased to 3 tiles apart

    // Calculate center position of all selected objects
    let centerX = 0;
    let centerY = 0;

    this.selectedObjects.forEach(obj => {
        centerX += Math.min(...obj.x) * 16;
        centerY += Math.min(...obj.y) * 16;
    });

    centerX /= this.selectedObjects.length;
    centerY /= this.selectedObjects.length;

    // Space out each selected object around the center position
    this.selectedObjects.forEach((obj, index) => {
        const angle = (index / this.selectedObjects.length) * Math.PI * 2;
        let newX = centerX + Math.cos(angle) * spacingDistance;
        let newY = centerY + Math.sin(angle) * spacingDistance;

        // Ensure new positions are whole numbers
        newX = Math.round(newX);
        newY = Math.round(newY);

        const offsetX = (newX / 16) - Math.min(...obj.x);
        const offsetY = (newY / 16) - Math.min(...obj.y);

        // Move each object to the new position
        obj.x = obj.x.map(coord => Math.round(coord + offsetX));
        obj.y = obj.y.map(coord => Math.round(coord + offsetY));
    });

    console.log('Objects spaced out on the map with whole number positions.');
},

selectAllObjects: function () {
    // Select all objects in the room
    this.selectedObjects = game.roomData.items.slice();  // Copy all items to selectedObjects
    console.log("All objects selected:", this.selectedObjects);

    // Switch to move mode since objects are now selected
    if (this.selectedObjects.length > 0) {
        this.changeMode('move');
    }
},

copySelectedObjects: function () {
        if (this.selectedObjects.length > 0) {
            // Deep clone the selected objects to the clipboard
            this.clipboard = this.selectedObjects.map(obj => JSON.parse(JSON.stringify(obj)));
            console.log("Objects copied:", this.clipboard);
        }
    },

    pasteCopiedObjects: function () {
    if (this.clipboard.length > 0) {
        // Use the current mouse position for placing the pasted objects
        const mouseX = this.mouseX;
        const mouseY = this.mouseY;

        // Determine the center of the clipboard objects
        const clipboardCenterX = this.clipboard.reduce((sum, obj) => sum + Math.min(...obj.x) * 16, 0) / this.clipboard.length;
        const clipboardCenterY = this.clipboard.reduce((sum, obj) => sum + Math.min(...obj.y) * 16, 0) / this.clipboard.length;

        // Calculate the offset to center the objects on the mouse cursor
        const offsetX = mouseX - clipboardCenterX;
        const offsetY = mouseY - clipboardCenterY;

        // If snapping is enabled, snap the offsets to the nearest grid size (16x16 grid), otherwise, use pixel-perfect positioning
        const offsetForX = this.isSnapEnabled ? Math.floor(offsetX / 16) * 16 : Math.round(offsetX);
        const offsetForY = this.isSnapEnabled ? Math.floor(offsetY / 16) * 16 : Math.round(offsetY);

        // Deep clone the copied objects and adjust their position relative to the mouse cursor
        const pastedObjects = this.clipboard.map(obj => {
            const newObj = JSON.parse(JSON.stringify(obj));

            // Adjust each object's x and y coordinates
            newObj.x = newObj.x.map(coord => {
                const newCoordX = coord * 16 + offsetForX;
                // If snapping is enabled, snap to the nearest grid. If not, use exact positioning (no grid snapping).
                return this.isSnapEnabled ? Math.floor(newCoordX / 16) : newCoordX / 16;
            });

            newObj.y = newObj.y.map(coord => {
                const newCoordY = coord * 16 + offsetForY;
                // If snapping is enabled, snap to the nearest grid. If not, use exact positioning (no grid snapping).
                return this.isSnapEnabled ? Math.floor(newCoordY / 16) : newCoordY / 16;
            });

            return newObj;
        });

        // Add the pasted objects to the room data
        game.roomData.items.push(...pastedObjects);
        console.log("Objects pasted at", this.isSnapEnabled ? 'grid-snapped' : 'pixel-perfect (rounded)', "positions:", pastedObjects);

        // Select the pasted objects
        this.selectedObjects = pastedObjects;
        console.log("Pasted objects are now selected:", this.selectedObjects);

        // Switch to 'move' mode after pasting
        this.changeMode('move');
    }
},

undo: function () {
    if (this.undoStack.length === 0) {
        console.log("Nothing to undo.");
        return;
    }

    // Push the current state to redo stack before undoing
    this.pushToRedoStack();

    // Restore the last state from the undo stack
    const lastState = this.undoStack.pop();
    this.restoreRoomData(lastState);

    console.log("Undo completed.");
},

redo: function () {
    if (this.redoStack.length === 0) {
        console.log("Nothing to redo.");
        return;
    }

    // Push the current state to undo stack before redoing
    this.pushToUndoStack();

    // Restore the last state from the redo stack
    const lastState = this.redoStack.pop();
    this.restoreRoomData(lastState);

    console.log("Redo completed.");
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
        url: 'plugins/editor/ajax/save_map.php',
        data: dataToSend,
        headers: {
            'Content-Type': 'application/json'
        },
        success: function (data) {
            console.log('Room data saved successfully:', data);
            
            // After a successful save, close the editor plugin
            plugin.close('edit_mode_window'); // Close the editor window
            collision.createWalkableGrid();
        },
        error: function (data) {
            console.error('Error saving room data:', data);
            // Optionally handle error UI or notifications here
        }
    });
},

revertToOriginalState: function () {
    if (this.originalRoomData) {
        // Restore the original room data
        game.roomData = JSON.parse(JSON.stringify(this.originalRoomData));

        // Recreate the walkable grid and re-render the map
        collision.createWalkableGrid();

        console.log("Room reverted to original state.");
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
    // Clear redo stack whenever a new action is performed
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
        return; // Prevent parsing an invalid state
    }

    try {
        game.roomData = JSON.parse(JSON.stringify(state));
        console.log("Restored room data:", game.roomData.items);  // Log the restored object positions

        // Update walkable grid and re-render the map
        collision.createWalkableGrid();
    } catch (error) {
        console.error("Error restoring room data:", error);
    }
},

    clearSelectionBox: function () {
    this.selectionStart = { x: 0, y: 0 };  // Reset the selection start coordinates
    this.selectionEnd = { x: 0, y: 0 };    // Reset the selection end coordinates
    game.render();  // Trigger a re-render to clear any visual artifacts from the selection box
},

clearLassoPath: function () {
    // Clear the lasso path if rendered
    this.lassoPath = [];
    game.render();  // Optionally trigger a re-render to clear any visual artifacts
},

// Helper functions
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
    const newZoomLevel = Math.max(1, Math.min(game.zoomLevel + zoomFactor, 10));  // Set minimum zoom level to 2

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
    const newZoomLevel = Math.max(2, Math.min(game.zoomLevel + scrollDirection, 10));  // Set minimum zoom level to 2

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
    this.brushRadius = Math.max(16, Math.min(this.brushRadius + scrollDirection, 500));  // Set minimum to 16 and maximum to 500 pixels
},

findConnectedClusters: function() {
    const clusters = [];
    const visited = new Set();  // To track objects that have been clustered

    this.selectedObjects.forEach(object => {
        if (!visited.has(object)) {
            // Start a new cluster
            const cluster = [];
            this.dfsFindCluster(object, cluster, visited);
            clusters.push(cluster);
        }
    });

    return clusters;
},

// Depth-first search to find all connected objects
dfsFindCluster: function(object, cluster, visited) {
    visited.add(object);
    cluster.push(object);

    // Find all objects that are connected to this one
    this.selectedObjects.forEach(otherObject => {
        if (!visited.has(otherObject) && this.areObjectsConnected(object, otherObject)) {
            this.dfsFindCluster(otherObject, cluster, visited);
        }
    });
},

// Check if two objects are connected by overlapping or adjacency
areObjectsConnected: function(obj1, obj2) {
    const obj1MinX = Math.min(...obj1.x) * 16;
    const obj1MinY = Math.min(...obj1.y) * 16;
    const obj1MaxX = Math.max(...obj1.x) * 16 + 16;
    const obj1MaxY = Math.max(...obj1.y) * 16 + 16;

    const obj2MinX = Math.min(...obj2.x) * 16;
    const obj2MinY = Math.min(...obj2.y) * 16;
    const obj2MaxX = Math.max(...obj2.x) * 16 + 16;
    const obj2MaxY = Math.max(...obj2.y) * 16 + 16;

    // Check if the bounding boxes of obj1 and obj2 overlap or touch
    const connected = obj1MaxX >= obj2MinX && obj1MinX <= obj2MaxX &&
                      obj1MaxY >= obj2MinY && obj1MinY <= obj2MaxY;

    return connected;
},

handleKeyDown: function (event) {
    const key = event.key;

    // Ignore key commands if focus is on a form input
    const activeElement = document.activeElement;
    if (
        activeElement.tagName === 'INPUT' ||
        activeElement.tagName === 'TEXTAREA' ||
        activeElement.tagName === 'SELECT' ||
        activeElement.isContentEditable
    ) {
        return; // Exit early to prevent executing editor keyboard commands
    }

    // Rest of the keyboard commands...
    if (event.ctrlKey && game.editorMode !== 'zoom') {
        this.previousMode = game.editorMode; // Store the current mode
        this.changeMode('zoom'); // Switch to zoom mode
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

    // Other shortcuts...
    if (key === 'Escape') {
        this.selectedObjects = [];
        this.clearSelectionBox();
        this.clearLassoPath();
        console.log('All selections cleared.');
        this.changeMode('select');
    } else if (event.ctrlKey && key === 'a') {
        this.selectAllObjects();
        event.preventDefault();
    } else if (event.ctrlKey && key === 'c') {
        this.copySelectedObjects();
    } else if (event.ctrlKey && key === 'v') {
        this.pasteCopiedObjects();
    } else if (event.ctrlKey && !event.shiftKey && key === 'z') {
        this.undo();
    } else if (event.ctrlKey && event.shiftKey && key === 'Z') {
        this.redo();
    } else if (event.ctrlKey && !event.shiftKey && key.toLowerCase() === 'g') {
        event.preventDefault();
    } else if (event.ctrlKey && event.shiftKey && key.toLowerCase() === 'g') {
        event.preventDefault();
    } else if (event.ctrlKey && key === 'ArrowUp') {
        this.pushSelectedObjectsToTop();
        event.preventDefault();
    } else if (event.ctrlKey && key === 'ArrowDown') {
        this.pushSelectedObjectsToBottom();
        event.preventDefault();
    } else if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(key)) {
        this.moveSelectedObjectsWithArrowKeys(key);
        event.preventDefault();
    } else if (event.ctrlKey && event.shiftKey && key.toLowerCase() === 's') {
        this.spaceOutSelectedObjects();
        event.preventDefault();
    } else if (key === 'Delete' || key === 'Backspace') {
        this.deleteSelectedObjects();
        event.preventDefault();
    }
},

handleKeyUp: function (event) {
    const key = event.key;

    // Ignore key commands if focus is on a form input
    const activeElement = document.activeElement;
    if (
        activeElement.tagName === 'INPUT' ||
        activeElement.tagName === 'TEXTAREA' ||
        activeElement.tagName === 'SELECT' ||
        activeElement.isContentEditable
    ) {
        return; // Exit early to prevent executing editor keyboard commands
    }

    // Revert to the previous mode when releasing the Ctrl key
    if (key === 'Control' && game.editorMode === 'zoom') {
        this.changeMode(this.previousMode);
        return;
    }

    // Shift key up - return to 'move' mode only if the previous mode was 'move'
    if (key === 'Shift' && (game.editorMode === 'select' || game.editorMode === 'lasso')) {
        this.changeMode('move');
    }
}

};
</script>

<?php
}
?>