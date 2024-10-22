<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
<div data-window='editor_window' class='window window_bg fixed top-2 right-2 rounded-sm' style='width: 57px;background: #3a445b;'>

<!-- Handle that spans the whole left side -->
<div data-part='handle' class='window_title rounded-none w-full mb-1' style='height: 15px; background-image: radial-gradient(#e5e5e58a 1px, transparent 0) !important; border-radius: 0;'>
</div>

<!-- Rest of the content -->
<div class='relative flex-grow'>
    <div class='container text-light window_body px-1 py-1'>

    <button type="button" id="select_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1 relative" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
    <div class="ui_icon ui_select"></div>
    <span class="absolute top-0 right-0 transform translate-x-1 -translate-y-1 text-white text-xs px-1 py-0.5">1</span>  <!-- Badge for shortcut number 1 -->
</button>

<button type="button" id="brush_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1 relative" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
    <div class="ui_icon ui_brush"></div>
    <span class="absolute top-0 right-0 transform translate-x-1 -translate-y-1 text-white text-xs px-1 py-0.5">2</span>  <!-- Badge for shortcut number 2 -->
</button>

<button type="button" id="zoom_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1 relative" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
    <div class="ui_icon ui_magnify"></div>
    <span class="absolute top-0 right-0 transform translate-x-1 -translate-y-1 text-white text-xs px-1 py-0.5">3</span>  <!-- Badge for shortcut number 3 -->
</button>

<button type="button" id="delete_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1 relative" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
    <div class="ui_icon ui_delete"></div>
    <span class="absolute top-0 right-0 transform translate-x-1 -translate-y-1 text-white text-xs px-1 py-0.5">4</span>  <!-- Badge for shortcut number 4 -->
</button>

<button type="button" id="pan_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1 relative" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
    <div class="ui_icon ui_pan"></div>
    <span class="absolute top-0 right-0 transform translate-x-1 -translate-y-1 text-white text-xs px-1 py-0.5">5</span>  <!-- Badge for shortcut number 5 -->
</button>

<button type="button" id="lasso_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1 relative" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
    <div class="ui_icon ui_lasso"></div>
    <span class="absolute top-0 right-0 transform translate-x-1 -translate-y-1 text-white text-xs px-1 py-0.5">6</span>  <!-- Badge for shortcut number 6 -->
</button>

<button type="button" id="move_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1 relative" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
    <div class="ui_icon ui_move"></div>
    <span class="absolute top-0 right-0 transform translate-x-1 -translate-y-1 text-white text-xs px-1 py-0.5">7</span>  <!-- Badge for shortcut number 7 -->
</button>
        <button type="button" id="undo_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1 relative" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;" onclick="edit_mode_window.undo()">
            <div class="ui_icon ui_undo"></div>
        </button>

        <button type="button" id="redo_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1 relative" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;" onclick="edit_mode_window.redo()">
            <div class="ui_icon ui_redo"></div>
        </button>

        <button type="button" id="save_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline relative" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;" onclick="edit_mode_window.saveRoomData()">
            <div class="ui_icon ui_save"></div>
        </button>

        <button type="button" id="close_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline relative" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;" onclick="edit_mode_window.revertToOriginalState(); edit_mode_window.unmount(); modal.close('editor_window')">
            <div class="ui_icon ui_close"></div>
        </button>
    </div>
</div>

  <style>
    body.move-cursor {
      cursor: move !important;
    }
  </style>

  <script>
