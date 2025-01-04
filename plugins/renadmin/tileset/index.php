<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='tileset_window' class='window window_bg' style='width: 800px;height: 700px; background: #323e69;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#192037 1px, transparent 0) !important;'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #323e69; color: #ede8d6;'>Tileset Manager</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2' style='height: 660px; overflow-y: hidden;'>

        <div id="tileset_window_tabs" style="height: 100%;">
        <div id="tabs" class="flex border-b border-gray-300">
    <button class="tab text-gray-800 p-2" data-tab="tab1">Upload</button>
    <button class="tab text-gray-800 p-2" data-tab="items">Items</button> <!-- New tab -->
</div>

          <div class="tab-content mt-2 hidden" data-tab-content="tab1" style="height: calc(100% - 35px);">
            <div class="flex h-full">
              <div class="w-8/12 p-2 pl-0 flex flex-col">
                <!-- Left column content -->
                <div id="drop_zone" class="flex-grow w-full border border-gray-300 rounded overflow-auto" style="position: relative;">
                  <p id="drop_prompt">Drop an image here to upload</p>
                  <canvas id="uploaded_canvas" style="display: none;"></canvas>
                  <canvas id="gridCanvas" style="display: none; position: absolute; top: 0; left: 0;"></canvas>
                  <canvas id="selectionCanvas" style="display: none; position: absolute; top: 0; left: 0;"></canvas>
                </div>
              </div>
              <div class="w-4/12 p-2" style="height: 100%; overflow-y: scroll;">
                <!-- Right column content -->
                <div id="right_tabs">
                <div class="mb-4">
                  <div class="flex justify-center items-center py-3" style="background: #63A650; position: relative;">
                    <canvas id="previewCanvas" class="max-w-full max-h-full"></canvas>
                    <canvas id="nightFilterCanvas" class="absolute top-0 left-0 w-full h-full"></canvas>
                  </div>

                  <div class="clearfix mt-2">
                  <input type="checkbox" id="night_filter_toggle" class="toggle-night-filter">
                    <label for="night_filter_toggle" class="inline-block text-gray-700">Night Filter:</label>
                </div>
                </div>
                  <div id="tabs" class="flex border-b border-gray-300">
                    <button class="tab text-gray-800 p-2" data-tab="info">Info</button>
                  </div>

                  <div class="tab-content mt-2 hidden" data-tab-content="info">
                    <div class="mb-4">
                      <label for="name" class="block text-gray-700">Name:</label>
                      <input type="text" id="name" name="name" class="w-full p-2 border border-gray-300 rounded disabled:opacity-50">
                    </div>

                    <div class="mb-4">
                    <button id='add_to_tileset' onclick="addToTileset()" class="green_button text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md" style="font-size: 16px;"><i class="fas fa-lock-open"></i> add to tileset</button>
                  </div>
                  </div>

              </div>
            </div>
          </div>
        </div>
        <div class="tab-content mt-2 hidden" data-tab-content="items" style="height: calc(100% - 35px);">
    <!-- Added 'h-full' to ensure the container takes up available space -->
    <div class="grid grid-cols-10 gap-2 p-2 h-full overflow-y-scroll" id="itemsGrid"></div>
  </div>
      </div>

    <style>
#drop_zone {
  display: flex;
  justify-content: center;
  align-items: center;
  position: relative;
}
#drop_zone.dropped {
  justify-content: flex-start;
  align-items: flex-start;
  overflow: auto;
}
#uploaded_canvas, #gridCanvas, #selectionCanvas {
  position: absolute;
  top: 0;
  left: 0;
}
#uploaded_canvas {
  z-index: 1;
}
#gridCanvas {
  z-index: 2;
}
#selectionCanvas {
  z-index: 3;
}
#nightFilterCanvas {
  position: absolute;
  top: 0;
  left: 0;
  z-index: 4;
  display: none; /* Initially hidden */
}
  </style>

