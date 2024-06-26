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
var ui_editor_tab_window = {
  start: function() {
    var itemData = assets.load('objectData');
    console.log('Loaded itemData:', itemData);

    var tilesetImage = assets.load('1'); // Directly get the image element

    var gridContainer = document.querySelector('.inventory-grid');
    var tileSize = 16;
    var tilesPerRow = 150;
    var fixedHeight = 32; // Height for single-tile items in the inventory

    for (var category in itemData) {
      if (itemData.hasOwnProperty(category)) {
        var items = itemData[category]; // Access the array of items in the category
        if (items.length === 0) continue;

        console.log('Processing category:', category);
        console.log('Items in category:', items);

        var itemGroupElement = document.createElement('div');
        itemGroupElement.classList.add('inventory-item-group', 'bg-[#202b3d]', 'py-1', 'rounded');

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
        itemElement.style.position = 'relative';
        itemElement.dataset.category = category; // Set the category as data attribute

        if (itemWidth === tileSize && itemHeight === tileSize) {
          // Single-tile item: Scale up to fixedHeight
          itemElement.style.width = `${fixedHeight}px`;
          itemElement.style.height = `${fixedHeight}px`;
        } else {
          // Multi-tile item: Keep original size
          itemElement.style.width = `${itemWidth}px`;
          itemElement.style.height = `${itemHeight}px`;
        }

        itemElement.appendChild(itemCanvas);
        itemGroupElement.appendChild(itemElement);

        // Center the items within the grid
        itemGroupElement.style.display = 'flex';
        itemGroupElement.style.justifyContent = 'center';
        itemGroupElement.style.alignItems = 'center';

        gridContainer.appendChild(itemGroupElement);
      }
    }

    this.setupClickToActivate(tileSize);
  },

  setupClickToActivate: function(tileSize) {
    let selectedItem = null;
    let activeItemGroup = null;
    let offsetX, offsetY;

    const uiMenu = document.querySelector('[data-window="ui_window"]');

    const onClick = function(event) {
      if (event.target.closest('.tabs')) {
        return;
      }

      const originalItem = event.currentTarget;
      const originalCanvas = originalItem.querySelector('canvas');
      const zoomLevel = game.zoomLevel;

      if (activeItemGroup) {
        activeItemGroup.classList.remove('active');
      }

      if (selectedItem) {
        selectedItem.remove();
      }

      selectedItem = document.createElement('div');
      selectedItem.classList.add('inventory-item-clone');
      selectedItem.style.position = 'absolute';
      selectedItem.style.pointerEvents = 'none';
      selectedItem.style.zIndex = 1000;
      selectedItem.dataset.category = originalItem.dataset.category;

      const clonedCanvas = document.createElement('canvas');
      clonedCanvas.width = originalCanvas.width;
      clonedCanvas.height = originalCanvas.height;
      clonedCanvas.getContext('2d').drawImage(originalCanvas, 0, 0);

      selectedItem.appendChild(clonedCanvas);
      document.body.appendChild(selectedItem);

      offsetX = originalCanvas.width / 2;
      offsetY = originalCanvas.height / 2;

      activeItemGroup = originalItem.closest('.inventory-item-group');
      activeItemGroup.classList.add('active');

      moveSelectedItem(event);
      game.pathfinding = false; // Disable pathfinding when an item is selected
      modal.hide('ui_window');
      modal.hide('quick_menu_window');
    };

    const onMouseMove = function(event) {
      if (selectedItem) {
        if (uiMenu.contains(event.target)) {
          selectedItem.style.display = 'none';
        } else {
          selectedItem.style.display = 'block';
          moveSelectedItem(event);
        }
      }
    };

    const onMouseUp = function(event) {
      if (selectedItem && !uiMenu.contains(event.target) && event.button === 0) {
        const zoomLevel = game.zoomLevel;
        const cameraX = game.cameraX;
        const cameraY = game.cameraY;

        const dropX = (event.clientX - offsetX + window.scrollX) / zoomLevel + cameraX;
        const dropY = (event.clientY - offsetY + window.scrollY) / zoomLevel + cameraY;

        const snappedX = Math.round(dropX / tileSize);
        const snappedY = Math.round(dropY / tileSize);

        console.log(`Dropped position: X=${snappedX}, Y=${snappedY}`);

        const newItem = {
          id: selectedItem.dataset.category,
          x: [],
          y: []
        };

        if (event.shiftKey) {
          addItemsInLine(newItem, snappedX, snappedY, tileSize);
        } else {
          calculateTilePositions(selectedItem, snappedX, snappedY, tileSize, newItem.x, newItem.y);
          addNewItemToRoomData(newItem);
        }
      } else if (event.button === 2) {
        // Right click: Deactivate the selected item
        if (activeItemGroup) {
          activeItemGroup.classList.remove('active');
        }
        if (selectedItem) {
          selectedItem.remove();
          selectedItem = null;
          game.pathfinding = true; // Enable pathfinding when an item is deselected
          modal.show('quick_menu_window');
          modal.show('ui_window');
        }
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

      selectedItem.style.left = `${(snappedX - cameraX) * zoomLevel + window.scrollX}px`;
      selectedItem.style.top = `${(snappedY - cameraY) * zoomLevel + window.scrollY}px`;

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

      for (let col = 0; col < cols; col++) {
        xArray.push(baseX + col);
      }
      for (let row = 0; row < rows; row++) {
        yArray.push(baseY + row);
      }
    }

    function addItemsInLine(item, endX, endY, tileSize) {
      if (!game.previousDrop) {
        game.previousDrop = { x: endX, y: endY };
        calculateTilePositions(selectedItem, endX, endY, tileSize, item.x, item.y);
        addNewItemToRoomData(item);
        return;
      }

      const startX = game.previousDrop.x;
      const startY = game.previousDrop.y;

      const stepX = endX > startX ? 1 : -1;
      const stepY = endY > startY ? 1 : -1;

      if (startX === endX) {
        for (let y = startY; y !== endY + stepY; y += stepY) {
          calculateTilePositions(selectedItem, endX, y, tileSize, item.x, item.y);
          addNewItemToRoomData({ ...item, y: [y], x: [endX] });
        }
      } else if (startY === endY) {
        for (let x = startX; x !== endX + stepX; x += stepX) {
          calculateTilePositions(selectedItem, x, endY, tileSize, item.x, item.y);
          addNewItemToRoomData({ ...item, y: [endY], x: [x] });
        }
      } else {
        // Handle diagonal placement if needed
      }

      game.previousDrop = { x: endX, y: endY };
    }

    function addNewItemToRoomData(item) {
      if (!game.roomData.items) {
        game.roomData.items = [];
      }
      game.roomData.items.push(item);

      effects.shakeMap(300, 3);

      effects.createParticles(item.x[0] * 16, item.y[0] * 16, {
        colors: ['rgba(0, 0, 255, 1)', 'rgba(0, 255, 255, 1)', 'rgba(255, 0, 0, 1)', 'rgba(255, 255, 0, 1)', 'rgba(0, 255, 0, 1)', 'rgba(255, 165, 0, 1)', 'rgba(128, 0, 128, 1)'],
            count: 32,
            speed: 1,
            life: 60,
            size: 1,
            spread: Math.PI * 2, // Full circle
            type: 'default'
        });


      console.log('New item added to roomData:', item);
      saveRoomData();
    }

    function saveRoomData() {
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
        success: function(data) {
          console.log('Room data saved successfully:', data);
        },
        error: function(data) {
          console.error('Error saving room data:', data);
        }
      });
    }

    this.clickHandler = onClick;
    this.mouseMoveHandler = onMouseMove;
    this.mouseUpHandler = onMouseUp;

    document.querySelectorAll('.inventory-item').forEach(item => {
      item.addEventListener('click', this.clickHandler);
    });

    document.addEventListener('mousemove', this.mouseMoveHandler);
    document.addEventListener('mouseup', this.mouseUpHandler);
    document.addEventListener('contextmenu', function(event) {
      event.preventDefault(); // Prevent the context menu from appearing
    });
  },

  unmount: function() {
        document.querySelectorAll('.inventory-item').forEach(item => {
            item.removeEventListener('click', this.clickHandler);
        });

        document.removeEventListener('mousemove', this.mouseMoveHandler);
        document.removeEventListener('mouseup', this.mouseUpHandler);
        game.pathfinding = true;
        modal.show('ui_window');
        modal.show('quick_menu_window');

    }
};

</script>

<style>
  .inventory-item canvas {
    width: 100%; /* Ensure the canvas takes the full width of the container */
    height: auto; /* Maintain the aspect ratio */
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
    background-color: #4CAF50; /* Green background for active item */
  }
</style>

<?php
}
?>
