<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
<div data-window='editor_inventory_window' class='window bg-gray-800 text-white shadow-2xl rounded-lg overflow-hidden' style='width: 350px; height: 540px;'>

<div data-part='handle' class='window_title' style='background-image: radial-gradient(#272031 1px, transparent 0) !important;'>
      <div class='float-right mt-1'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #1f2937; color: #ede8d6;'>Inventory</div>
    </div>

    <!-- Content Section -->
    <div class='p-4 relative'>

        <!-- Inventory Grid -->
        <div id="inventory_grid" class="grid grid-cols-4 gap-4 overflow-y-auto" style="max-height: 480px;">
            <!-- Inventory items will be dynamically inserted here -->
        </div>
    </div>

    <script>
    editor_inventory_window = {
        selectedInventoryItem: null, 
        selectedInventoryItemPos: { x: 0, y: 0 },
        isDragging: false, 

        start: function() {
            this.displayItems();
            this.addGlobalListeners();
        },

        displayItems: function() {
            var itemData = game.objectData;
            var gridContainer = document.querySelector('#inventory_grid');
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
                        const tilesetImage = assets.use(item.t);
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

                        itemCanvas.addEventListener('mousedown', editor_inventory_window.handleMouseDown);

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

            editor_inventory_window.stopDragging();

            editor_inventory_window.selectedInventoryItem = itemId;

            document.addEventListener('mousemove', editor_inventory_window.trackMouseForItem);
            editor_inventory_window.isDragging = true;
        },

        trackMouseForItem: function(event) {
            const rect = game.canvas.getBoundingClientRect();
            const mouseX = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
            const mouseY = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;

            editor_inventory_window.selectedInventoryItemPos.x = mouseX;
            editor_inventory_window.selectedInventoryItemPos.y = mouseY;

            document.addEventListener('mouseup', editor_inventory_window.handleMapClick);
        },

        handleMapClick: function(event) {
            if (event.button === 2) return;

            const clonedObjectX = editor_inventory_window.selectedInventoryItemPos.x;
            const clonedObjectY = editor_inventory_window.selectedInventoryItemPos.y;

            if (editor_inventory_window.selectedInventoryItem) {
                const itemData = game.objectData[editor_inventory_window.selectedInventoryItem][0];
                if (!itemData) return;

                const objectWidth = (itemData.a + 1) * 16;
                const objectHeight = (itemData.b + 1) * 16;

                const adjustedMouseX = clonedObjectX - objectWidth / 2;
                const adjustedMouseY = clonedObjectY - objectHeight / 2;

                editor_inventory_window.addItemToRoomData(editor_inventory_window.selectedInventoryItem, adjustedMouseX, adjustedMouseY);
            }
        },

        stopDragging: function() {
            if (editor_inventory_window.selectedInventoryItem) {
                editor_inventory_window.selectedInventoryItem = null;
                document.removeEventListener('mousemove', editor_inventory_window.trackMouseForItem);
                document.removeEventListener('mouseup', editor_inventory_window.handleMapClick);
                editor_inventory_window.isDragging = false;
            }
        },

        addItemToRoomData: function(itemId, mouseX, mouseY) {
            const baseX = Math.floor(mouseX / 16);
            const baseY = Math.floor(mouseY / 16);

            const itemData = game.objectData[itemId] ? game.objectData[itemId][0] : null;
            if (!itemData) {
                console.error("Item data not found for itemId:", itemId);
                return;
            }

            const maxColumns = itemData.a || 1;
            const maxRows = itemData.b || 1;

            const newX = [];
            const newY = [];

            for (let col = 0; col <= maxColumns; col++) {
                newX.push(baseX + col);
            }

            for (let row = 0; row <= maxRows; row++) {
                newY.push(baseY + row);
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
        },

        unmount: function() {
            editor_inventory_window.stopDragging();
        },

        addGlobalListeners: function() {
            document.addEventListener('contextmenu', function(event) {
                event.preventDefault();
                editor_inventory_window.stopDragging();
            });

            document.addEventListener('keydown', function(event) {
                if (event.key === 'Escape') {
                    editor_inventory_window.stopDragging();
                }
            });
        }
    };

    editor_inventory_window.start();
    </script>

</div>

<?php
}
?>