<script>
var tileset_window = {
    selectedTiles: [],
    nightFilterEnabled: false,
    imageCanvas: document.getElementById('uploaded_canvas'),
    start: function() {
        ui.initTabs('tileset_window_tabs', 'tab1');
        ui.initTabs('right_tabs', 'info');
        document.querySelector('button[data-tab="items"]').addEventListener('click', this.displayTilesetItems);

        var dropZone = document.getElementById('drop_zone');
        var dropPrompt = document.getElementById('drop_prompt');
        var imageCanvas = tileset_window.imageCanvas;
        var gridCanvas = document.getElementById('gridCanvas');
        var selectionCanvas = document.getElementById('selectionCanvas');
        var previewCanvas = document.getElementById('previewCanvas');
        var nightFilterCanvas = document.getElementById('nightFilterCanvas');
        var ctxImage = imageCanvas.getContext('2d');
        var ctxGrid = gridCanvas.getContext('2d');
        var ctxSelection = selectionCanvas.getContext('2d');
        var ctxPreview = previewCanvas.getContext('2d');
        var ctxNightFilter = nightFilterCanvas.getContext('2d');

        var isDragging = false;
        var startX, startY, scrollLeft, scrollTop;
        var ctrlPressed = false;
        var middleMousePressed = false;
        var scale = 1;

        document.addEventListener('keydown', function(e) {
            if (e.key === 'Control') {
                ctrlPressed = true;
            }
        });

        document.addEventListener('keyup', function(e) {
            if (e.key === 'Control') {
                ctrlPressed = false;
            }
        });

        dropZone.addEventListener('dragover', function(e) {
            e.preventDefault();
            dropZone.style.borderColor = '#000';
        });

        dropZone.addEventListener('dragleave', function(e) {
            e.preventDefault();
            dropZone.style.borderColor = '#ccc';
        });

        dropZone.addEventListener('drop', function(e) {
            e.preventDefault();
            dropZone.style.borderColor = '#ccc';
            var files = e.dataTransfer.files;
            if (files.length > 0) {
                var reader = new FileReader();
                reader.onload = function(event) {
                    var img = new Image();
                    img.onload = function() {
                        imageCanvas.width = img.width;
                        imageCanvas.height = img.height;
                        gridCanvas.width = img.width;
                        gridCanvas.height = img.height;
                        selectionCanvas.width = img.width;
                        selectionCanvas.height = img.height;
                        ctxImage.drawImage(img, 0, 0, img.width, img.height);
                        imageCanvas.style.display = 'block';
                        gridCanvas.style.display = 'block';
                        selectionCanvas.style.display = 'block';
                        dropPrompt.style.display = 'none';
                        dropZone.classList.add('dropped');
                        drawGrid();
                        console.log("Image loaded and canvases updated.");

                        // Disable the form fields after a new image is dropped
                        disableFormFields();
                    }
                    img.src = event.target.result;
                };
                reader.readAsDataURL(files[0]);
            }
        });

        function drawGrid() {
            ctxGrid.clearRect(0, 0, gridCanvas.width, gridCanvas.height);
            ctxGrid.strokeStyle = '#000000';
            ctxGrid.lineWidth = 0.5;

            for (var x = 0; x <= gridCanvas.width; x += 16) {
                ctxGrid.moveTo(x, 0);
                ctxGrid.lineTo(x, gridCanvas.height);
            }

            for (var y = 0; y <= gridCanvas.height; y += 16) {
                ctxGrid.moveTo(0, y);
                ctxGrid.lineTo(gridCanvas.width, y);
            }

            ctxGrid.stroke();
            console.log("Grid drawn.");
        }

        function getCanvasCoordinates(e, canvas) {
            const rect = canvas.getBoundingClientRect();
            const x = (e.clientX - rect.left) / scale;
            const y = (e.clientY - rect.top) / scale;
            return { x, y };
        }

        selectionCanvas.addEventListener('mousedown', function(e) {
            console.log("Canvas clicked:", e.target.id);
            if (e.button === 1) {
                middleMousePressed = true;
                const coords = getCanvasCoordinates(e, selectionCanvas);
                startX = coords.x;
                startY = coords.y;
                scrollLeft = dropZone.scrollLeft;
                scrollTop = dropZone.scrollTop;
                console.log("Middle mouse button pressed:", startX, startY);
                e.preventDefault();
            } else if (e.button === 0) {
                isDragging = true;
                const coords = getCanvasCoordinates(e, selectionCanvas);
                startX = coords.x;
                startY = coords.y;
                console.log("Left mouse button pressed:", startX, startY);

                tileset_window.selectedTiles = [];
            }
        });

        selectionCanvas.addEventListener('mouseleave', function() {
            isDragging = false;
            middleMousePressed = false;
            console.log("Mouse left the canvas");
        });

        selectionCanvas.addEventListener('mouseup', function(e) {
            console.log("Mouse button released");
            if (e.button === 0) {
                isDragging = false;
                const coords = getCanvasCoordinates(e, selectionCanvas);
                const endX = coords.x;
                const endY = coords.y;
                console.log("Mouse up position:", endX, endY);

                const tileSize = 16;
                const startXTile = Math.floor(startX / tileSize) * tileSize;
                const startYTile = Math.floor(startY / tileSize) * tileSize;
                const endXTile = Math.floor(endX / tileSize) * tileSize;
                const endYTile = Math.floor(endY / tileSize) * tileSize;

                const x = Math.min(startXTile, endXTile);
                const y = Math.min(startYTile, endYTile);
                const width = Math.abs(endXTile - startXTile) + tileSize;
                const height = Math.abs(endYTile - startYTile) + tileSize;

                // Clear previous selections
                tileset_window.selectedTiles = [];

                console.log("Selection area:", {
                    x: x, 
                    y: y, 
                    width: width, 
                    height: height
                });

                // Calculate the number of tiles to be added
                const numCols = width / tileSize;
                const numRows = height / tileSize;

                for (let row = 0; row < numRows; row++) {
                    for (let col = 0; col < numCols; col++) {
                        const tile = { 
                            x: x + col * tileSize, 
                            y: y + row * tileSize, 
                            width: tileSize, 
                            height: tileSize 
                        };
                        tileset_window.selectedTiles.push(tile);
                        console.log("Adding tile:", tile);
                    }
                }

                console.log("Final selected tiles:", tileset_window.selectedTiles);
                drawSelection(x, y, width, height);

                // Enable the form fields after tiles are selected
                enableFormFields();

                // Update the preview canvas
                updatePreviewCanvas();
            }
        });

        selectionCanvas.addEventListener('mousemove', function(e) {
            if (middleMousePressed) {
                e.preventDefault();
                const coords = getCanvasCoordinates(e, selectionCanvas);
                const x = coords.x;
                const y = coords.y;
                var walkX = (x - startX) * scale;
                var walkY = (y - startY) * scale;
                dropZone.scrollLeft = scrollLeft - walkX;
                dropZone.scrollTop = scrollTop - walkY;
                console.log("Middle mouse drag:", walkX, walkY);
            } else if (isDragging) {
                e.preventDefault();
                const coords = getCanvasCoordinates(e, selectionCanvas);
                const x = coords.x;
                const y = coords.y;
                console.log("Mouse move position:", x, y);

                const tileSize = 16;
                const startXTile = Math.floor(startX / tileSize) * tileSize;
                const startYTile = Math.floor(startY / tileSize) * tileSize;
                const currentXTile = Math.floor(x / tileSize) * tileSize;
                const currentYTile = Math.floor(y / tileSize) * tileSize;

                const selectionX = Math.min(startXTile, currentXTile);
                const selectionY = Math.min(startYTile, currentYTile);
                const selectionWidth = Math.abs(currentXTile - startXTile) + tileSize;
                const selectionHeight = Math.abs(currentYTile - startYTile) + tileSize;

                console.log("Dragging selection:", selectionX, selectionY, selectionWidth, selectionHeight);
                drawSelection(selectionX, selectionY, selectionWidth, selectionHeight);
            }
        });

        dropZone.addEventListener('wheel', function(e) {
            if (ctrlPressed) {
                e.preventDefault();
                var rect = imageCanvas.getBoundingClientRect();
                var offsetX = e.clientX - rect.left;
                var offsetY = e.clientY - rect.top;

                var delta = e.deltaY > 0 ? -0.1 : 0.1;
                var previousScale = scale;
                scale += delta;
                if (scale < 0.5) scale = 0.5;
                if (scale > 3) scale = 3;
                imageCanvas.style.transform = `scale(${scale})`;
                gridCanvas.style.transform = `scale(${scale})`;
                selectionCanvas.style.transform = `scale(${scale})`;
                imageCanvas.style.transformOrigin = 'top left';
                gridCanvas.style.transformOrigin = 'top left';
                selectionCanvas.style.transformOrigin = 'top left';

                var newScrollLeft = (offsetX * scale / previousScale) - offsetX;
                var newScrollTop = (offsetY * scale / previousScale) - offsetY;

                dropZone.scrollLeft += newScrollLeft;
                dropZone.scrollTop += newScrollTop;

                console.log("Canvas scaled:", scale);
            }
        });

        function drawSelection(x, y, width, height) {
            ctxSelection.clearRect(0, 0, selectionCanvas.width, selectionCanvas.height);
            ctxSelection.fillStyle = 'rgba(255, 0, 0, 0.5)';
            ctxSelection.fillRect(x, y, width, height);
            console.log("Drawing selection:", x, y, width, height);
        }

        function enableFormFields() {
            document.getElementById('name').disabled = false;
            document.getElementById('add_to_tileset').disabled = false;
        }

        function disableFormFields() {
            document.getElementById('name').disabled = true;
            document.getElementById('add_to_tileset').disabled = true;
        }

        function drawNightFilter(ctx, width, height) {
            const nightFilter = {
                color: { r: 102, g: 39, b: 255 },
                opacity: 0.5 // Adjust the opacity as needed
            };
            ctx.fillStyle = `rgba(${nightFilter.color.r}, ${nightFilter.color.g}, ${nightFilter.color.b}, ${nightFilter.opacity})`;
            ctx.globalCompositeOperation = 'multiply';
            ctx.fillRect(0, 0, width, height);
        }

        function updatePreviewCanvas() {
            ctxPreview.clearRect(0, 0, previewCanvas.width, previewCanvas.height);
            ctxNightFilter.clearRect(0, 0, nightFilterCanvas.width, nightFilterCanvas.height);

            if (tileset_window.selectedTiles.length > 0) {
                var tileSize = 16; // Assuming tile size is 16x16

                // Calculate the number of columns and rows based on the selected tiles
                const minX = Math.min(...tileset_window.selectedTiles.map(tile => tile.x));
                const minY = Math.min(...tileset_window.selectedTiles.map(tile => tile.y));
                const maxX = Math.max(...tileset_window.selectedTiles.map(tile => tile.x));
                const maxY = Math.max(...tileset_window.selectedTiles.map(tile => tile.y));
                
                const numCols = (maxX - minX) / tileSize + 1;
                const numRows = (maxY - minY) / tileSize + 1;

                // Set the width and height of the preview canvas based on the number of columns and rows
                const canvasWidth = numCols * tileSize * 2; // Double the width
                const canvasHeight = numRows * tileSize * 2; // Double the height

                // Resize the preview canvas
                previewCanvas.width = canvasWidth;
                previewCanvas.height = canvasHeight;

                // Resize the night filter canvas to match the preview canvas
                nightFilterCanvas.width = canvasWidth;
                nightFilterCanvas.height = canvasHeight;

                // Turn off image smoothing
                ctxPreview.imageSmoothingEnabled = false;

                // Log the canvas width and height
                console.log(`Canvas Width: ${canvasWidth}, Canvas Height: ${canvasHeight}`);

                tileset_window.selectedTiles.forEach((tile, index) => {
                    // Calculate the position on the preview canvas
                    const col = (tile.x - minX) / tileSize;
                    const row = (tile.y - minY) / tileSize;
                    const x = col * tileSize * 2; // Double the x position
                    const y = row * tileSize * 2; // Double the y position

                    console.log("Drawing tile at (x, y):", x, y);

                    // Draw each tile on the preview canvas at the calculated position (x, y)
                    ctxPreview.drawImage(
                        imageCanvas,
                        tile.x, tile.y, tile.width, tile.height,
                        x, y, tileSize * 2, tileSize * 2 // Double the size of each tile
                    );
                });

                console.log("Preview updated.");

                // Draw the night filter if enabled
                if (tileset_window.nightFilterEnabled) {
                    nightFilterCanvas.style.display = 'block';
                    drawNightFilter(ctxNightFilter, nightFilterCanvas.width, nightFilterCanvas.height);
                } else {
                    nightFilterCanvas.style.display = 'none';
                }
            }
        }

        // Event listener for night filter toggle switch
        var nightFilterToggle = document.getElementById('night_filter_toggle');
        nightFilterToggle.addEventListener('change', function() {
            tileset_window.nightFilterEnabled = this.checked;
            updatePreviewCanvas();
        });

        // Initialize the preview canvas
        updatePreviewCanvas();
    },
    displayTilesetItems: function () {
    const itemsGrid = document.getElementById('itemsGrid');
    itemsGrid.innerHTML = '';

    for (const itemId in game.objectData) {
        if (game.objectData.hasOwnProperty(itemId)) {
            const itemArray = game.objectData[itemId];

            itemArray.forEach((item) => {
                const itemDiv = document.createElement('div');
                itemDiv.className = 'item-preview bg-[#333] rounded shadow-md hover:bg-[#222] cursor-pointer transition duration-200 p-4 flex flex-col items-center justify-center';

                const itemTitle = document.createElement('h3');
                itemTitle.className = 'text-center font-bold text-white mb-2';
                itemTitle.innerText = item.n;

                const itemCanvasContainer = tileset_window.drawItemOnCanvas(item);
                itemDiv.appendChild(itemTitle);
                itemDiv.appendChild(itemCanvasContainer);

                // Assuming each item has a unique identifier property, e.g., item.id or item.identifier
                const uniqueId = item.id || itemId; // Fallback to itemId if item.id is not defined
                const pluginId = `tileset_item_editor_window_${uniqueId}`; // Create a unique plugin ID

                // Attach a click event to open the item in the plugin using plugin.load
                itemDiv.addEventListener('click', function() {
                    plugin.load({ id: 'tileset_item_editor_window', url: `renadmin/tileset/items.php?id=${uniqueId}`, name: 'Item Editor', drag: true, reload: true });
                    console.log(uniqueId); // Log the ID to ensure it's correct
                });

                itemsGrid.appendChild(itemDiv);
            });
        }
    }
},

drawItemOnCanvas: function (item) {
    // Assuming assets.load(item.t) returns a tileset image
    const tilesetImage = assets.use(item.t);

    // Determine the number of tiles and the canvas size
    const tileSize = 16; // Size of each tile (e.g., 16x16)

    // Calculate the number of columns and rows based on the provided data
    const maxCol = item.a; // Single integer for column
    const maxRow = item.b; // Single integer for row

    // Create a temporary canvas to draw the item
    const itemCanvas = document.createElement('canvas');
    const ctx = itemCanvas.getContext('2d');

    // Set canvas size based on the number of tiles and the tile size
    itemCanvas.width = (maxCol + 1) * tileSize;
    itemCanvas.height = (maxRow + 1) * tileSize;

    // Determine which frames to render
    let framesToRender = [];

    if (item.d && Array.isArray(item.i[0])) {
        // Animated item with duration, only render the first sub-array
        framesToRender = item.i[0];  // Use only the first set [8, 9] in this case
    } else if (Array.isArray(item.i[0])) {
        // Animated item without duration, render all frames
        framesToRender = item.i.flat(); // Flatten to render all frames
    } else {
        // Non-animated item, parse ranges and render all
        framesToRender = item.i.map(frame => {
            if (typeof frame === 'string' && frame.includes('-')) {
                return render.parseRange(frame);
            }
            return [frame];
        }).flat();
    }

    // Draw each tile
    framesToRender.forEach((frame, index) => {
        const srcX = (frame % 150) * tileSize;
        const srcY = Math.floor(frame / 150) * tileSize;

        const destX = (index % (maxCol + 1)) * tileSize;  // Calculate the x position
        const destY = Math.floor(index / (maxCol + 1)) * tileSize;  // Calculate the y position

        // Draw the tile from the tileset image onto the item canvas
        ctx.drawImage(
            tilesetImage,  // Assuming this is the tileset image loaded earlier
            srcX, srcY, tileSize, tileSize, // Source dimensions
            destX, destY, tileSize, tileSize // Destination dimensions
        );
    });

    // Adjust canvas style for responsiveness and grid fitting
    itemCanvas.className = 'mx-auto block';

    // Wrap the canvas in a responsive container if needed
    const canvasContainer = document.createElement('div');
    canvasContainer.className = 'flex justify-center items-center w-full h-full max-w-[150px] max-h-[150px] aspect-w-1 aspect-h-1 overflow-hidden'; // Adjust the size to fit your grid

    // Add a class to make the canvas fill the container
    itemCanvas.className += ' w-full h-full object-contain';

    // Append canvas to container
    canvasContainer.appendChild(itemCanvas);

    return canvasContainer; // Return the container element with the item drawn
},

    unmount: function() {
        ui.destroyTabs('tileset_window_tabs');
        ui.destroyTabs('right_tabs');
    }
}

