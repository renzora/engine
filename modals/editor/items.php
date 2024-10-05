<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='inventory_template_window' class='window bg-gray-700' style='width: 330px; height: 500px;'>

    <div data-part='handle' class='window_title bg-gray-600 text-gray-100 p-2 rounded-t'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 text-white" aria-label="Close (ESC)" data-close>&times;</button>
      </div>
      <div data-part='title' class='title_bg window_border text-gray-100'>Inventory Template</div>
    </div>
    
    <div class='clearfix'></div>
    
    <div class='relative'>
      <div class='container text-white p-2'>

        <!-- Inventory Grid -->
        <div id="inventory_grid" class="inventory-grid bg-gray-800 p-2 rounded-md grid grid-cols-3 gap-2 overflow-y-auto" style="max-height: 400px;">
          <!-- Dynamically loaded inventory items will appear here -->
        </div>

      </div>
    </div>

    <script>
var inventory_template_window = {
    start: function() {
        this.displayItems();
        this.handleObjectClick();  // Set up click event listeners for items
    },

    displayItems: function() {
        var itemData = assets.load('objectData'); // Load object data
        console.log('Loaded itemData:', itemData);

        var gridContainer = document.querySelector('#inventory_grid');
        var tileSize = 16;
        var tilesPerRow = 150;

        for (var category in itemData) {
            if (itemData.hasOwnProperty(category)) {
                var items = itemData[category];
                if (items.length === 0) continue;

                var itemGroupElement = document.createElement('div');
                itemGroupElement.classList.add('inventory-item-group', 'bg-[#202b3d]', 'py-1', 'rounded');

                items.forEach(function(item) {
                    const tilesetImage = assets.load(item.t); // Load tileset image
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

                    // Attach the category ID as data attribute (which is the key in objectData)
                    itemCanvas.setAttribute('data-item-id', category); 

                    const canvasContainer = document.createElement('div');
                    canvasContainer.className = 'flex justify-center items-center w-full h-full max-w-[150px] max-h-[150px] aspect-w-1 aspect-h-1 overflow-hidden';
                    itemCanvas.className += ' w-full h-full object-contain';

                    canvasContainer.appendChild(itemCanvas);
                    itemGroupElement.appendChild(canvasContainer);
                });

                gridContainer.appendChild(itemGroupElement);
            }
        }
    },

    handleObjectClick: function() {
        const gridContainer = document.querySelector('#inventory_grid');
        gridContainer.addEventListener('click', (event) => {
            const clickedItem = event.target.closest('canvas');
            if (clickedItem) {
                const itemId = clickedItem.getAttribute('data-item-id');
                this.cloneObjectToCursor(itemId);
            }
        });
    },

    cloneObjectToCursor: function(itemId) {
        const objectData = assets.load('objectData');

        // Debugging: Check the itemId and its data
        console.log('Cloning Item ID:', itemId);
        console.log('Object Data for Item:', objectData[itemId]);

        const item = objectData[itemId] ? objectData[itemId][0] : null;

        if (!item) {
            console.error('Error: Item not found for ID', itemId);
            return;
        }

        // Store the item to be added when the map is clicked
        window.selectedItemForPlacement = item;

        // Create a canvas to represent the object image that follows the cursor
        const canvas = document.createElement('canvas');
        canvas.width = 32; // Example size, adjust accordingly
        canvas.height = 32;
        const ctx = canvas.getContext('2d');
        const tileImage = assets.load(item.t);

        // Determine which frames to use for rendering
        let framesToRender = [];
        if (item.i && Array.isArray(item.i)) {
            if (typeof item.i[0] === 'string' && item.i[0].includes('-')) {
                framesToRender = render.parseRange(item.i[0]);
            } else if (Array.isArray(item.i[0])) {
                framesToRender = item.i.flat();
            } else {
                framesToRender = item.i;
            }
        }

        // Draw the first frame of the object
        const frame = framesToRender[0]; // Use the first frame by default
        const srcX = (frame % 150) * 16;
        const srcY = Math.floor(frame / 150) * 16;

        ctx.drawImage(tileImage, srcX, srcY, 16, 16, 0, 0, 32, 32);

        // Attach the canvas to the document body
        canvas.style.position = 'absolute';
        canvas.style.pointerEvents = 'none'; // Allow clicks through the object
        document.body.appendChild(canvas);

        // Update canvas position based on mouse movement
        document.addEventListener('mousemove', function moveObject(event) {
            canvas.style.left = `${event.pageX - 16}px`;
            canvas.style.top = `${event.pageY - 16}px`;
        });

        // Listen for a click on the game map to place the object
        document.addEventListener('mousedown', function placeObject(event) {
            if (window.selectedItemForPlacement) {
                inventory_template_window.addObjectToRoomData(window.selectedItemForPlacement, event);
                window.selectedItemForPlacement = null;
                document.body.removeChild(canvas);
                document.removeEventListener('mousemove', moveObject);
                document.removeEventListener('mousedown', placeObject);
            }
        });
    },

    addObjectToRoomData: function(item, event) {
        const rect = game.canvas.getBoundingClientRect();
        const mouseX = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
        const mouseY = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;
        const x = Math.floor(mouseX / 16);
        const y = Math.floor(mouseY / 16);

        const newItem = {
            id: item.id,
            x: [x],
            y: [y],
            animationState: [{
                currentFrame: 0,
                elapsedTime: 0
            }],
            zIndex: [2], // Example zIndex, adjust accordingly
            type: item.type || "item"
        };

        // Add the new item to the room data
        game.roomData.items.push(newItem);
        console.log(`Added ${item.n} to game at (${x}, ${y})`);

        // Update the game rendering
        collision.createWalkableGrid();
        game.render();
    }
};

// Start the inventory window
inventory_template_window.start();


    </script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