var edit_mode_window = {
    originalRoomData: JSON.parse(JSON.stringify(game.roomData)),
    modeButtons: {},
    brushRadius: 16,  // Initial brush radius in pixels
    isBrushModeActive: false,
    isPanning: false,  // Flag for tracking if the pan mode is active
    isMiddleClickPanning: false,  // Flag for panning via middle mouse click
    mouseX: 0,  // Track mouse X position
    mouseY: 0,  // Track mouse Y position
    lastMouseX: 0,  // Track the last mouse position for panning
    lastMouseY: 0,  // Track the last mouse position for panning
    defaultCursor: 'default',  // Store the default cursor for the current mode
    previousMode: null,
    modes: ['select', 'brush', 'zoom', 'delete', 'pan', 'lasso', 'move'],
    lassoPath: [],  // Stores the points of the lasso path
    isLassoActive: false,  // Tracks if the lasso is being drawn
    boundMouseMoveHandler: null,
    boundMouseDownHandler: null,
    boundMouseUpHandler: null,
    boundMouseScrollHandler: null,
    boundKeyDownHandler: null,
    clipboard: [],
    isAddingNewObject: false,
    isDragging: false,  // Flag to track if selection drag is active
    selectionStart: { x: 0, y: 0 },  // Coordinates where drag selection starts
    selectionEnd: { x: 0, y: 0 },  // Coordinates where drag selection ends
    selectedObjects: [],  // Stores objects inside the selected area
    undoStack: [],  // Stack for undo operations
    redoStack: [],  // Stack for redo operations
    isMovingObjects: false,
    moveOffsetX: 0,
    moveOffsetY: 0,

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

    // Attach click handlers to each button
    this.modeButtons.brush.addEventListener('click', () => this.changeMode('brush'));
    this.modeButtons.select.addEventListener('click', () => this.changeMode('select'));
    this.modeButtons.zoom.addEventListener('click', () => this.changeMode('zoom'));
    this.modeButtons.delete.addEventListener('click', () => this.changeMode('delete'));
    this.modeButtons.pan.addEventListener('click', () => this.changeMode('pan'));
    this.modeButtons.lasso.addEventListener('click', () => this.changeMode('lasso'));
    this.modeButtons.move.addEventListener('click', () => this.changeMode('move'));

    // Hide the main sprite
    game.displaySprite = false;

    // Set the time to 12:00 noon and stop time updates
    game.gameTime.hours = 12;  // Set to 12 noon
    game.gameTime.minutes = 0;  // Set minutes to 0
    game.timeActive = false;  // Stop time updates

    // Disable game pathfinding and enable editor mode
    game.isEditMode = true;
    game.pathfinding = false;
    game.allowControls = false;
    camera.lerpEnabled = false;
    camera.manual = true;
    game.zoomLevel = 2;

    game.mainSprite.stopPathfinding();

    this.changeMode('select');  // Default mode

    // Store bound event handlers
    this.boundMouseMoveHandler = this.handleMouseMove.bind(this);
    this.boundMouseDownHandler = this.handleMouseDown.bind(this);
    this.boundMouseUpHandler = this.handleMouseUp.bind(this);
    this.boundMouseScrollHandler = this.handleMouseScroll.bind(this);
    this.boundKeyDownHandler = this.handleKeyDown.bind(this);

    // Add mouse move and scroll event listeners
    game.canvas.addEventListener('mousemove', this.boundMouseMoveHandler);
    game.canvas.addEventListener('mousedown', this.boundMouseDownHandler);
    game.canvas.addEventListener('mouseup', this.boundMouseUpHandler);
    game.canvas.addEventListener('wheel', this.boundMouseScrollHandler);
    window.addEventListener('keyup', this.handleKeyUp.bind(this));
    
    // Add keyboard listener for switching modes
    window.addEventListener('keydown', this.boundKeyDownHandler);

    modal.minimize('ui_inventory_window');
    //modal.minimize('console_window');
    modal.minimize('ui_overlay_window');
    //modal.minimize('ui_footer_window');
},

