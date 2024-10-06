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

        <button type="button" id="close_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline relative" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;" onclick="edit_mode_window.unmount(); modal.close('editor_window')">
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
    // New drag select properties
    isDragging: false,  // Flag to track if selection drag is active
    selectionStart: { x: 0, y: 0 },  // Coordinates where drag selection starts
    selectionEnd: { x: 0, y: 0 },  // Coordinates where drag selection ends
    selectedObjects: [],  // Stores objects inside the selected area

    undoStack: [],  // Stack for undo operations
    redoStack: [],  // Stack for redo operations

        // Properties for move mode
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

handleMouseMove: function (event) {
    const rect = game.canvas.getBoundingClientRect();
    this.mouseX = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
    this.mouseY = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;

    // Track if the camera moved
    const prevCameraX = camera.cameraX;
    const prevCameraY = camera.cameraY;

    // Handle moving selected objects without camera panning
    if (this.isDragging && game.editorMode === 'move' && !event.shiftKey) {
        const deltaX = this.mouseX - this.lastMouseX;
        const deltaY = this.mouseY - this.lastMouseY;

        // Move the selected objects if they are being dragged
        if (this.selectedObjects.length > 0) {
            this.selectedObjects.forEach((obj, index) => {
                const offset = this.initialOffsets[index];

                // Update object coordinates without camera movement
                obj.x = obj.x.map((coord) => coord + deltaX / 16);
                obj.y = obj.y.map((coord) => coord + deltaY / 16);
            });

            // Update the last mouse position for the next movement step
            this.lastMouseX = this.mouseX;
            this.lastMouseY = this.mouseY;

            this.constrainCamera();  // Ensure the camera stays within bounds
        }
    }

    // Handle panning when middle mouse button or pan mode is active
    if (this.isPanning && this.isDragging) {
        const deltaX = event.clientX - this.lastMouseX;
        const deltaY = event.clientY - this.lastMouseY;
        camera.cameraX -= deltaX / game.zoomLevel;
        camera.cameraY -= deltaY / game.zoomLevel;
        this.constrainCamera();

        this.lastMouseX = event.clientX;
        this.lastMouseY = event.clientY;
        return; // Prevent further execution for zoom or selection while panning
    }

    // Handle zoom dragging in zoom mode
    if (this.isDragging && game.editorMode === 'zoom') {
        const deltaY = event.clientY - this.lastMouseY;

        // Accumulate the zoom dragging distance
        this.cumulativeZoomDrag = (this.cumulativeZoomDrag || 0) + deltaY;

        // Only zoom after dragging beyond a threshold
        const zoomThreshold = 50;  // Set threshold to control sensitivity (higher value = less sensitive)
        if (Math.abs(this.cumulativeZoomDrag) > zoomThreshold) {
            const zoomFactor = this.cumulativeZoomDrag < 0 ? 1 : -1;  // Increment zoom by 1
            const newZoomLevel = Math.max(2, Math.min(game.zoomLevel + zoomFactor, 10));

            const mouseXBeforeZoom = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
            const mouseYBeforeZoom = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;
            game.zoomLevel = newZoomLevel;
            const mouseXAfterZoom = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
            const mouseYAfterZoom = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;

            camera.cameraX += mouseXBeforeZoom - mouseXAfterZoom;
            camera.cameraY += mouseYBeforeZoom - mouseYAfterZoom;

            this.constrainCamera();

            // Reset the cumulative drag after applying zoom
            this.cumulativeZoomDrag = 0;
        }

        this.lastMouseY = event.clientY;
        return;
    }

    // Handle lasso tool dragging
    if (this.isDragging && game.editorMode === 'lasso') {
        this.lassoPath.push({ x: this.mouseX, y: this.mouseY });
        this.renderLasso(); // Render the lasso path
        return;
    }

    // Handle drag selection or movement
    if (this.isDragging) {
        // Handle drag selection when Shift is held
        if (game.editorMode === 'select' || (game.editorMode === 'move' && event.shiftKey)) {
            this.selectionEnd = {
                x: (event.clientX - rect.left) / game.zoomLevel + camera.cameraX,
                y: (event.clientY - rect.top) / game.zoomLevel + camera.cameraY
            };

            this.renderSelectionBox();  // Render selection box
        }

        // Handle moving selected objects
        if (game.editorMode === 'move' && !event.shiftKey && this.selectedObjects.length > 0) {
            const deltaX = this.mouseX - this.lastMouseX;
            const deltaY = this.mouseY - this.lastMouseY;

            // Move the selected objects based on exact cursor movement
            this.selectedObjects.forEach((obj, index) => {
                const offset = this.initialOffsets[index];
                obj.x = obj.x.map((coord) => coord + deltaX / 16);
                obj.y = obj.y.map((coord) => coord + deltaY / 16);
            });

            // Update the last mouse position for the next movement step
            this.lastMouseX = this.mouseX;
            this.lastMouseY = this.mouseY;

            this.constrainCamera();  // Ensure the camera stays within bounds
        }
    }
},

