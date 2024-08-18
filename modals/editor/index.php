<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='edit_mode_window' class='window window_bg position-fixed top-2 right-2 rounded-sm' style='width: 57px;background: #3a445b;'>

<!-- Handle that spans the whole left side -->
<div data-part='handle' class='window_title rounded-none w-full mb-1' style='height: 15px; background-image: radial-gradient(#e5e5e58a 1px, transparent 0) !important; border-radius: 0;'>
</div>

<!-- Rest of the content -->
<div class='relative flex-grow'>
  <div class='container text-light window_body px-1 py-1'>
    <button type="button" id="select_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_select"></div>
    </button>

    <button type="button" id="drop_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_pencil"></div>
    </button>

    <button type="button" id="brush_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #276b4f618b49; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_brush"></div>
    </button>

    <button type="button" id="move_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_scissors"></div>
    </button>

    <button type="button" id="pickup_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_magnify"></div>
    </button>

    <button type="button" id="navigate_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_delete"></div>
    </button>

    <button type="button" id="undo_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_undo"></div>
    </button>

    <button type="button" id="redo_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_redo"></div>
    </button>
    <button type="button" id="save_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_save"></div>
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
    tileSize: 16,
    tilesPerRow: 150,
    fixedHeight: 32,
    selectedItem: null,
    activeItemGroup: null,
    offsetX: null,
    offsetY: null,
    clickHandler: null,
    mouseMoveHandler: null,
    mouseUpHandler: null,
    isPlacingItem: false,
    brushRadius: 10,
    maxBrushRadius: 100,
    mouseX: 0, // Property to store current mouse X position
    mouseY: 0, // Property to store current mouse Y position
    modeButtons: {},

    start: function () {
        this.modeButtons = {
            brush: document.getElementById('brush_button'),
            select: document.getElementById('select_button'),
            move: document.getElementById('move_button'),
            pickup: document.getElementById('pickup_button'),
            drop: document.getElementById('drop_button'),
            navigate: document.getElementById('navigate_button')
        };

        game.isEditorActive = true;
        this.changeMode('select');

        Object.keys(this.modeButtons).forEach(mode => {
            var handler = () => this.changeMode(mode);
            this.modeButtons[mode].addEventListener('click', handler.bind(this));
        });

        game.isEditorActive = true;
        game.displaySprite = false;
        this.init(); // Initialize editor functions
    },

    init: function () {
        this.setupClickToActivate();
        this.setupGamepadEventListeners();
        this.setupBrushMode();
        this.changeMode('select');
    },

    setupClickToActivate: function () {
        this.clickHandler = this.handleClick.bind(this);
        this.mouseMoveHandler = this.handleMouseMove.bind(this);
        this.mouseUpHandler = this.handleMouseUp.bind(this);

        document.querySelectorAll('.inventory-item').forEach(item => {
            item.addEventListener('click', this.clickHandler);
        });

        document.addEventListener('mousemove', this.mouseMoveHandler);
        document.addEventListener('mouseup', this.mouseUpHandler);
        document.addEventListener('contextmenu', function (event) {
            event.preventDefault();
        });

        game.isEditorActive = false; // Set the flag to indicate the editor is active
        console.log('Editor setupClickToActivate triggered');
    },

    setupBrushMode: function () {
        window.addEventListener('wheel', this.handleMouseWheel.bind(this));
    },

    teardownClickToActivate: function () {
        document.querySelectorAll('.inventory-item').forEach(item => {
            item.removeEventListener('click', this.clickHandler);
        });

        document.removeEventListener('mousemove', this.mouseMoveHandler);
        document.removeEventListener('mouseup', this.mouseUpHandler);
        game.pathfinding = true;
        modal.showAll();
        game.isEditorActive = false; // Set the flag to indicate the editor is no longer active
        console.log('Editor teardownClickToActivate triggered');
    },

    setupGamepadEventListeners: function () {
        window.addEventListener('gamepadxPressed', gamepad.throttle((event) => {
            if (game.mainSprite.isCarrying) {
                this.dropItem(game.mainSprite);
                console.log("Dropped item");
            } else {
                const gridX = Math.floor(game.mainSprite.x / 16);
                const gridY = Math.floor(game.mainSprite.y / 16);

                const item = game.findObjectAt(gridX * 16, gridY * 16);

                if (item) {
                    this.pickUpItem(game.mainSprite, item.id); // Pass the item ID
                    console.log("Picked up", item.id);
                } else {
                    console.log("No item found at", gridX, gridY);
                }
            }
        }, 200));
    },

    pickUpItem: function (sprite) {
        const headroom = 16; // Extra margin around the item's boundary

        // Round sprite position to nearest integer
        const spriteX = Math.round(sprite.x);
        const spriteY = Math.round(sprite.y);

        console.log(`Sprite Position: (x: ${spriteX}, y: ${spriteY})`);

        // Function to check if the sprite is within the item's boundary with headroom
        function isSpriteInItemBoundary(spriteX, spriteY, item) {
            const itemData = game.objectData[item.id];
            if (!itemData || itemData.length === 0) return false;

            const xCoordinates = item.x.map(x => x * 16); // Convert to pixel coordinates
            const yCoordinates = item.y.map(y => y * 16); // Convert to pixel coordinates

            // Calculate the bounding box of the item with headroom
            const minX = Math.min(...xCoordinates) - headroom;
            const maxX = Math.max(...xCoordinates) + headroom;
            const minY = Math.min(...yCoordinates) - headroom;
            const maxY = Math.max(...yCoordinates) + headroom;

            // Special case for single-tile items
            if (xCoordinates.length === 1 && yCoordinates.length === 1) {
                const singleTileX = xCoordinates[0];
                const singleTileY = yCoordinates[0];
                const singleTileMinX = singleTileX - headroom;
                const singleTileMaxX = singleTileX + 16 + headroom;
                const singleTileMinY = singleTileY - headroom;
                const singleTileMaxY = singleTileY + 16 + headroom;
                return (spriteX >= singleTileMinX && spriteX <= singleTileMaxX && spriteY >= singleTileMinY && spriteY <= singleTileMaxY);
            }

            return (spriteX >= minX && spriteX <= maxX && spriteY >= minY && spriteY <= maxY);
        }

        let closestItem = null;

        // Find the nearest item within the boundary with headroom
        game.roomData.items.forEach(item => {
            if (isSpriteInItemBoundary(spriteX, spriteY, item)) {
                closestItem = item;
            }
        });

        // If a closest item is found, pick it up
        if (closestItem) {
            sprite.isCarrying = true;
            sprite.carriedItem = closestItem.id; // Store only the item ID

            // Store the exact position of the item
            sprite.carriedItemExactX = closestItem.x[0] * 16; // Assuming single tile item, adjust if needed
            sprite.carriedItemExactY = closestItem.y[0] * 16; // Assuming single tile item, adjust if needed

            // Remove the item from the room data by matching both ID and position
            game.roomData.items = game.roomData.items.filter(roomItem => !(roomItem.id === closestItem.id && roomItem.x[0] === closestItem.x[0] && roomItem.y[0] === closestItem.y[0]));

            // Update the game state and re-render
            game.render();
        } else {
            console.error("No item found within the pickup radius.");
        }
    },

    dropItem: function (sprite) {
        sprite.isCarrying = false;

        const itemId = sprite.carriedItem;
        const exactX = sprite.x;
        const exactY = sprite.y - 16;

        const item = { id: itemId, x: [], y: [] };

        const itemData = game.objectData[item.id];
        if (!itemData || itemData.length === 0) {
            console.error('Invalid item data for item ID:', item.id);
            return;
        }

        const tileData = itemData[0];
        const xCoordinates = tileData.a || [];
        const yCoordinates = tileData.b || [];

        // Calculate the base grid coordinates one tile away in the direction the sprite is facing
        let baseGridX = Math.floor(exactX / 16);
        let baseGridY = Math.floor(exactY / 16);

        switch (sprite.direction) {
            case 'N':
                baseGridY -= 2;
                break;
            case 'S':
                baseGridY -= 2; // Drop to the top if facing South
                break;
            case 'E':
                baseGridX += 2;
                break;
            case 'W':
                baseGridX -= 2;
                break;
            default:
                break;
        }

        const calculatedXSet = new Set();
        const calculatedYSet = new Set();

        for (let i = 0; i < xCoordinates.length; i++) {
            const calculatedX = baseGridX + xCoordinates[i];
            calculatedXSet.add(calculatedX);
        }

        for (let j = 0; j < yCoordinates.length; j++) {
            const calculatedY = baseGridY + yCoordinates[j];
            calculatedYSet.add(calculatedY);
        }

        item.x = Array.from(calculatedXSet);
        item.y = Array.from(calculatedYSet);

        game.roomData.items.push(item);
        sprite.carriedItem = null;

        // Update the game state and re-render
        game.render();
    },

    handleClick: function (event) { 
        console.log('Editor handleClick triggered');
        if (event.target.closest('.tabs')) {
            return;
        }

        const originalItem = event.currentTarget;
        const originalCanvas = originalItem.querySelector('canvas');

        if (this.activeItemGroup) {
            this.activeItemGroup.classList.remove('active');
        }

        if (this.selectedItem) {
            this.selectedItem.remove();
        }

        this.selectedItem = document.createElement('div');
        this.selectedItem.classList.add('inventory-item-clone');
        this.selectedItem.style.position = 'absolute';
        this.selectedItem.style.pointerEvents = 'none';
        this.selectedItem.style.zIndex = 1000;
        this.selectedItem.dataset.category = originalItem.dataset.category;

        const clonedCanvas = document.createElement('canvas');
        clonedCanvas.width = originalCanvas.width;
        clonedCanvas.height = originalCanvas.height;
        clonedCanvas.getContext('2d').drawImage(originalCanvas, 0, 0);

        this.selectedItem.appendChild(clonedCanvas);
        document.body.appendChild(this.selectedItem);

        this.offsetX = originalCanvas.width / 2;
        this.offsetY = originalCanvas.height / 2;

        this.activeItemGroup = originalItem.closest('.inventory-item-group');
        this.activeItemGroup.classList.add('active');

        this.moveSelectedItem(event);
        game.pathfinding = false;
        this.isPlacingItem = true;
        modal.hideAll(); // Hide all modals when an item is selected
    },

    handleMouseWheel: function (event) {
        if (game.editorMode === 'brush') {
            const delta = Math.sign(event.deltaY);
            // Increase the radius by 5 units instead of 2, with a max limit
            this.brushRadius = Math.max(5, Math.min(this.maxBrushRadius, this.brushRadius - delta * 5));
            console.log(`Brush radius changed to: ${this.brushRadius}`);
        }
    },

    handleMouseMove: function (event) {
        if (game.editorMode === 'brush') {
            // Store the current mouse position
            this.mouseX = event.clientX;
            this.mouseY = event.clientY;
            game.render(); // Re-render to update brush position
        } else if (this.selectedItem) {
            const uiMenu = document.querySelector('[data-window="ui_window"]');
            if (uiMenu && uiMenu.contains(event.target)) {
                this.selectedItem.style.display = 'none';
            } else {
                this.selectedItem.style.display = 'block';
                this.moveSelectedItem(event);
                const zoomLevel = game.zoomLevel;
                const cameraX = camera.cameraX;
                const cameraY = camera.cameraY;
                const dropX = (event.clientX - this.offsetX + window.scrollX) / zoomLevel + cameraX;
                const dropY = (event.clientY - this.offsetY + window.scrollY) / zoomLevel + cameraY;
                const snappedX = Math.round(dropX / this.tileSize);
                const snappedY = Math.round(dropY / this.tileSize);
                const newItem = { id: this.selectedItem.dataset.category, x: [], y: [] };
                this.calculateTilePositions(this.selectedItem, snappedX, snappedY, this.tileSize, newItem.x, newItem.y);
                this.highlightOverlappingTiles(newItem);
            }
        }
    },

    changeMode: function (newMode) {
        game.editorMode = newMode;
        console.log(`Editor mode changed to: ${newMode}`);
        // Update UI or perform other actions based on the mode change

        // Reset styles for all buttons
        Object.values(this.modeButtons).forEach(button => {
            button.style.background = '#4f618b';
            button.style.color = 'white';
        });

        // Highlight the active mode button
        if (this.modeButtons[newMode]) {
            this.modeButtons[newMode].style.background = 'white';
            this.modeButtons[newMode].style.color = '#276b49';
        }

        // Set the editor's current mode
        console.log(`Current mode: ${newMode}`);
    },

    handleMouseUp: function (event) {
        const isInWindow = event.target.closest('.window') !== null;
        console.log('Mouse up event:', event);
        console.log('Is in window:', isInWindow);

        if (this.selectedItem && !isInWindow && event.button === 0) {
            console.log('Placing item:', this.selectedItem);

            const zoomLevel = game.zoomLevel;
            const cameraX = camera.cameraX;
            const cameraY = camera.cameraY;

            const dropX = (event.clientX - this.offsetX + window.scrollX) / zoomLevel + cameraX;
            const dropY = (event.clientY - this.offsetY + window.scrollY) / zoomLevel + cameraY;

            const snappedX = Math.round(dropX / this.tileSize);
            const snappedY = Math.round(dropY / this.tileSize);

            const newItem = {
                id: this.selectedItem.dataset.category,
                x: [],
                y: []
            };

            if (event.shiftKey) {
                console.log('Adding items in line');
                this.addItemsInLine(newItem, snappedX, snappedY, this.tileSize);
            } else {
                console.log('Calculating tile positions');
                this.calculateTilePositions(this.selectedItem, snappedX, snappedY, this.tileSize, newItem.x, newItem.y);
                this.addNewItemToRoomData(newItem);
            }
        } else if (event.button === 2) {
            console.log('Deselecting item');

            if (this.activeItemGroup) {
                console.log('Removing active class from item group');
                this.activeItemGroup.classList.remove('active');
            }
            if (this.selectedItem) {
                console.log('Removing selected item');
                this.selectedItem.remove();
                this.selectedItem = null;
                game.pathfinding = true;
                this.isPlacingItem = false;
                game.overlappingTiles = [];
            }
            console.log('Showing all modals');
            modal.showAll(); // Show all modals only when the item is deselected
        }
    },

    moveSelectedItem: function (event) {
        const zoomLevel = game.zoomLevel;
        const cameraX = camera.cameraX;
        const cameraY = camera.cameraY;

        const canvasX = (event.clientX - this.offsetX + window.scrollX) / zoomLevel + cameraX;
        const canvasY = (event.clientY - this.offsetY + window.scrollY) / zoomLevel + cameraY;

        const snappedX = Math.round(canvasX / this.tileSize) * this.tileSize;
        const snappedY = Math.round(canvasY / this.tileSize) * this.tileSize;

        this.selectedItem.style.left = `${(snappedX - cameraX) * zoomLevel + window.scrollX}px`;
        this.selectedItem.style.top = `${(snappedY - cameraY) * zoomLevel + window.scrollY}px`;

        this.selectedItem.style.transform = `scale(${zoomLevel})`;
        this.selectedItem.style.transformOrigin = 'top left';
    },

    calculateTilePositions: function (item, baseX, baseY, tileSize, xArray, yArray) {
        const canvas = item.querySelector('canvas');
        const width = canvas.width;
        const height = canvas.height;
        const cols = width / tileSize;
        const rows = height / tileSize;

        for (let col = 0; col < cols; col++) {
            xArray.push(baseX + col);
        }
        for (let row = 0; row < rows; row++) {
            yArray.push(baseY + row);
        }
    },

    addItemsInLine: function (item, endX, endY, tileSize) {
        if (!game.previousDrop) {
            game.previousDrop = { x: endX, y: endY };
            this.calculateTilePositions(this.selectedItem, endX, endY, tileSize, item.x, item.y);
            this.addNewItemToRoomData(item);
            return;
        }

        const startX = game.previousDrop.x;
        const startY = game.previousDrop.y;

        const stepX = endX > startX ? 1 : -1;
        const stepY = endY > startY ? 1 : -1;

        if (startX === endX) {
            for (let y = startY; y !== endY + stepY; y += stepY) {
                this.calculateTilePositions(this.selectedItem, endX, y, tileSize, item.x, item.y);
                this.addNewItemToRoomData({ ...item, y: [y], x: [endX] });
            }
        } else if (startY === endY) {
            for (let x = startX; x !== endX + stepX; x += stepX) {
                this.calculateTilePositions(this.selectedItem, x, endY, tileSize, item.x, item.y);
                this.addNewItemToRoomData({ ...item, y: [endY], x: [x] });
            }
        } else {
            // Handle diagonal placement if needed
        }

        game.previousDrop = { x: endX, y: endY };
    },

    addNewItemToRoomData: function (newItem) {
    if (!game.roomData.items) {
        game.roomData.items = [];
    }

    function isOverlapping(existingItem, newItem) {
        const existingItemData = game.objectData[existingItem.id];
        const newItemData = game.objectData[newItem.id];

        if (!existingItemData || !newItemData) {
            return false;
        }

        const existingTiles = new Set();
        for (let i = 0; i < existingItem.x.length; i++) {
            for (let j = 0; j < existingItem.y.length; j++) {
                existingTiles.add(`${existingItem.x[i]}_${existingItem.y[j]}`);
            }
        }

        for (let i = 0; i < newItem.x.length; i++) {
            for (let j = 0; j < newItem.y.length; j++) {
                const tileKey = `${newItem.x[i]}_${newItem.y[j]}`;
                if (existingTiles.has(tileKey)) {
                    if (game.objectData[existingItem.id][0].z >= game.objectData[newItem.id][0].z) {
                        game.overlappingTiles.push({ x: newItem.x[i], y: newItem.y[j] });
                    }
                }
            }
        }

        return game.overlappingTiles.length > 0;
    }

    let overlapDetected = false;
    for (let i = 0; i < game.roomData.items.length; i++) {
        const existingItem = game.roomData.items[i];
        if (isOverlapping(existingItem, newItem)) {
            overlapDetected = true;
            break;
        }
    }

    if (overlapDetected && newItem.s !== 1) {
        console.log('Overlap detected, item not added.');
        return;
    }

    game.roomData.items.push(newItem);

    // Check if the new item has the createScene property set to true
    const itemData = game.objectData[newItem.id][0];
    if (itemData.createScene) {
        this.createSceneForItem(newItem, itemData);
    }

    effects.shakeMap(300, 2);

    console.log('New item added to roomData:', newItem);
    audio.playAudio("objectDrop", assets.load('objectDrop'), 'sfx');
    this.saveRoomData();
},