function addToTileset() {
    var name = document.getElementById('name').value.trim();
    if (!name) {
        alert('Please enter a name for the tileset item.');
        return;
    }

    var newObject = {
        "n": name,
        "t": "gen1",
        "i": [],
        "a": [],
        "b": [],
        "w": 1,
        "s": 1,
        "z": 1
    };

    var tileSize = 16;
    var minX = Math.min(...tileset_window.selectedTiles.map(tile => tile.x));
    var minY = Math.min(...tileset_window.selectedTiles.map(tile => tile.y));

    tileset_window.selectedTiles.forEach((tile, index) => {
        var col = (tile.x - minX) / tileSize;
        var row = (tile.y - minY) / tileSize;
        newObject.i.push(index + game.objectData.item_count);
        newObject.a.push(col);
        newObject.b.push(row);
    });

    // Create a new canvas to hold the original size image
    var originalCanvas = document.createElement('canvas');
    var ctxOriginal = originalCanvas.getContext('2d');
    var numCols = (Math.max(...tileset_window.selectedTiles.map(tile => tile.x)) - minX) / tileSize + 1;
    var numRows = (Math.max(...tileset_window.selectedTiles.map(tile => tile.y)) - minY) / tileSize + 1;
    originalCanvas.width = numCols * tileSize;
    originalCanvas.height = numRows * tileSize;

    // Draw the original size tiles on the new canvas
    tileset_window.selectedTiles.forEach((tile, index) => {
        var col = (tile.x - minX) / tileSize;
        var row = (tile.y - minY) / tileSize;
        ctxOriginal.drawImage(
            tileset_window.imageCanvas,
            tile.x, tile.y, tile.width, tile.height,
            col * tileSize, row * tileSize, tileSize, tileSize
        );
    });

    // Convert original canvas image to base64 string
    var imageData = originalCanvas.toDataURL('image/png');

    // Log the base64 image data to the console
    console.log('Base64 Image Data:', imageData);

    // Prepare data to send to server
    var data = {
        newObject: newObject,
        selectedTiles: tileset_window.selectedTiles,
        imageData: imageData
    };

    // Send data to server using AJAX
    ui.ajax({
        outputType: 'json',
        method: 'POST',
        url: 'plugins/renadmin/tileset/ajax/save_tileset.php',
        data: JSON.stringify(data),
        processData: false, // Not needed for JSON
        contentType: 'application/json', // Important for JSON
        success: function(data) {
            console.log('PHP Response:', data);
            if (data.success) {
                alert('Tileset item added successfully.');

                const assetsToReload = [
                    { name: 'gen1', path: 'img/tiles/gen1.png' },
                    { name: 'objectData', path: 'json/objectData.json' }
                ];

                assets.reloadAssets(assetsToReload, () => {
                    console.log('Game data reloaded');
                    
                    game.objectData = assets.use('objectData');
                });

                // Update the game object data and item count
                game.objectData[name] = game.objectData[name] || [];
                game.objectData[name].push(newObject);
            } else {
                console.error('Error:', data.message);
                alert('Failed to add tileset item: ' + data.message);
            }
        },
        error: function(err) {
            console.error('Error occurred while adding tileset item:', err);
            alert('Error occurred while adding tileset item. See console for details.');
        }
    });
}

tileset_window.start();
</script>



    <div class='resize-handle'></div>
  </div>
<?php
}
?>
