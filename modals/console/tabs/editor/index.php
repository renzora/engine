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
    this.displayItems();
    modal.hideAll();
    modal.load('editor');
  },

  displayItems: function() {
    var itemData = assets.load('objectData');
    console.log('Loaded itemData:', itemData);

    var tilesetImage = assets.load('gen1'); // Directly get the image element

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
          let tileIndexesArray = Array.isArray(item.i[0]) ? item.i[0] : item.i;

          tileIndexesArray.forEach((tileIndex, i) => {
            var tileX = (tileIndex % tilesPerRow) * tileSize;
            var tileY = Math.floor(tileIndex / tilesPerRow) * tileSize;

            var canvasX = (item.a[i] - minX) * tileSize;
            var canvasY = (item.b[i] - minY) * tileSize;

            console.log(tileIndex);

            context.drawImage(tilesetImage, tileX, tileY, tileSize, tileSize, canvasX, canvasY, tileSize, tileSize);
          });
        });

        var itemElement = document.createElement('div');
        itemElement.classList.add('inventory-item', 'm-1');
        itemElement.style.position = 'relative';
        itemElement.dataset.category = category;

        if (itemWidth === tileSize && itemHeight === tileSize) {
          itemElement.style.width = `${fixedHeight}px`;
          itemElement.style.height = `${fixedHeight}px`;
        } else {
          itemElement.style.width = `${itemWidth}px`;
          itemElement.style.height = `${itemHeight}px`;
        }

        itemElement.appendChild(itemCanvas);
        itemGroupElement.appendChild(itemElement);

        itemGroupElement.style.display = 'flex';
        itemGroupElement.style.justifyContent = 'center';
        itemGroupElement.style.alignItems = 'center';

        gridContainer.appendChild(itemGroupElement);
      }
    }
  },

  unmount: function() {
    editor.teardownClickToActivate();
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