createSceneForItem: function (item, itemData) {
    const sceneName = `${itemData.n} ${itemData.sceneType === 'int' ? 'Interior' : 'Exterior'}`;

    if (!game.serverid) {
        console.error('Server ID is not defined');
        return;
    }

    const requestData = {
        server_id: game.serverid,
        name: sceneName,
        type: itemData.sceneType
    };

    ui.ajax({
        outputType: 'json',
        method: 'POST',
        url: '/modals/menus/console/tabs/servers/ajax/createScene.php',
        data: JSON.stringify(requestData),
        headers: {
            'Content-Type': 'application/json'
        },
        success: function (data) {
            if (!data.error) {
                console.log('Scene created successfully:', data.scene);
                // Optionally, store the scene ID in the item data or link it to the item
                // Example: itemData.sceneId = data.scene.id;
            } else {
                console.error('Error creating scene:', data.message);
            }
        },
        error: function (xhr, status, error) {
            console.error('Error creating scene:', error || xhr.statusText);
        }
    });
},

    pickUpSelectedItems: function () {
        if (game.selectedObjects.length === 0) return;

        // Remove selected items from roomData
        game.roomData.items = game.roomData.items.filter(roomItem => !game.selectedObjects.includes(roomItem));

        // Remove associated light sources and effects
        game.selectedObjects.forEach(item => {
            const xCoordinates = item.x.map(x => parseInt(x, 10) * 16);
            const yCoordinates = item.y.map(y => parseInt(y, 10) * 16);
            xCoordinates.forEach((x, index) => {
                const itemData = game.objectData[item.id];
                if (itemData && itemData.length > 0) {
                    const tileData = itemData[0];
                    if (tileData.l && tileData.l.length > 0) {
                        tileData.l.forEach(light => {
                            if (Array.isArray(light) && light.length === 2) {
                                const lightXIndex = light[0];
                                const lightYIndex = light[1];

                                if (lightXIndex >= 0 && lightXIndex < item.x.length &&
                                    lightYIndex >= 0 && lightYIndex < item.y.length) {

                                    const tileX = item.x[lightXIndex];
                                    const tileY = item.y[lightYIndex];

                                    const lightId = `${item.id}_${tileX}_${tileY}`;
                                    lighting.lights = lighting.lights.filter(light => light.id !== lightId);
                                }
                            }
                        });
                    }
                }
            });

            // Call removeItemEffects to remove associated effects
            particles.removeItemEffects(item);
        });

        // Highlight the removed item tiles
        game.overlappingTiles = [];
        game.selectedObjects.forEach(item => {
            const xCoordinates = item.x.map(x => parseInt(x, 10) * 16);
            const yCoordinates = item.y.map(y => parseInt(y, 10) * 16);
            xCoordinates.forEach((x, index) => {
                game.overlappingTiles.push({ x: x, y: yCoordinates[index] });
            });
        });

        // Clear selectedObjects
        game.selectedObjects = [];

        // Save the updated room data
        this.saveRoomData();

        // Re-render the game state to reflect the changes immediately
        game.render();

        console.log('Selected items picked up and associated light sources removed');
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
            url: 'modals/inventory/ajax/save_map.php',
            data: dataToSend,
            headers: {
                'Content-Type': 'application/json'
            },
            success: function (data) {
                console.log('Room data saved successfully:', data);
            },
            error: function (data) {
                console.error('Error saving room data:', data);
            }
        });
    },

    highlightOverlappingTiles: function (newItem) {
        if (!game.roomData.items) return;

        game.overlappingTiles = [];

        function isOverlapping(existingItem, newItem) {
            const existingItemData = game.objectData[existingItem.id];
            const newItemData = game.objectData[newItem.id];

            if (!existingItemData || !newItemData) {
                return false;
            }

            const existingTiles = new Set();
            for (let i = 0; i < existingItem.x.length; i++) {
                for (let j = 0; j < existingItem.y.length; j++) {
                    existingTiles.add(`${existingItem.x[i]}_${existingItem.y[j]}`);
                }
            }

            for (let i = 0; i < newItem.x.length; i++) {
                for (let j = 0; j < newItem.y.length; j++) {
                    const tileKey = `${newItem.x[i]}_${newItem.y[j]}`;
                    if (existingTiles.has(tileKey)) {
                        if (game.objectData[existingItem.id][0].z >= game.objectData[newItem.id][0].z) {
                            game.overlappingTiles.push({ x: newItem.x[i], y: newItem.y[j] });
                        }
                    }
                }
            }

            return game.overlappingTiles.length > 0;
        }

        for (let i = 0; i < game.roomData.items.length; i++) {
            const existingItem = game.roomData.items[i];
            if (isOverlapping(existingItem, newItem)) {
                break;
            }
        }
    },

    drawBrushCircle: function () {
        const rect = game.canvas.getBoundingClientRect();
        const mouseX = (this.mouseX - rect.left) / game.zoomLevel + camera.cameraX;
        const mouseY = (this.mouseY - rect.top) / game.zoomLevel + camera.cameraY;
        const brushRadius = this.brushRadius;

        game.ctx.save();
        game.ctx.globalAlpha = 0.5; // Set opacity
        game.ctx.beginPath();
        game.ctx.arc(mouseX, mouseY, brushRadius, 0, 2 * Math.PI);
        game.ctx.fillStyle = 'rgba(0, 255, 0, 0.5)'; // Fill color
        game.ctx.fill();
        game.ctx.restore();
    }
};
// Start the edit mode window when required
edit_mode_window.start();
  </script>

</div>
<?php
}
?>