handleMouseDown: function (event) {
    if (this.isAddingNewObject) {
            // Prevent selection of existing objects while adding a new object
            return;
        }
    const rect = game.canvas.getBoundingClientRect();
    this.mouseX = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
    this.mouseY = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;

    // Handle panning with middle mouse button or pan mode
    if (event.button === 1 || game.editorMode === 'pan') {
        this.isDragging = true;
        this.isPanning = true;
        this.lastMouseX = event.clientX;
        this.lastMouseY = event.clientY;
        document.body.style.cursor = 'grabbing';
        return; // Ensure no other actions (zoom/selection) are triggered
    }

    // Only handle left-click (button 0) for other modes
    if (event.button !== 0) {
        return; // Prevent selection or dragging if not left-click
    }

    // Handle zoom dragging (in zoom mode)
    if (game.editorMode === 'zoom' && event.button === 0) {
        this.isDragging = true;
        this.lastMouseY = event.clientY; // Track Y axis for zooming
        return; // Exit to handle zoom separately
    }

    // Handle lasso tool
    if (game.editorMode === 'lasso' && event.button === 0) {
        this.isDragging = true;
        this.lassoPath = [{ x: this.mouseX, y: this.mouseY }]; // Start the lasso path
        return;
    }

    // In move mode, handle object movement or switch to select mode on non-object
    if (game.editorMode === 'move' && event.button === 0) {
        const clickedOnSelectedObject = this.selectedObjects.some(obj => {
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

        if (!clickedOnSelectedObject) {
            // If clicked on non-object, switch to select mode and start selecting immediately
            this.changeMode('select');
            this.isDragging = true;
            this.selectionStart = {
                x: (event.clientX - rect.left) / game.zoomLevel + camera.cameraX,
                y: (event.clientY - rect.top) / game.zoomLevel + camera.cameraY
            };
            this.selectionEnd = { ...this.selectionStart };  // Initialize the selection area
        } else {
            // Continue with move logic if clicked on selected object
            this.isDragging = true;
            this.initialOffsets = [];

            this.selectedObjects.forEach(obj => {
                this.initialOffsets.push({
                    obj: obj,
                    offsetX: this.mouseX - obj.x[0] * 16,  // Store the offset
                    offsetY: this.mouseY - obj.y[0] * 16
                });
            });

            this.lastMouseX = this.mouseX;
            this.lastMouseY = this.mouseY;
            this.clearSelectionBox();  // Clear the selection box right when moving starts
        }
        return; // Exit after handling move logic
    }

    // In select mode, handle dragging selection box or shift-click to toggle
    if (game.editorMode === 'select' && event.button === 0) {
        this.isDragging = true;
        this.selectionStart = {
            x: (event.clientX - rect.left) / game.zoomLevel + camera.cameraX,
            y: (event.clientY - rect.top) / game.zoomLevel + camera.cameraY
        };
        this.selectionEnd = { ...this.selectionStart };

        if (!event.shiftKey) {
            // Clear current selection unless Shift is held
            this.selectedObjects = [];
        }
    }
},

handleMouseUp: function (event) {
    // Stop panning when the middle mouse button (button 1) is released
    if (event.button === 1 || game.editorMode === 'pan') {
        this.isPanning = false;
        this.isDragging = false;
        document.body.style.cursor = this.defaultCursor;
        this.stopSlidingCamera();  // Stop camera sliding
        return;
    }

    // Stop sliding camera when dragging ends
    this.stopSlidingCamera();

    // Only handle left-click (button 0) for other modes
    if (event.button !== 0) {
        return; // Prevent further actions if not left-click
    }

    // Handle selection completion in 'select' mode
    if (game.editorMode === 'select') {
        this.isDragging = false;
        this.updateSelectedObjects(event.shiftKey);
        if (this.selectedObjects.length > 0) {
            this.changeMode('move');  // Switch to move mode if objects are selected
        }
        // Clear the selection box after the selection is made
        this.clearSelectionBox();
    }

    // Handle lasso selection completion in 'lasso' mode
    if (game.editorMode === 'lasso' && this.lassoPath.length > 0) {
        this.isDragging = false;
        this.updateSelectedObjectsWithLasso(event.shiftKey);
        this.clearLassoPath();  // Clear the lasso path after the selection is made

        if (this.selectedObjects.length > 0 && !event.shiftKey) {
            this.changeMode('move');  // Switch to move mode if objects are selected
        }
    }

    // In move mode, finalize object movement
    if (game.editorMode === 'move') {
        this.isDragging = false;

        // After dragging, the final position of selected objects is locked in
        if (this.selectedObjects.length > 0) {
            this.pushToUndoStack();  // Add the current state to the undo stack after movement
        }
    }
},

handleMouseScroll: function(event) {
    // Check if Shift is held down to pan left/right
    if (event.shiftKey) {
        const scrollDirection = event.deltaY < 0 ? -1 : 1;
        camera.cameraX += scrollDirection * 10;
        this.constrainCamera();
    } 
    // Handle different modes based on the current editor mode
    else if (game.editorMode === 'select' || game.editorMode === 'lasso' || game.editorMode === 'move' || game.editorMode === 'zoom' || game.editorMode === 'delete') {
        camera.lerpEnabled = false;
        camera.manual = true;

        const rect = game.canvas.getBoundingClientRect();
        const mouseXBeforeZoom = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
        const mouseYBeforeZoom = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;
        const zoomFactor = event.deltaY < 0 ? 1 : -1;
        const newZoomLevel = Math.max(2, Math.min(game.zoomLevel + zoomFactor, 10));

        game.zoomLevel = newZoomLevel;
        const mouseXAfterZoom = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
        const mouseYAfterZoom = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;

        camera.cameraX += mouseXBeforeZoom - mouseXAfterZoom;
        camera.cameraY += mouseYBeforeZoom - mouseYAfterZoom;

        this.constrainCamera();
    } 
    // Pan the camera vertically in pan mode
    else if (game.editorMode === 'pan') {
        const scrollDirection = event.deltaY < 0 ? -1 : 1;
        camera.cameraY += scrollDirection * 10;
        this.constrainCamera();
    } 
    // Handle brush mode for changing brush size or zooming
    else if (game.editorMode === 'brush') {
        // Check if the Ctrl key is held down for zooming
        if (event.ctrlKey) {
            const scrollDirection = event.deltaY < 0 ? 1 : -1;
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
        } else {
            // Adjust the brush size with scrolling, allowing for a larger maximum brush size
            const scrollDirection = event.deltaY < 0 ? 5 : -5;  // Change the brush size faster by using a larger step value
            this.brushRadius = Math.max(16, Math.min(this.brushRadius + scrollDirection, 500));  // Set minimum to 16 and maximum to 500 pixels
        }
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

    // Update the walkable grid after removing objects
    collision.createWalkableGrid();

    console.log("Selected objects deleted.");

    // Switch back to select or lasso mode based on the previous mode
    if (this.previousMode === 'lasso') {
        this.changeMode('lasso');
    } else {
        this.changeMode('select'); // Default to select mode if not in lasso
    }
},

handleToggleSelection: function (mousePosition) {
    const clickedObject = game.roomData.items.find(item => {
        const itemRect = {
            x: Math.min(...item.x) * 16,
            y: Math.min(...item.y) * 16,
            width: (Math.max(...item.x) - Math.min(...item.x) + 1) * 16,
            height: (Math.max(...item.y) - Math.min(...item.y) + 1) * 16
        };

        return (
            mousePosition.x >= itemRect.x &&
            mousePosition.x <= itemRect.x + itemRect.width &&
            mousePosition.y >= itemRect.y &&
            mousePosition.y <= itemRect.y + itemRect.height
        );
    });

    if (clickedObject) {
        const objectIndex = this.selectedObjects.indexOf(clickedObject);

        if (objectIndex === -1) {
            // If the object is not already selected, add it to the selection
            this.selectedObjects.push(clickedObject);
            console.log("Object selected:", clickedObject);
        } else {
            // If the object is already selected, remove it from the selection
            this.selectedObjects.splice(objectIndex, 1);
            console.log("Object deselected:", clickedObject);
        }

        // Return true to indicate that an object was clicked
        return true;
    }

    // Return false to indicate that no object was clicked
    return false;
},

renderSelectedTiles: function() {
    if (this.selectedObjects.length > 0) {
        this.selectedObjects.forEach(selectedObject => {
            // Check if x and y arrays exist and have data
            if (selectedObject.x && selectedObject.y && selectedObject.x.length > 0 && selectedObject.y.length > 0) {
                // Get the minimum and maximum values from the x and y arrays
                const minX = Math.min(...selectedObject.x) * 16;  // Minimum X in pixels
                const minY = Math.min(...selectedObject.y) * 16;  // Minimum Y in pixels
                const maxX = Math.max(...selectedObject.x) * 16;  // Maximum X in pixels
                const maxY = Math.max(...selectedObject.y) * 16;  // Maximum Y in pixels

                // Calculate width and height in pixels
                const width = (maxX - minX) + 16;  // Width in pixels
                const height = (maxY - minY) + 16;  // Height in pixels

                // Apply a subtle shadow to the selected object for depth
                game.ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
                game.ctx.shadowBlur = 6;
                game.ctx.shadowOffsetX = 4;
                game.ctx.shadowOffsetY = 4;

                // Thin dotted border effect for selected object
                game.ctx.strokeStyle = 'rgba(255, 255, 255, 0.9)';  // White dotted border
                game.ctx.lineWidth = 1;
                game.ctx.setLineDash([4, 2]);  // Dotted line pattern: 4px dash, 2px gap
                game.ctx.strokeRect(minX, minY, width, height);

                // Reset shadow and line dash after rendering the object
                game.ctx.setLineDash([]);
                game.ctx.shadowBlur = 0;
                game.ctx.shadowOffsetX = 0;
                game.ctx.shadowOffsetY = 0;
            }
        });
    }
},

renderBrush: function () {
    if (this.isBrushModeActive) {
        const centerX = this.mouseX;
        const centerY = this.mouseY;

        game.ctx.fillStyle = 'rgba(0, 0, 255, 0.5)';  // Set brush color and transparency
        game.ctx.beginPath();
        game.ctx.arc(centerX, centerY, this.brushRadius / 2, 0, Math.PI * 2);  // Draw a circle with the brushRadius
        game.ctx.fill();
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

        // Semi-transparent fill
        game.ctx.fillStyle = 'rgba(0, 128, 255, 0.2)';
        game.ctx.fillRect(rect.x, rect.y, rect.width, rect.height);

        // Pulsating border effect
        const pulseSpeed = (performance.now() / 500) % 1;  // Control the pulse speed
        const pulseOpacity = 0.6 + 0.4 * Math.sin(pulseSpeed * Math.PI * 2);  // Create a pulsing opacity effect

        game.ctx.strokeStyle = `rgba(0, 128, 255, ${pulseOpacity})`;
        game.ctx.lineWidth = 2;
        game.ctx.setLineDash([6, 3]); // Dash pattern: 6px line, 3px gap
        game.ctx.shadowColor = `rgba(0, 0, 0, ${pulseOpacity})`;  // Adjust shadow opacity based on pulse
        game.ctx.shadowBlur = 8;
        game.ctx.strokeRect(rect.x, rect.y, rect.width, rect.height);

        // Reset shadow and line dash for other elements
        game.ctx.setLineDash([]);
        game.ctx.shadowBlur = 0;
    }
},

renderLasso: function () {
    if (this.lassoPath.length > 1) {
        const pulseSpeed = (performance.now() / 500) % 1;  // Control the pulse speed
        const pulseOpacity = 0.6 + 0.4 * Math.sin(pulseSpeed * Math.PI * 2);  // Create a pulsing opacity effect

        game.ctx.strokeStyle = `rgba(0, 255, 0, ${pulseOpacity})`;
        game.ctx.lineWidth = 2;
        game.ctx.setLineDash([6, 3]);  // Dash pattern: 6px line, 3px gap
        game.ctx.shadowColor = `rgba(0, 255, 0, ${pulseOpacity})`;  // Adjust shadow opacity based on pulse
        game.ctx.shadowBlur = 8;

        game.ctx.beginPath();
        game.ctx.moveTo(this.lassoPath[0].x, this.lassoPath[0].y);

        for (let i = 1; i < this.lassoPath.length; i++) {
            game.ctx.lineTo(this.lassoPath[i].x, this.lassoPath[i].y);
        }

        game.ctx.closePath();  // Optionally close the lasso path
        game.ctx.stroke();

        // Reset shadow and line dash for other elements
        game.ctx.setLineDash([]);
        game.ctx.shadowBlur = 0;
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

handleKeyDown: function (event) {
        const key = event.key;

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
        // Ctrl + X to cut
        else if (event.ctrlKey && key === 'x') {
            this.cutSelectedObjects();
        }
        // Ctrl + Z to undo
        else if (event.ctrlKey && !event.shiftKey && key === 'z') {
            this.undo();
        }
        // Ctrl + Shift + Z to redo
        else if (event.ctrlKey && event.shiftKey && key === 'Z') {
            this.redo();
        }
        else if (key === 'Delete' || key === 'Backspace') {
            this.deleteSelectedObjects();
        }
        else if (key === 'Shift' && game.editorMode === 'move') {
            if (this.previousMode === 'lasso') {
                this.changeMode('lasso');
            } else if (this.previousMode === 'select') {
                this.changeMode('select');
            }
        } else if (key === 'Shift' && game.editorMode === 'lasso') {
            // Stay in lasso mode
        } else {
            const modeIndex = parseInt(key, 10) - 1;
            if (modeIndex >= 0 && modeIndex < this.modes.length) {
                const selectedMode = this.modes[modeIndex];
                this.changeMode(selectedMode);
            }
        }
    },

handleKeyUp: function (event) {
    const key = event.key;

    // Shift key up - return to 'move' mode only if the previous mode was 'move'
    if (key === 'Shift' && (game.editorMode === 'select' || game.editorMode === 'lasso')) {
        this.changeMode('move');  // Return to move mode when Shift is released
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
            // Deep clone the copied objects and adjust their position
            const pastedObjects = this.clipboard.map(obj => {
                const newObj = JSON.parse(JSON.stringify(obj));
                // Shift pasted objects slightly to differentiate them from the originals
                newObj.x = newObj.x.map(coord => coord + 1);  // Shift by 1 tile
                newObj.y = newObj.y.map(coord => coord + 1);  // Shift by 1 tile
                return newObj;
            });

            // Add the pasted objects to the room data
            game.roomData.items.push(...pastedObjects);
            console.log("Objects pasted:", pastedObjects);

            // Update walkable grid and re-render the scene
            collision.createWalkableGrid();
            game.render();
        }
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
        game.render();
    } catch (error) {
        console.error("Error restoring room data:", error);
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

startSlidingCamera: function (mouseX, mouseY, boundaryXStart, boundaryXEnd, boundaryYStart, boundaryYEnd, boundarySpeed) {
    // Clear any existing sliding movement
    this.stopSlidingCamera();

    const slideInterval = 16; // Interval in ms (about 60fps)
    this.cameraSlidingInterval = setInterval(() => {
        let cameraMoved = false;
        let deltaX = 0;
        let deltaY = 0;

        // Check if the mouse is near the horizontal boundary
        if (mouseX < boundaryXStart) {
            deltaX = -boundarySpeed;  // Move camera left
            camera.cameraX += deltaX;
            cameraMoved = true;
        } else if (mouseX > boundaryXEnd) {
            deltaX = boundarySpeed;  // Move camera right
            camera.cameraX += deltaX;
            cameraMoved = true;
        }

        // Check if the mouse is near the vertical boundary
        if (mouseY < boundaryYStart) {
            deltaY = -boundarySpeed;  // Move camera up
            camera.cameraY += deltaY;
            cameraMoved = true;
        } else if (mouseY > boundaryYEnd) {
            deltaY = boundarySpeed;  // Move camera down
            camera.cameraY += deltaY;
            cameraMoved = true;
        }

        // Move selected objects along with the camera
        if (cameraMoved && this.selectedObjects.length > 0) {
            this.selectedObjects.forEach(obj => {
                obj.x = obj.x.map(coord => coord + deltaX / 16);  // Update X position
                obj.y = obj.y.map(coord => coord + deltaY / 16);  // Update Y position
            });

            this.lastMouseX = this.mouseX;
            this.lastMouseY = this.mouseY;

            this.constrainCamera();  // Ensure the camera stays within bounds
            game.render();  // Re-render the scene to reflect changes
        }

    }, slideInterval);  // Move camera every interval while the mouse is near the boundary
},

stopSlidingCamera: function () {
    if (this.cameraSlidingInterval) {
        clearInterval(this.cameraSlidingInterval);
        this.cameraSlidingInterval = null;
    }
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
        },
        error: function (data) {
            console.error('Error saving room data:', data);
            // Optionally handle error UI or notifications here
        }
    });
}

};

// Start the map editor window
edit_mode_window.start();


  </script>

</div>
<?php
}
?>
