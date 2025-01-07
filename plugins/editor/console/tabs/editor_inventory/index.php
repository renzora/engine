<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config/db.php';
if ($auth) {
?>

<!-- Inventory Search and Grid -->
<div class="tabs mb-4">
  <input id="inventory-search" type="text" class="p-2 w-full rounded light_input text-base text-black" placeholder="Search..."/>
</div>

<!-- Inventory Grid -->
<div id="inventory-grid" class="inventory-grid grid grid-cols-4 gap-1 overflow-x-hidden">
  <!-- Items will be appended here dynamically -->
</div>

<script>
var ui_console_editor_inventory = {
    selectedInventoryItem: null,
    selectedInventoryItemPos: { x: 0, y: 0 },
    isDragging: false,
    itemData: null, // Store item data for filtering

    start: function() {
        this.itemData = assets.use('objectData');
        this.displayItems();
        this.addGlobalListeners();
        this.addSearchListener();
    },

    displayItems: function(filteredData = null) {
        const itemData = filteredData || this.itemData;
        const gridContainer = document.querySelector('#inventory-grid');
        const tileSize = 16;
        const tilesPerRow = 150;

        gridContainer.innerHTML = ''; // Clear existing items

        for (let category in itemData) {
            if (itemData.hasOwnProperty(category)) {
                const items = itemData[category];
                if (items.length === 0) continue;

                const itemGroupElement = document.createElement('div');
                itemGroupElement.classList.add('inventory-item-group', 'bg-gray-700', 'py-2', 'rounded', 'mb-4', 'shadow-lg', 'hover:bg-gray-600', 'transition', 'duration-300');

                items.forEach(item => {
                    const tilesetImage = assets.use(item.t); // Load tileset image
                    const itemCanvas = document.createElement('canvas');
                    const ctx = itemCanvas.getContext('2d');

                    const maxCol = item.a;
                    const maxRow = item.b;
                    itemCanvas.width = (maxCol + 1) * tileSize;
                    itemCanvas.height = (maxRow + 1) * tileSize;

                    let framesToRender = [];

                    if (item.d && Array.isArray(item.i[0])) {
                        framesToRender = item.i[0];
                    } else if (Array.isArray(item.i[0])) {
                        framesToRender = item.i.flat();
                    } else {
                        framesToRender = item.i.map(frame => {
                            if (typeof frame === 'string' && frame.includes('-')) {
                                return render.parseRange(frame);
                            }
                            return [frame];
                        }).flat();
                    }

                    framesToRender.forEach((frame, index) => {
                        const srcX = (frame % tilesPerRow) * tileSize;
                        const srcY = Math.floor(frame / tilesPerRow) * tileSize;

                        const destX = (index % (maxCol + 1)) * tileSize;
                        const destY = Math.floor(index / (maxCol + 1)) * tileSize;

                        ctx.drawImage(
                            tilesetImage,
                            srcX, srcY, tileSize, tileSize,
                            destX, destY, tileSize, tileSize
                        );
                    });

                    itemCanvas.setAttribute('data-item-id', category); // Using category

                    const canvasContainer = document.createElement('div');
                    canvasContainer.className = 'flex justify-center items-center w-full h-full max-w-[150px] max-h-[150px] aspect-w-1 aspect-h-1 overflow-hidden rounded-lg shadow-md transition duration-300 transform hover:scale-105';
                    itemCanvas.className += ' w-full h-full object-contain';

                    itemCanvas.addEventListener('mousedown', ui_console_editor_inventory.handleMouseDown);

                    canvasContainer.appendChild(itemCanvas);
                    itemGroupElement.appendChild(canvasContainer);
                });

                gridContainer.appendChild(itemGroupElement);
            }
        }
    },

    addSearchListener: function() {
        const searchInput = document.querySelector('#inventory-search');
        searchInput.addEventListener('input', (event) => {
            const searchTerm = event.target.value.toLowerCase();
            this.filterItems(searchTerm);
        });
    },

    filterItems: function(searchTerm) {
    if (!this.itemData) return;

    const filteredData = {};
    for (let category in this.itemData) {
        if (this.itemData.hasOwnProperty(category)) {
            const items = this.itemData[category];
            const matchingItems = items.filter(item =>
                item.n && item.n.toLowerCase().includes(searchTerm) // Check the `n` property
            );
            if (matchingItems.length > 0) {
                filteredData[category] = matchingItems;
            }
        }
    }

    this.displayItems(filteredData);
},

    render: function() {
        const itemData = game.objectData[this.selectedInventoryItem][0];
        if (!itemData) return;

        const objectWidth = (itemData.a + 1) * 16;
        const objectHeight = (itemData.b + 1) * 16;

        let posX = this.selectedInventoryItemPos.x - objectWidth / 2;
        let posY = this.selectedInventoryItemPos.y - objectHeight / 2;

        posX = Math.round(posX);
        posY = Math.round(posY);

        const parseRange = (rangeString) => {
            const [start, end] = rangeString.split('-').map(Number);
            const rangeArray = [];
            for (let i = start; i <= end; i++) {
                rangeArray.push(i);
            }
            return rangeArray;
        };

        let frameIndices = [];
        if (typeof itemData.i[0] === 'string' && itemData.i[0].includes('-')) {
            frameIndices = parseRange(itemData.i[0]);
        } else {
            frameIndices = itemData.i;
        }

        const img = assets.use(itemData.t);
        if (!img) return;

        let frameIndex = 0;
        for (let row = 0; row < itemData.b + 1; row++) {
            for (let col = 0; col < itemData.a + 1; col++) {
                if (frameIndex >= frameIndices.length) break;

                const tileFrameIndex = frameIndices[frameIndex];
                const srcX = (tileFrameIndex % 150) * 16;
                const srcY = Math.floor(tileFrameIndex / 150) * 16;

                const tilePosX = Math.round(posX + col * 16);
                const tilePosY = Math.round(posY + row * 16);

                game.ctx.drawImage(img, srcX, srcY, 16, 16, tilePosX, tilePosY, 16, 16);
                frameIndex++;
            }
        }
    },

    handleMouseDown: function(event) {
    const clickedCanvas = event.currentTarget;
    const itemId = clickedCanvas.getAttribute('data-item-id');

    if (!itemId) {
        console.error("Item ID not found.");
        return;
    }

    ui_console_editor_inventory.stopDragging();

    // Set the selected inventory item and allow continuous placement
    edit_mode_window.isAddingNewObject = true;
    ui_console_editor_inventory.selectedInventoryItem = itemId;
    
    document.addEventListener('mousemove', ui_console_editor_inventory.trackMouseForItem);
    ui_console_editor_inventory.isDragging = true;
},



    trackMouseForItem: function(event) {
        const rect = game.canvas.getBoundingClientRect();
        const mouseX = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
        const mouseY = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;

        ui_console_editor_inventory.selectedInventoryItemPos.x = mouseX;
        ui_console_editor_inventory.selectedInventoryItemPos.y = mouseY;

        document.addEventListener('mouseup', ui_console_editor_inventory.handleMapClick);
    },

handleMapClick: function(event) {
    if (event.button === 2) return;  // Prevent right-click from triggering
    if (edit_mode_window.isAddingNewObject) {
        const clonedObjectX = ui_console_editor_inventory.selectedInventoryItemPos.x;
        const clonedObjectY = ui_console_editor_inventory.selectedInventoryItemPos.y;

        if (ui_console_editor_inventory.selectedInventoryItem) {
            const itemData = game.objectData[ui_console_editor_inventory.selectedInventoryItem][0];
            if (!itemData) return;

            const objectWidth = (itemData.a + 1) * 16;
            const objectHeight = (itemData.b + 1) * 16;

            const adjustedMouseX = clonedObjectX - objectWidth / 2;
            const adjustedMouseY = clonedObjectY - objectHeight / 2;

            // Add the item to the room
            ui_console_editor_inventory.addItemToRoomData(ui_console_editor_inventory.selectedInventoryItem, adjustedMouseX, adjustedMouseY);

            // Continue allowing placement of the same object without selecting existing ones
            return; // Stop further event propagation
        }
    }

    // Call the selection logic here if not adding a new object
    // Continue with other selection or interaction logic
    // Example: You can call a different function to handle normal object selection
    this.handleSelectionStart(event);
},


    stopDragging: function() {
        if (ui_console_editor_inventory.selectedInventoryItem) {
            ui_console_editor_inventory.deselectInventoryItem();
            document.removeEventListener('mousemove', ui_console_editor_inventory.trackMouseForItem);
            document.removeEventListener('mouseup', ui_console_editor_inventory.handleMapClick);
            ui_console_editor_inventory.isDragging = false;
        }
    },

    deselectInventoryItem: function() {
        ui_console_editor_inventory.selectedInventoryItem = null;
        // Re-enable normal selection behavior
        edit_mode_window.isAddingNewObject = false;
    },

    cancelAddingObject: function() {
    // Cancel continuous placement and reset the object
    ui_console_editor_inventory.stopDragging();
    ui_console_editor_inventory.deselectInventoryItem();

    console.log("object selection cancelled");
},

addItemToRoomData: function(itemId, mouseX, mouseY) {
    // Retrieve item data from the game object
    const itemData = game.objectData[itemId] ? game.objectData[itemId][0] : null;
    if (!itemData) {
        console.error("Item data not found for itemId:", itemId);
        return;
    }

    // Determine base position (snap to grid or pixel-perfect)
    let baseX = editor_context_menu_window.isSnapEnabled ? Math.floor(mouseX / 16) * 16 : Math.round(mouseX);
    let baseY = editor_context_menu_window.isSnapEnabled ? Math.floor(mouseY / 16) * 16 : Math.round(mouseY);

    const maxColumns = itemData.a || 0;  // Number of columns (width)
    const maxRows = itemData.b || 0;     // Number of rows (height)

    // Arrays to store the new X and Y positions for each tile of the object
    const newX = [];
    const newY = [];

    // Loop through columns to calculate the X positions
    for (let col = 0; col <= maxColumns; col++) {
        if (editor_context_menu_window.isSnapEnabled) {
            // Snap to grid
            newX.push(Math.floor(baseX / 16) + col);  
        } else {
            // Pixel-perfect position (without snapping)
            newX.push((baseX + col * 16) / 16);  
        }
    }

    // Loop through rows to calculate the Y positions
    for (let row = 0; row <= maxRows; row++) {
        if (editor_context_menu_window.isSnapEnabled) {
            // Snap to grid
            newY.push(Math.floor(baseY / 16) + row);  
        } else {
            // Pixel-perfect position (without snapping)
            newY.push((baseY + row * 16) / 16);  
        }
    }

    // Create a new object to add to the room
    const newItem = {
        id: itemId,
        x: newX,
        y: newY,
        animationState: [{ currentFrame: 0, elapsedTime: 0 }],
        w: itemData.w || []  // Object width data, if any
    };

    // Check if roomData exists and initialize if necessary
    if (!game.roomData.items) {
        game.roomData.items = [];
    }

    // Add the new item to the room data
    game.roomData.items.push(newItem);

    // Debugging: Log the new item added to ensure correct data is being sent
    console.log('New object added:', newItem, 'with', editor_utils_window.isSnapEnabled ? 'grid-snapping' : 'pixel-perfect', 'placement.');
},


    unmount: function() {
        ui_console_editor_inventory.stopDragging();
    },

    addGlobalListeners: function() {
    document.addEventListener('contextmenu', function(event) {
        event.preventDefault();  // Prevent the default right-click menu
        console.log("Right-click detected - calling cancelAddingObject()");
        ui_console_editor_inventory.cancelAddingObject();
    });

    document.addEventListener('keydown', function(event) {
        if (event.key === 'Escape') {
            console.log("Escape key pressed - calling cancelAddingObject()");
            ui_console_editor_inventory.cancelAddingObject();
        }
    });
}


};

ui_console_editor_inventory.start();
</script>

<?php
}
?>