unmount: function () {
    console.log('Editor unmounted, game and weather restored, and scene reloaded.');

    // Restore game controls and state
    game.isEditMode = false;
    game.pathfinding = true;
    game.allowControls = true;
    game.gameTime.hours = 0;  // Reset to 0
    game.gameTime.minutes = 0;
    game.timeActive = true;
    game.displaySprite = true;  // Show the main sprite
    camera.lerpEnabled = true;
    camera.manual = false;
    game.zoomLevel = localStorage.getItem('zoomLevel') ? parseInt(localStorage.getItem('zoomLevel')) : 4;

    // Remove all event listeners using stored references
    game.canvas.removeEventListener('mousemove', this.boundMouseMoveHandler);
    game.canvas.removeEventListener('mousedown', this.boundMouseDownHandler);
    game.canvas.removeEventListener('mouseup', this.boundMouseUpHandler);
    game.canvas.removeEventListener('wheel', this.boundMouseScrollHandler);
    window.removeEventListener('keydown', this.boundKeyDownHandler);
    window.removeEventListener('keyup', this.boundKeyUpHandler);  // Unmount keyup event listener as well

    // Reset selection states and flags
    this.isDragging = false;
    this.isLassoActive = false;
    this.selectedObjects = [];
    this.lassoPath = [];
    this.selectionStart = { x: 0, y: 0 };
    this.selectionEnd = { x: 0, y: 0 };
    this.isMovingObjects = false;
    this.isMiddleClickPanning = false;

    // Clear selection visuals
    this.clearSelectionBox();
    this.clearLassoPath();

    // Restore the weather system
    weather.createFireflys();
    weather.createRain(0.7);
    weather.createSnow(0.2);

    // Reset the cursor to default
    document.body.style.cursor = 'default';

    // Show minimized windows
    modal.show('ui_inventory_window');
    modal.show('console_window');
    modal.show('ui_overlay_window');
    modal.show('ui_footer_window');

    console_window.load_tab_buttons();
    modal.close('editor_utils_window');
},

changeMode: function (newMode) {
    // Only store the previous mode when switching from 'select' or 'lasso' to 'move'
    if (newMode === 'move') {
        if (game.editorMode === 'select' || game.editorMode === 'lasso') {
            this.previousMode = game.editorMode;  // Store the exact previous mode (either 'select' or 'lasso')
        }
    }

    // Clear selection box render when switching to 'lasso' mode
    if (newMode === 'lasso') {
        this.clearSelectionBox();  // Clear any selection box if switching to lasso mode
    }

    // Change the editor mode to the new mode
    game.editorMode = newMode;

    // Reset button styles and apply selected mode style
    Object.values(this.modeButtons).forEach(button => {
        button.style.background = '#4f618b';
        button.style.color = 'white';
    });

    if (this.modeButtons[newMode]) {
        this.modeButtons[newMode].style.background = 'white';
        this.modeButtons[newMode].style.color = '#276b49';
    }

    // Set cursor and mode-specific flags
    this.isBrushModeActive = (newMode === 'brush');
    this.isMovingObjects = (newMode === 'move');
    this.isPanning = (newMode === 'pan');

    // Show or hide the brush size input based on the active mode, only if the element exists
    const brushSizeInput = document.getElementById('brush_amount');
    if (brushSizeInput && brushSizeInput.parentElement) {
        if (this.isBrushModeActive) {
            brushSizeInput.parentElement.style.display = 'flex';
        } else {
            brushSizeInput.parentElement.style.display = 'none';
        }
    }

    // Update the cursor based on the current mode
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
},


handleMouseDown: function (event) {
    if (edit_mode_window.isAddingNewObject) return;  // Prevent any mouse down action when adding a new object

    // Continue with the usual mouse down logic
    this.updateMousePosition(event);

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

    if (event.button === 0 && game.editorMode === 'pan') {
        this.isPanning = true;
        this.lastMouseX = event.clientX;
        this.lastMouseY = event.clientY;
        return;
    }

    if (event.button !== 0) return;

    if (game.editorMode === 'zoom') {
        this.changeMode('select');
    }

    if (game.editorMode === 'select') {
        this.handleSelectionStart(event);
    } else if (game.editorMode === 'move') {
        this.handleMoveMode(event);
    } else if (game.editorMode === 'lasso') {
        this.handleLassoStart(event);
    }
},

