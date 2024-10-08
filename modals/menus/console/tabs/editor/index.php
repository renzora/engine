<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>

<!-- Inventory Search and Grid -->
<div class="tabs mb-4">
  <input type="text" class="p-2 w-full rounded light_input text-base text-black" placeholder="search..."/>
</div>

<!-- Inventory Grid -->
<div class="inventory-grid grid grid-cols-4 gap-1 overflow-x-hidden">
  <!-- Items will be appended here dynamically -->
</div>

<script>
var ui_console_tab_window = {
    selectedInventoryItem: null,
    selectedInventoryItemPos: { x: 0, y: 0 },
    isDragging: false,

    start: function() {
        this.displayItems();
        this.addGlobalListeners();
    },

    displayItems: function() {
        var itemData = game.objectData;
        var gridContainer = document.querySelector('.inventory-grid');
        var tileSize = 16;
        var tilesPerRow = 150;

        gridContainer.innerHTML = '';

        for (var itemId in itemData) {
            if (itemData.hasOwnProperty(itemId)) {
                var items = itemData[itemId];
                if (items.length === 0) continue;

                var itemGroupElement = document.createElement('div');
                itemGroupElement.classList.add('inventory-item-group', 'bg-gray-700', 'py-2', 'rounded', 'mb-4', 'shadow-lg', 'hover:bg-gray-600', 'transition', 'duration-300');

                items.forEach(function(item) {
                    const tilesetImage = assets.load(item.t);
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

                    itemCanvas.setAttribute('data-item-id', itemId);

                    const canvasContainer = document.createElement('div');
                    canvasContainer.className = 'flex justify-center items-center w-full h-full max-w-[150px] max-h-[150px] aspect-w-1 aspect-h-1 overflow-hidden rounded-lg shadow-md transition duration-300 transform hover:scale-105';
                    itemCanvas.className += ' w-full h-full object-contain';

                    itemCanvas.addEventListener('mousedown', ui_console_tab_window.handleMouseDown);

                    canvasContainer.appendChild(itemCanvas);
                    itemGroupElement.appendChild(canvasContainer);
                });

                gridContainer.appendChild(itemGroupElement);
            }
        }
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

        const img = assets.load(itemData.t);
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

        ui_console_tab_window.stopDragging();

        // Set the selected inventory item and allow continuous placement
        edit_mode_window.isAddingNewObject = true;
        ui_console_tab_window.selectedInventoryItem = itemId;

        document.addEventListener('mousemove', ui_console_tab_window.trackMouseForItem);
        ui_console_tab_window.isDragging = true;
    },

    trackMouseForItem: function(event) {
        const rect = game.canvas.getBoundingClientRect();
        const mouseX = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
        const mouseY = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;

        ui_console_tab_window.selectedInventoryItemPos.x = mouseX;
        ui_console_tab_window.selectedInventoryItemPos.y = mouseY;

        document.addEventListener('mouseup', ui_console_tab_window.handleMapClick);
    },

    handleMapClick: function(event) {
    // Prevent selection when adding a new object
    if (edit_mode_window.isAddingNewObject) {
        const clonedObjectX = ui_console_tab_window.selectedInventoryItemPos.x;
        const clonedObjectY = ui_console_tab_window.selectedInventoryItemPos.y;

        if (ui_console_tab_window.selectedInventoryItem) {
            const itemData = game.objectData[ui_console_tab_window.selectedInventoryItem][0];
            if (!itemData) return;

            const objectWidth = (itemData.a + 1) * 16;
            const objectHeight = (itemData.b + 1) * 16;

            const adjustedMouseX = clonedObjectX - objectWidth / 2;
            const adjustedMouseY = clonedObjectY - objectHeight / 2;

            // Add the item to the room
            ui_console_tab_window.addItemToRoomData(ui_console_tab_window.selectedInventoryItem, adjustedMouseX, adjustedMouseY);

            // Continue allowing placement of the same object without selecting existing ones
            return; // Stop further event propagation
        }
    }
},


    stopDragging: function() {
        if (ui_console_tab_window.selectedInventoryItem) {
            ui_console_tab_window.deselectInventoryItem();
            document.removeEventListener('mousemove', ui_console_tab_window.trackMouseForItem);
            document.removeEventListener('mouseup', ui_console_tab_window.handleMapClick);
            ui_console_tab_window.isDragging = false;
        }
    },

    deselectInventoryItem: function() {
        ui_console_tab_window.selectedInventoryItem = null;
        // Re-enable normal selection behavior
        edit_mode_window.isAddingNewObject = false;
    },

    cancelAddingObject: function() {
        // Cancel continuous placement and reset the object
        ui_console_tab_window.stopDragging();
        ui_console_tab_window.deselectInventoryItem();
    },

    addItemToRoomData: function(itemId, mouseX, mouseY) {
    const itemData = game.objectData[itemId] ? game.objectData[itemId][0] : null;
    if (!itemData) {
        console.error("Item data not found for itemId:", itemId);
        return;
    }

    // Determine whether to snap or use pixel-perfect placement
    let baseX = editor_utils_window.isSnapEnabled ? Math.floor(mouseX / 16) * 16 : Math.round(mouseX);
    let baseY = editor_utils_window.isSnapEnabled ? Math.floor(mouseY / 16) * 16 : Math.round(mouseY);

    const maxColumns = itemData.a || 1;
    const maxRows = itemData.b || 1;

    const newX = [];
    const newY = [];

    for (let col = 0; col <= maxColumns; col++) {
        if (editor_utils_window.isSnapEnabled) {
            newX.push(Math.floor(baseX / 16) + col);  // Snap to grid when enabled
        } else {
            newX.push((baseX + col * 16) / 16);  // Pixel-perfect position (exact without grid snapping)
        }
    }

    for (let row = 0; row <= maxRows; row++) {
        if (editor_utils_window.isSnapEnabled) {
            newY.push(Math.floor(baseY / 16) + row);  // Snap to grid when enabled
        } else {
            newY.push((baseY + row * 16) / 16);  // Pixel-perfect position (exact without grid snapping)
        }
    }

    const newItem = {
        id: itemId,
        x: newX,
        y: newY,
        animationState: [{ currentFrame: 0, elapsedTime: 0 }],
        w: itemData.w || []
    };

    if (!game.roomData.items) {
        game.roomData.items = [];
    }
    game.roomData.items.push(newItem);

    console.log('New object added with', editor_utils_window.isSnapEnabled ? 'grid-snapping' : 'pixel-perfect', 'placement.');
},



    unmount: function() {
        ui_console_tab_window.stopDragging();
    },

    addGlobalListeners: function() {
        document.addEventListener('contextmenu', function(event) {
            event.preventDefault();
            ui_console_tab_window.stopDragging();
        });

        document.addEventListener('keydown', function(event) {
            if (event.key === 'Escape') {
                ui_console_tab_window.stopDragging();
            }
        });
    }
};

ui_console_tab_window.start();
</script>

<?php
}
?>