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
  start: function() {
    this.displayItems();
  },

  displayItems: function() {
    var itemData = assets.load('objectData');
    console.log('Loaded itemData:', itemData);

    var gridContainer = document.querySelector('.inventory-grid');
    var tileSize = 16;
    var tilesPerRow = 150;

    for (var category in itemData) {
        if (itemData.hasOwnProperty(category)) {
            var items = itemData[category];
            if (items.length === 0) continue;

            console.log('Processing category:', category);
            console.log('Items in category:', items);

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

                const canvasContainer = document.createElement('div');
                canvasContainer.className = 'flex justify-center items-center w-full h-full max-w-[150px] max-h-[150px] aspect-w-1 aspect-h-1 overflow-hidden';
                itemCanvas.className += ' w-full h-full object-contain';

                canvasContainer.appendChild(itemCanvas);
                itemGroupElement.appendChild(canvasContainer);
            });

            itemGroupElement.style.display = 'flex';
            itemGroupElement.style.justifyContent = 'center';
            itemGroupElement.style.alignItems = 'center';

            gridContainer.appendChild(itemGroupElement);
        }
    }
  },

  unmount: function() {
    modal.close('edit_window');
  }
};

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