handleMouseMove: function (event) {
    const rect = game.canvas.getBoundingClientRect();
    this.mouseX = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
    this.mouseY = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;

    // Check if middle mouse button is being dragged to prevent selection
    if ((event.buttons === 4 || event.buttons === 1) && this.isPanning) {  // Only pan if mouse is pressed
        this.handleCameraPanning(event); // Handle panning when middle or left mouse is used
        return; // Exit early to avoid triggering selection or other modes
    }

    // Only trigger the selection logic if left mouse button is pressed (event.buttons === 1)
    if (this.isDragging && event.buttons === 1) {
        if (game.editorMode === 'move' && !event.shiftKey) {
            this.handleObjectMovement();  // Moving selected objects
        } else if (game.editorMode === 'select') {
            this.handleSelectionBox(event, rect);  // Dragging selection box
        } else if (game.editorMode === 'lasso') {
            this.handleLassoDragging();  // Dragging lasso
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

handleMouseScroll: function(event) {
    const shiftKeyPressed = event.shiftKey;
    const ctrlKeyPressed = event.ctrlKey;
    const deltaY = event.deltaY;

    // If Shift is held down, pan the camera left/right
    if (shiftKeyPressed) {
        this.panCameraHorizontally(deltaY);
    } 
    // Handle zooming and camera movement for specific modes
    else if (this.isModeWithZoomOrMovement()) {
        this.handleZoomOrMovement(deltaY, event);
    } 
    // Pan the camera vertically in pan mode
    else if (game.editorMode === 'pan') {
        this.panCameraVertically(deltaY);
    } 
    // Handle brush mode for changing brush size or zooming
    else if (game.editorMode === 'brush') {
        this.handleBrushModeScroll(deltaY, ctrlKeyPressed, event);
    }
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

handleObjectMovement: function() {
    let totalDeltaX = this.mouseX - this.lastMouseX;
    let totalDeltaY = this.mouseY - this.lastMouseY;
    let snapDeltaX = 0;
    let snapDeltaY = 0;

    if (editor_utils_window.isSnapEnabled) {
        if (Math.abs(totalDeltaX) >= 16) {
            snapDeltaX = Math.floor(totalDeltaX / 16) * 16;
            this.lastMouseX += snapDeltaX;
        }
        if (Math.abs(totalDeltaY) >= 16) {
            snapDeltaY = Math.floor(totalDeltaY / 16) * 16;
            this.lastMouseY += snapDeltaY;
        }
    } else {
        snapDeltaX = totalDeltaX;
        snapDeltaY = totalDeltaY;
        this.lastMouseX = this.mouseX;
        this.lastMouseY = this.mouseY;
    }

    if (this.selectedObjects.length > 0) {
        this.selectedObjects.forEach((obj, index) => {
            obj.x = obj.x.map((coord) => editor_utils_window.isSnapEnabled ? Math.round(coord + snapDeltaX / 16) : coord + snapDeltaX / 16);
            obj.y = obj.y.map((coord) => editor_utils_window.isSnapEnabled ? Math.round(coord + snapDeltaY / 16) : coord + snapDeltaY / 16);
        });
        this.constrainCamera();
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
    const step = editor_utils_window.isSnapEnabled ? 16 : 1;

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

renderSelectedTiles: function() {
    if (this.selectedObjects.length > 0) {
        const clusters = this.findConnectedClusters();

        // Iterate over each cluster and draw a border around it
        clusters.forEach(cluster => {
            // Initialize the min and max coordinates with the first object's coordinates in the cluster
            let minX = Math.min(...cluster[0].x) * 16;
            let minY = Math.min(...cluster[0].y) * 16;
            let maxX = Math.max(...cluster[0].x) * 16;
            let maxY = Math.max(...cluster[0].y) * 16;

            // Iterate over all objects in the cluster to find the min and max coordinates for the bounding box
            cluster.forEach(selectedObject => {
                const objectMinX = Math.min(...selectedObject.x) * 16;
                const objectMinY = Math.min(...selectedObject.y) * 16;
                const objectMaxX = Math.max(...selectedObject.x) * 16;
                const objectMaxY = Math.max(...selectedObject.y) * 16;

                // Update the bounding box coordinates
                minX = Math.min(minX, objectMinX);
                minY = Math.min(minY, objectMinY);
                maxX = Math.max(maxX, objectMaxX);
                maxY = Math.max(maxY, objectMaxY);
            });

            // Calculate the width and height of the bounding box
            const width = (maxX - minX) + 16;  // Add 16 to account for tile size
            const height = (maxY - minY) + 16;

            // Save canvas state before applying styles
            game.ctx.save();

            // Apply shadow to the selected cluster
            game.ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
            game.ctx.shadowBlur = 6;
            game.ctx.shadowOffsetX = 4;
            game.ctx.shadowOffsetY = 4;

            // Static dashed border for the group of selected objects
            game.ctx.strokeStyle = 'rgba(255, 255, 255, 0.9)';  // White dashed border
            game.ctx.lineWidth = 1;
            game.ctx.setLineDash([4, 2]);  // Static dashed line pattern: 4px dash, 2px gap
            game.ctx.lineDashOffset = 0;   // Explicitly set to 0 to prevent animation
            game.ctx.strokeRect(minX, minY, width, height);  // Draw the border around the cluster

            // Restore canvas state after rendering
            game.ctx.restore();
        });
    }
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
    const selectionRect = {
        x: Math.min(this.selectionStart.x, this.selectionEnd.x),
        y: Math.min(this.selectionStart.y, this.selectionEnd.y),
        width: Math.abs(this.selectionEnd.x - this.selectionStart.x),
        height: Math.abs(this.selectionEnd.y - this.selectionStart.y)
    };

    const affectedObjects = game.roomData.items.filter(item => {
        const itemRect = {
            x: Math.min(...item.x) * 16,
            y: Math.min(...item.y) * 16,
            width: (Math.max(...item.x) - Math.min(...item.x) + 1) * 16,
            height: (Math.max(...item.y) - Math.min(...item.y) + 1) * 16
        };

        return (
            itemRect.x < selectionRect.x + selectionRect.width &&
            itemRect.x + itemRect.width > selectionRect.x &&
            itemRect.y < selectionRect.y + selectionRect.height &&
            itemRect.y + itemRect.height > selectionRect.y
        );
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
    editor_utils_window.toggleBringButtons(this.selectedObjects.length > 0);

    // Removed the code that moves objects to the front of the array.
    if (this.selectedObjects.length > 0) {
        this.changeMode('move');
    }
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
    editor_utils_window.toggleBringButtons(this.selectedObjects.length > 0);

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
        const offsetForX = editor_utils_window.isSnapEnabled ? Math.floor(offsetX / 16) * 16 : Math.round(offsetX);
        const offsetForY = editor_utils_window.isSnapEnabled ? Math.floor(offsetY / 16) * 16 : Math.round(offsetY);

        // Deep clone the copied objects and adjust their position relative to the mouse cursor
        const pastedObjects = this.clipboard.map(obj => {
            const newObj = JSON.parse(JSON.stringify(obj));

            // Adjust each object's x and y coordinates
            newObj.x = newObj.x.map(coord => {
                const newCoordX = coord * 16 + offsetForX;
                // If snapping is enabled, snap to the nearest grid. If not, use exact positioning (no grid snapping).
                return editor_utils_window.isSnapEnabled ? Math.floor(newCoordX / 16) : newCoordX / 16;
            });

            newObj.y = newObj.y.map(coord => {
                const newCoordY = coord * 16 + offsetForY;
                // If snapping is enabled, snap to the nearest grid. If not, use exact positioning (no grid snapping).
                return editor_utils_window.isSnapEnabled ? Math.floor(newCoordY / 16) : newCoordY / 16;
            });

            return newObj;
        });

        // Add the pasted objects to the room data
        game.roomData.items.push(...pastedObjects);
        console.log("Objects pasted at", editor_utils_window.isSnapEnabled ? 'grid-snapped' : 'pixel-perfect (rounded)', "positions:", pastedObjects);

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
        url: 'modals/editor/ajax/save_map.php',
        data: dataToSend,
        headers: {
            'Content-Type': 'application/json'
        },
        success: function (data) {
            console.log('Room data saved successfully:', data);
            
            // After a successful save, close the editor modal
            edit_mode_window.unmount(); // Call the unmount function to clean up
            modal.close('editor_window'); // Close the editor window
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
    const newZoomLevel = Math.max(2, Math.min(game.zoomLevel + zoomFactor, 10));  // Set minimum zoom level to 2

    game.zoomLevel = newZoomLevel;
    const mouseXAfterZoom = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
    const mouseYAfterZoom = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;

    camera.cameraX += mouseXBeforeZoom - mouseXAfterZoom;
    camera.cameraY += mouseYBeforeZoom - mouseYAfterZoom;

    this.constrainCamera();
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

    // Switch to zoom mode while holding down the Ctrl key
    if (event.ctrlKey && game.editorMode !== 'zoom') {
        this.previousMode = game.editorMode;  // Store the current mode
        this.changeMode('zoom');  // Switch to zoom mode
        return;
    }

    // Handle shortcuts for 1 to 7 keys
    switch (key) {
        case '1':
            this.changeMode('select');  // Select mode
            break;
        case '2':
            this.changeMode('brush');  // Brush mode
            break;
        case '3':
            this.changeMode('zoom');  // Zoom mode
            break;
        case '4':
            this.changeMode('delete');  // Delete mode
            break;
        case '5':
            this.changeMode('pan');  // Pan mode
            break;
        case '6':
            this.changeMode('lasso');  // Lasso mode
            break;
        case '7':
            this.changeMode('move');  // Move mode
            break;
        default:
            break;
    }

    // Ctrl + A to select all objects
    if (event.ctrlKey && key === 'a') {
        this.selectAllObjects();
        event.preventDefault();
    }
    // Ctrl + C to copy selected objects
    else if (event.ctrlKey && key === 'c') {
        this.copySelectedObjects();
    }
    // Ctrl + V to paste copied objects
    else if (event.ctrlKey && key === 'v') {
        this.pasteCopiedObjects();
    }
    // Ctrl + Z to undo
    else if (event.ctrlKey && !event.shiftKey && key === 'z') {
        this.undo();
    }
    // Ctrl + Shift + Z to redo
    else if (event.ctrlKey && event.shiftKey && key === 'Z') {
        this.redo();
    }
    // Ctrl + G to toggle grid
    else if (event.ctrlKey && !event.shiftKey && key.toLowerCase() === 'g') {
        editor_utils_window.toggleGridCheckbox();
        event.preventDefault();
    }
    // Ctrl + Shift + G to toggle snap
    else if (event.ctrlKey && event.shiftKey && key.toLowerCase() === 'g') {
        editor_utils_window.toggleSnapCheckbox();
        event.preventDefault();
    }
    // Ctrl + Arrow Up to move selected objects to the top
    else if (event.ctrlKey && key === 'ArrowUp') {
        this.pushSelectedObjectsToTop();
        event.preventDefault();
    }
    // Ctrl + Arrow Down to move selected objects to the bottom
    else if (event.ctrlKey && key === 'ArrowDown') {
        this.pushSelectedObjectsToBottom();
        event.preventDefault();
    }
    // Arrow keys to move selected objects
    else if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(key)) {
        this.moveSelectedObjectsWithArrowKeys(key);
        event.preventDefault();
    }
    // Ctrl + Shift + S to space out selected objects
    else if (event.ctrlKey && event.shiftKey && key.toLowerCase() === 's') {
        this.spaceOutSelectedObjects();
        event.preventDefault();
    }
    // Delete or Backspace to delete selected objects
    else if (key === 'Delete' || key === 'Backspace') {
        this.deleteSelectedObjects();
        event.preventDefault();
    }
},

handleKeyUp: function (event) {
    const key = event.key;

    // Revert to the previous mode when releasing the Ctrl key
    if (key === 'Control' && game.editorMode === 'zoom') {
        this.changeMode(this.previousMode);  // Restore the previous mode
        return;
    }

    // Shift key up - return to 'move' mode only if the previous mode was 'move'
    if (key === 'Shift' && (game.editorMode === 'select' || game.editorMode === 'lasso')) {
        this.changeMode('move');  // Return to move mode when Shift is released
    }
}

};

// Start the map editor window
edit_mode_window.start();


  </script>

</div>
<?php
}
?>