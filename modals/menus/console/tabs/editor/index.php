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
  clonedObject: null,
  isDraggingObject: false,
  selectedItem: null, // To track the selected item from the inventory

  start: function() {
    this.displayItems();

    // Wait until the game canvas is initialized before adding the event listener
    this.waitForCanvasInitialization();
  },

  waitForCanvasInitialization: function() {
    const interval = setInterval(() => {
      if (game.canvas) {
        clearInterval(interval); // Clear the interval once the canvas is available

        // Add event listener to the game canvas for placing items
        game.canvas.addEventListener('mouseup', this.handleMapClick.bind(this));
        game.canvas.addEventListener('wheel', this.handleZoomChange.bind(this)); // Zoom handling
        console.log("Canvas initialized, event listeners added.");
      }
    }, 100); // Check every 100ms if the game canvas is initialized
  },

  handleMapClick: function(event) {
    console.log("Map clicked at:", event.clientX, event.clientY); // Log the click position on the map

    if (this.isDraggingObject && this.clonedObject) {
        // Remove the cloned object from the DOM
        document.body.removeChild(this.clonedObject);
        this.isDraggingObject = false;
        this.clonedObject = null;

        // Convert mouse position to game coordinates using the existing game.canvas
        const rect = game.canvas.getBoundingClientRect();
        const mouseX = (event.clientX - rect.left) / game.zoomLevel + camera.cameraX;
        const mouseY = (event.clientY - rect.top) / game.zoomLevel + camera.cameraY;
        const x = Math.floor(mouseX / 16); // Assuming tiles are 16px
        const y = Math.floor(mouseY / 16);

        console.log("Converted game coordinates:", { x, y }); // Log the game coordinates

        // Add the selected object to the map's room data (game.roomData)
        if (this.selectedItem) {
            const itemId = this.selectedItem.getAttribute('data-item-id'); // Get the item ID

            if (itemId) {
                // JSON structure to add to game.roomData
                const newObject = {
                    id: itemId,
                    x: [x],
                    y: [y],
                    type: 'item'
                };

                // Push the new object to game.roomData.items
                game.roomData.items.push(newObject); 

                // Trigger a game re-render to display the new object
                game.render(); 

                console.log("Added object to map:", newObject); // Log the added object details
            } else {
                console.error("Failed to retrieve item ID.");
            }
        }
    }
  },

  displayItems: function() {
    var itemData = assets.load('objectData'); // Assume we have object data loaded from assets
    console.log('Loaded itemData:', itemData);

    var gridContainer = document.querySelector('.inventory-grid');
    var tileSize = 16;
    var tilesPerRow = 150;

    for (var category in itemData) {
        if (itemData.hasOwnProperty(category)) {
            var items = itemData[category];
            if (items.length === 0) continue;

            var itemGroupElement = document.createElement('div');
            itemGroupElement.classList.add('inventory-item-group', 'bg-[#202b3d]', 'py-1', 'rounded');

            items.forEach(function(item) {
                const tilesetImage = assets.load(item.t); // Load the tileset image
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

                // Attach the item ID as a data attribute for tracking
                itemCanvas.setAttribute('data-item-id', item.id); 
                const canvasContainer = document.createElement('div');
                canvasContainer.className = 'flex justify-center items-center w-full h-full max-w-[150px] max-h-[150px] aspect-w-1 aspect-h-1 overflow-hidden';
                itemCanvas.className += ' w-full h-full object-contain';

                canvasContainer.appendChild(itemCanvas);
                itemGroupElement.appendChild(canvasContainer);
            });

            gridContainer.appendChild(itemGroupElement);
        }
    }

    // Set up dragging functionality
    this.setupInventoryDrag();
  },

  setupInventoryDrag: function() {
    var self = this;
    document.querySelectorAll('.inventory-item-group canvas').forEach(item => {
        item.addEventListener('mousedown', function(event) {
            self.createCloneAndFollowMouse(event, item);
        });
    });
  },

  createCloneAndFollowMouse: function(event, item) {
    const canvas = document.createElement('canvas');
    const ctx = canvas.getContext('2d');
    const itemCanvas = item; // Canvas from the inventory item

    // Set canvas dimensions to match the original object
    canvas.width = itemCanvas.width * game.zoomLevel; // Scale the canvas based on game.zoomLevel
    canvas.height = itemCanvas.height * game.zoomLevel; // Scale the canvas based on game.zoomLevel

    // Draw the clone object on the new canvas, taking into account the zoom level
    ctx.drawImage(itemCanvas, 0, 0, canvas.width, canvas.height);

    // Append the clone canvas to the document body
    document.body.appendChild(canvas);
    this.clonedObject = canvas;
    this.isDraggingObject = true;
    this.selectedItem = item; // Store the selected item

    // Follow the mouse movement
    document.addEventListener('mousemove', this.moveCloneWithMouse.bind(this));
  },

  moveCloneWithMouse: function(event) {
    if (this.isDraggingObject && this.clonedObject) {
        const zoomFactor = game.zoomLevel; // Get the zoom level from the game

        // Position the cloned object in the center of the cursor, taking the zoom level into account
        this.clonedObject.style.position = 'absolute';
        this.clonedObject.style.left = `${event.pageX - (this.clonedObject.width / 2)}px`;
        this.clonedObject.style.top = `${event.pageY - (this.clonedObject.height / 2)}px`;
    }
  },

  handleZoomChange: function(event) {
    const delta = Math.sign(event.deltaY); // Get scroll direction (positive or negative)
    const newZoomLevel = Math.max(2, Math.min(10, game.zoomLevel - delta)); // Ensure zoom level stays within 2 and 10

    // Adjust the game zoom level
    game.zoomLevel = newZoomLevel;
    console.log("Zoom level changed:", game.zoomLevel);

    // Adjust cloned object size to match the new zoom level
    if (this.isDraggingObject && this.clonedObject) {
        this.clonedObject.width = this.selectedItem.width * game.zoomLevel;
        this.clonedObject.height = this.selectedItem.height * game.zoomLevel;
    }
  }
};

// Start the inventory window
ui_console_tab_window.start();

</script>


<style>
  .inventory-item canvas {
    width: 100%;
    height: auto;
    display: block;
  }
  .inventory-item-group {
    display: flex;
    justify-content: center;
    align-items: center;
  }
  .inventory-item-clone canvas {
    width: auto;
    height: auto;
  }
  .inventory-item-group.active {
    background-color: #4CAF50; 
  }
</style>

<?php
}
?>
