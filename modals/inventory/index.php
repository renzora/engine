<div data-window='inventory_window' class='window window_bg' style='width: 400px; background: #222; border: 0;'>
  <div data-part='handle' class='window_title' style='background-image: radial-gradient(#111 1px, transparent 0) !important;'>
    <div class='float-right'>
      <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
    </div>
    <div data-part='title' class='title_bg window_border' style='background: #222; color: #ede8d6;'>Inventory</div>
  </div>
  <div class='clearfix'></div>
  <div class='window_body relative p-4'>
    <!-- Tabbed Menu -->
    <div class="tabs mb-4">
      <input type="text" class="p-2 mb-3 w-full" style="background: #333;" placeholder="search..." />
      <div class="flex">
        <button class="tab-button px-4 py-2 bg-blue-500 text-white" data-category="category1">Objects</button>
      </div>
    </div>

    <!-- Inventory Grids -->
    <div class="inventory-grid grid grid-cols-4 gap-2" style="height: 400px; overflow-y: auto;">
      <!-- Items will be appended here dynamically -->
    </div>
  </div>

  <script>
var inventory_window = {
  start: function() {
    var itemData = assets.load('objectData');
    console.log('Loaded itemData:', itemData);

    var tilesetImage = assets.load('1'); // Directly get the image element

    var gridContainer = document.querySelector('.inventory-grid');
    var tileSize = 16;
    var tilesPerRow = 150;

    for (var category in itemData) {
      if (itemData.hasOwnProperty(category)) {
        var items = itemData[category]; // Access the array of items in the category
        if (items.length === 0) continue;

        console.log('Processing category:', category);
        console.log('Items in category:', items);

        var itemGroupElement = document.createElement('div');
        itemGroupElement.classList.add('inventory-item-group', 'bg-gray-800', 'p-2', 'text-white', 'rounded', 'mb-2');

        // Determine the bounding box of the object
        var allA = items.map(item => item.a).flat();
        var allB = items.map(item => item.b).flat();
        
        var minX = Math.min(...allA);
        var minY = Math.min(...allB);
        var maxX = Math.max(...allA);
        var maxY = Math.max(...allB);

        console.log('Bounding box for category:', category, { minX, minY, maxX, maxY });

        var itemWidth = (maxX - minX + 1) * tileSize;
        var itemHeight = (maxY - minY + 1) * tileSize;

        var itemCanvas = document.createElement('canvas');
        var context = itemCanvas.getContext('2d');
        itemCanvas.width = itemWidth;
        itemCanvas.height = itemHeight;

        items.forEach(function(item, index) {
          item.i.forEach((tileIndex, i) => {
            var tileX = (tileIndex % tilesPerRow) * tileSize;
            var tileY = Math.floor(tileIndex / tilesPerRow) * tileSize;

            // Position the tile on the canvas based on 'a' and 'b' coordinates
            var canvasX = (item.a[i] - minX) * tileSize;
            var canvasY = (item.b[i] - minY) * tileSize;

            console.log('Drawing tile for item:', item, { tileIndex, tileX, tileY, canvasX, canvasY });

            context.drawImage(tilesetImage, tileX, tileY, tileSize, tileSize, canvasX, canvasY, tileSize, tileSize);
          });
        });

        var itemElement = document.createElement('div');
        itemElement.classList.add('inventory-item', 'm-1');
        itemElement.style.width = `${itemWidth}px`;
        itemElement.style.height = `${itemHeight}px`;
        itemElement.style.position = 'relative';
        itemElement.dataset.category = category; // Set the category as data attribute

        itemElement.appendChild(itemCanvas);
        itemGroupElement.appendChild(itemElement);

        var categoryTitle = document.createElement('div');
        categoryTitle.classList.add('category-title', 'text-white', 'font-bold', 'mb-1');
        categoryTitle.innerText = category;

        itemGroupElement.appendChild(categoryTitle);
        gridContainer.appendChild(itemGroupElement);
      }
    }

    this.setupDragAndDrop(tileSize);
  },

  setupDragAndDrop: function(tileSize) {
    let selectedItem = null;
    let offsetX, offsetY;

    const onMouseDown = function(event) {
      const originalItem = event.currentTarget;
      selectedItem = originalItem.cloneNode(true);

      // Clone the canvas from the original item to the selected item
      const originalCanvas = originalItem.querySelector('canvas');
      const clonedCanvas = selectedItem.querySelector('canvas');
      clonedCanvas.width = originalCanvas.width;
      clonedCanvas.height = originalCanvas.height;
      clonedCanvas.getContext('2d').drawImage(originalCanvas, 0, 0);

      selectedItem.style.position = 'absolute';
      selectedItem.style.pointerEvents = 'none';
      selectedItem.style.zIndex = 1000;
      selectedItem.dataset.category = originalItem.dataset.category; // Set the category on the clone

      document.body.appendChild(selectedItem);

      offsetX = event.offsetX;
      offsetY = event.offsetY;

      moveSelectedItem(event);
    };

    const onMouseMove = function(event) {
      if (selectedItem) {
        moveSelectedItem(event);
      }
    };

    const onMouseUp = function(event) {
      if (selectedItem) {
        const zoomLevel = game.zoomLevel;
        const cameraX = game.cameraX;
        const cameraY = game.cameraY;

        const dropX = (event.clientX - offsetX + window.scrollX) / zoomLevel + cameraX;
        const dropY = (event.clientY - offsetY + window.scrollY) / zoomLevel + cameraY;

        const snappedX = Math.round(dropX / tileSize);
        const snappedY = Math.round(dropY / tileSize);

        console.log(`Dropped position: X=${snappedX}, Y=${snappedY}`);

        const newItem = {
          id: selectedItem.dataset.category, // Correctly assign the category as the ID
          x: [],
          y: []
        };

        calculateTilePositions(selectedItem, snappedX, snappedY, tileSize, newItem.x, newItem.y);

        addNewItemToRoomData(newItem);

        selectedItem.remove();
        selectedItem = null;
      }
    };

    function moveSelectedItem(event) {
      const zoomLevel = game.zoomLevel;
      const cameraX = game.cameraX;
      const cameraY = game.cameraY;

      const canvasX = (event.clientX - offsetX + window.scrollX) / zoomLevel + cameraX;
      const canvasY = (event.clientY - offsetY + window.scrollY) / zoomLevel + cameraY;

      const snappedX = Math.round(canvasX / tileSize) * tileSize;
      const snappedY = Math.round(canvasY / tileSize) * tileSize;

      selectedItem.style.left = `${(snappedX - cameraX) * zoomLevel}px`;
      selectedItem.style.top = `${(snappedY - cameraY) * zoomLevel}px`;

      selectedItem.style.transform = `scale(${zoomLevel})`;
      selectedItem.style.transformOrigin = 'top left';

      console.log(`Cursor position: X=${event.clientX}, Y=${event.clientY}`);
      console.log(`Snapped position: X=${snappedX}, Y=${snappedY}`);
    }

    function calculateTilePositions(item, baseX, baseY, tileSize, xArray, yArray) {
      const canvas = item.querySelector('canvas');
      const width = canvas.width;
      const height = canvas.height;
      const cols = width / tileSize;
      const rows = height / tileSize;

      // Generate unique x and y coordinates
      for (let col = 0; col < cols; col++) {
        xArray.push(baseX + col);
      }
      for (let row = 0; row < rows; row++) {
        yArray.push(baseY + row);
      }
    }

    function addNewItemToRoomData(item) {
      game.roomData.items.push(item);
      console.log('New item added to roomData:', item);
      saveRoomData();
    }

    function saveRoomData() {
      const dataToSend = JSON.stringify(game.roomData);
      console.log('Data being sent to server:', dataToSend); // Log the data being sent

      ui.ajax({
        outputType: 'json',
        method: 'POST',
        url: 'modals/inventory/ajax/save_map.php',
        data: dataToSend,
        headers: {
          'Content-Type': 'application/json' // Ensure the content type is JSON
        },
        success: function(data) {
          console.log('Room data saved successfully:', data);
        },
        error: function(data) {
          console.error('Error saving room data:', data);
        }
      });
    }

    this.mouseDownHandler = onMouseDown;
    this.mouseMoveHandler = onMouseMove;
    this.mouseUpHandler = onMouseUp;

    document.querySelectorAll('.inventory-item').forEach(item => {
      item.addEventListener('mousedown', this.mouseDownHandler);
    });

    document.addEventListener('mousemove', this.mouseMoveHandler);
    document.addEventListener('mouseup', this.mouseUpHandler);
  },

  unmount: function() {
    document.querySelectorAll('.inventory-item').forEach(item => {
      item.removeEventListener('mousedown', this.mouseDownHandler);
    });

    document.removeEventListener('mousemove', this.mouseMoveHandler);
    document.removeEventListener('mouseup', this.mouseUpHandler);
  }
};

// Initialize the inventory window
inventory_window.start();
  </script>

  <style>
    .inventory-item canvas {
      width: 100%; /* Ensure the canvas takes the full width of the container */
      height: auto; /* Maintain the aspect ratio */
      display: block;
    }
  </style>

  <div class='resize-handle'></div>
</div>