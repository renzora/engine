<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='inventory_window' class='window window_bg' style='width: 400px; background: #222; border: 0;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#111 1px, transparent 0) !important;'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #222; color: #ede8d6;'>Inventory</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative p-4'>

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
          var tilesetImage = assets.load('1'); // Directly get the image element

          var gridContainer = document.querySelector('.inventory-grid');
          var tileSize = 16;
          var tilesPerRow = 150;

          for (var category in itemData) {
            if (itemData.hasOwnProperty(category)) {
              var itemGroupElement = document.createElement('div');
              itemGroupElement.classList.add('inventory-item-group', 'bg-gray-800', 'p-2', 'text-white', 'rounded', 'mb-2');

              // Determine the bounding box of the object
              var minX = Math.min(...itemData[category].map(item => item.a));
              var minY = Math.min(...itemData[category].map(item => item.b));
              var maxX = Math.max(...itemData[category].map(item => item.a));
              var maxY = Math.max(...itemData[category].map(item => item.b));

              var itemWidth = (maxX - minX + 1) * tileSize;
              var itemHeight = (maxY - minY + 1) * tileSize;

              var itemCanvas = document.createElement('canvas');
              var context = itemCanvas.getContext('2d');
              itemCanvas.width = itemWidth;
              itemCanvas.height = itemHeight;

              itemData[category].forEach(function(item) {
                var tileIndex = item.i;
                var tileX = (tileIndex % tilesPerRow) * tileSize;
                var tileY = Math.floor(tileIndex / tilesPerRow) * tileSize;

                // Position the tile on the canvas based on 'a' and 'b' coordinates
                var canvasX = (item.a - minX) * tileSize;
                var canvasY = (item.b - minY) * tileSize;

                console.log(`Category: ${category}, Tile Index: ${tileIndex}, a: ${item.a}, b: ${item.b}, Canvas Position: (${canvasX}, ${canvasY})`);

                context.drawImage(tilesetImage, tileX, tileY, tileSize, tileSize, canvasX, canvasY, tileSize, tileSize);
              });

              var itemElement = document.createElement('div');
              itemElement.classList.add('inventory-item', 'm-1');

              itemElement.style.width = '100%';
              itemElement.style.height = 'auto';
              itemElement.style.position = 'relative';

              itemElement.appendChild(itemCanvas);
              itemGroupElement.appendChild(itemElement);

              var categoryTitle = document.createElement('div');
              categoryTitle.classList.add('category-title', 'text-white', 'font-bold', 'mb-1');
              categoryTitle.innerText = category;

              gridContainer.appendChild(itemGroupElement);
            }
          }
        },
        unmount: function() {
          // Cleanup if necessary
        }
      }

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
<?php
}
?>
