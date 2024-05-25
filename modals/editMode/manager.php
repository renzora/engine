<div data-window='editMode_manager_window' class='window window_bg' style='width: 900px; height: 550px; background: #242d39;'>

  <div data-part='handle' class='window_title' style='background-image: radial-gradient(#424b59 1px, transparent 0) !important;'>
    <div class='float-right'>
      <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
    </div>
    <div data-part='title' class='title_bg window_border' style='background: #242d39; color: #ede8d6;'>Tileset Manager</div>
  </div>
  <div class='clearfix'></div>
  
  <!-- Top Bar Menu -->
  <div class="menu mt-1 flex items-center justify-between">
    <div class="flex">
      <div class="dropdown">
        <button class="dropbtn">File</button>
        <div class="dropdown-content">
          <a href="#">New Tileset</a>
          <a href="#">Add Image</a>
        </div>
      </div>
      <div class="dropdown">
        <button class="dropbtn">Edit</button>
        <div class="dropdown-content">
          <a href="#">Undo</a>
          <a href="#">Redo</a>
          <a href="#">Cut</a>
          <a href="#">Copy</a>
          <a href="#">Paste</a>
        </div>
      </div>
      <div class="dropdown">
        <button class="dropbtn">View</button>
        <div class="dropdown-content">
          <a href="#">Zoom In</a>
          <a href="#">Zoom Out</a>
          <a href="#">Grid</a>
        </div>
      </div>
    </div>
    <!-- Floating select box -->
    <div class="ml-auto">
      <select class="bg-gray-800 text-white py-2 px-4 mr-1 rounded">
        <option value="option1">Interior</option>
        <option value="option2">Exterior</option>
      </select>
    </div>
  </div>
  
  <div class='relative container text-white window_body p-2' style="display: flex; height: calc(100% - 70px);">
    <div style="width: 70%; overflow-y: auto; height: 100%;">
      <form id="tilesetForm">
        <input type="file" id="tilesetInput" class="form-control" accept="image/*">
        <div id="tilesetDisplay" style="margin-top: 20px; position: relative; width: 100%;">
          <canvas id="gridCanvas"></canvas>
          <canvas id="tilesetCanvas"></canvas>
          <canvas id="selectionCanvas"></canvas>
        </div>
      </form>
    </div>
    <div style="width: 30%; padding-left: 10px; overflow-y: scroll;">
      <div id="propertiesPanel" style="display: none; padding: 10px;">
        <div>
          <canvas id="previewCanvas" style="border: 1px solid #ffffff; width: 100%;"></canvas>
        </div>
        <div style="margin-top: 10px;">
          <label for="tileName">Name:</label>
          <input type="text" id="tileName" class="form-control p-2" style="color: #000; width: 100%;">
        </div>
        <div style="margin-top: 10px;">
          <label for="tileDescription">Description:</label>
          <textarea id="tileDescription" class="form-control p-2" style="width: 100%; color: #000;"></textarea>
        </div>
        <div style="margin-top: 10px;">
          <label for="tilesetSelect">Tileset:</label><br />
          <select id="tilesetSelect" class="bg-gray-800 text-white py-2 px-4 mr-1 rounded">
            <?php
            $tilesetDir = $_SERVER['DOCUMENT_ROOT'] . '/assets/img/tiles';
            $tilesets = array_diff(scandir($tilesetDir), array('..', '.'));

            foreach ($tilesets as $tileset) {
                if (is_file("$tilesetDir/$tileset")) {
                    echo "<option value='$tileset'>$tileset</option>";
                }
            }
            ?>
        </select>
        </div>
        <div style="margin-top: 10px;">
          <button id="addToTilesetBtn" class="green_button text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md">Add to Tileset</button>
        </div>
      </div>
    </div>
  </div>

  <style>
    .menu {
      display: flex;
      background-color: #1d232b;
    }
    .dropdown {
      position: relative;
      display: inline-block;
      margin-right: 10px;
    }
    .dropbtn {
      color: white;
      padding: 10px;
      font-size: 16px;
      border: none;
      cursor: pointer;
    }
    .dropdown-content {
      display: none;
      position: absolute;
      background-color: #f9f9f9;
      border: 1px #000 solid;
      min-width: 160px;
      box-shadow: 0px 8px 16px 0px rgba(0,0,0,0.2);
      z-index: 1;
    }
    .dropdown-content a {
      color: black;
      padding: 12px 16px;
      text-decoration: none;
      display: block;
    }
    .dropdown-content a:hover {
      background-color: #f1f1f1;
    }
    .dropdown:hover .dropdown-content {
      display: block;
    }
    .dropdown:hover .dropbtn {
      background-color: rgba(0,0,0,0.5);
    }

    #tilesetDisplay {
      position: relative;
      overflow: hidden;
    }

    #gridCanvas, #tilesetCanvas, #selectionCanvas {
      display: block;
      width: 100%;
      height: auto;
    }

    #tilesetCanvas, #selectionCanvas {
      position: absolute;
      top: 0;
      left: 0;
    }

    #selectionCanvas {
      pointer-events: none;
    }

    #previewCanvas {
      display: block;
      width: 100%;
      height: auto;
      margin-bottom: 10px;
    }

    .window_body {
      overflow: hidden;
    }
  </style>

  <script>
    var editMode_manager_window = {
      start: function() {
        let isDragging = false;
        let isShiftPressed = false;
        let startX, startY, endX, endY;
        let mouseDownTime;
        let selectedTiles = [];

        // Track shift key state
        document.addEventListener('keydown', function(event) {
          if (event.key === 'Shift') {
            isShiftPressed = true;
          }
        });

        document.addEventListener('keyup', function(event) {
          if (event.key === 'Shift') {
            isShiftPressed = false;
          }
        });

        // Image upload and canvas drawing
        document.getElementById('tilesetInput').addEventListener('change', function(event) {
          const file = event.target.files[0];
          if (file) {
            console.log("Image selected:", file.name);
            const reader = new FileReader();
            reader.onload = function(e) {
              const img = new Image();
              img.onload = function() {
                const gridCanvas = document.getElementById('gridCanvas');
                const gridCtx = gridCanvas.getContext('2d');
                const tilesetCanvas = document.getElementById('tilesetCanvas');
                const tilesetCtx = tilesetCanvas.getContext('2d');
                const selectionCanvas = document.getElementById('selectionCanvas');
                const selectionCtx = selectionCanvas.getContext('2d');
                
                gridCanvas.width = img.width;
                gridCanvas.height = img.height;
                tilesetCanvas.width = img.width;
                tilesetCanvas.height = img.height;
                selectionCanvas.width = img.width;
                selectionCanvas.height = img.height;
                
                tilesetCtx.drawImage(img, 0, 0, tilesetCanvas.width, tilesetCanvas.height);
                drawGrid(gridCtx, gridCanvas.width, gridCanvas.height);
                tilesetCtx.imageSmoothingEnabled = false;
                console.log("Image loaded and drawn on canvas:", img.src);
              }
              img.src = e.target.result;
            }
            reader.readAsDataURL(file);
          }
        });

        document.getElementById('tilesetCanvas').addEventListener('mousedown', function(event) {
          const canvas = event.target;
          const rect = canvas.getBoundingClientRect();
          const scaleX = canvas.width / rect.width;
          const scaleY = canvas.height / rect.height;
          startX = (event.clientX - rect.left) * scaleX;
          startY = (event.clientY - rect.top) * scaleY;

          // Clear the selected tiles array for every new selection
          selectedTiles = [];

          isDragging = true;
          mouseDownTime = Date.now();

          console.log("Mouse down at:", startX, startY);
        });

        document.getElementById('tilesetCanvas').addEventListener('mousemove', function(event) {
          if (!isDragging) return;

          const canvas = event.target;
          const rect = canvas.getBoundingClientRect();
          const scaleX = canvas.width / rect.width;
          const scaleY = canvas.height / rect.height;
          const currentX = (event.clientX - rect.left) * scaleX;
          const currentY = (event.clientY - rect.top) * scaleY;

          const tileSize = 16; // Fixed tile size
          const startXTile = Math.floor(startX / tileSize) * tileSize;
          const startYTile = Math.floor(startY / tileSize) * tileSize;
          const currentXTile = Math.floor(currentX / tileSize) * tileSize;
          const currentYTile = Math.floor(currentY / tileSize) * tileSize;

          const x = Math.min(startXTile, currentXTile);
          const y = Math.min(startYTile, currentYTile);
          const width = Math.abs(currentXTile - startXTile) + tileSize;
          const height = Math.abs(currentYTile - startYTile) + tileSize;

          drawSelection(x, y, width, height);

          console.log("Mouse move at:", currentX, currentY);
          console.log("Selection area:", x, y, width, height);
        });

        document.getElementById('tilesetCanvas').addEventListener('mouseup', function(event) {
          if (!isDragging) return;

          const canvas = event.target;
          const rect = canvas.getBoundingClientRect();
          const scaleX = canvas.width / rect.width;
          const scaleY = canvas.height / rect.height;
          endX = (event.clientX - rect.left) * scaleX;
          endY = (event.clientY - rect.top) * scaleY;

          const tileSize = 16; // Fixed tile size
          const startXTile = Math.floor(startX / tileSize) * tileSize;
          const startYTile = Math.floor(startY / tileSize) * tileSize;
          const endXTile = Math.floor(endX / tileSize) * tileSize;
          const endYTile = Math.floor(endY / tileSize) * tileSize;

          const x = Math.min(startXTile, endXTile);
          const y = Math.min(startYTile, endYTile);
          const width = Math.abs(endXTile - startXTile) + tileSize;
          const height = Math.abs(endYTile - startYTile) + tileSize;

          for (let i = 0; i < width; i += tileSize) {
            for (let j = 0; j < height; j += tileSize) {
              selectedTiles.push({ x: x + i, y: y + j, width: tileSize, height: tileSize });
            }
          }

          drawSelection(x, y, width, height);
          showPropertiesPanel({ x, y, width, height });

          console.log("Mouse up at:", endX, endY);
          console.log("Selected tiles:", selectedTiles);

          isDragging = false;
        });

        function drawGrid(ctx, width, height) {
          const tileSize = 16; // Fixed tile size
          ctx.strokeStyle = 'rgba(0, 0, 0, 0.3)'; // Lighter color for better visibility
          for (var x = 0; x <= width; x += tileSize) {
            ctx.beginPath();
            ctx.moveTo(x, 0);
            ctx.lineTo(x, height);
            ctx.stroke();
          }
          for (var y = 0; y <= height; y += tileSize) {
            ctx.beginPath();
            ctx.moveTo(0, y);
            ctx.lineTo(width, y);
            ctx.stroke();
          }
          console.log("Grid drawn on canvas");
        }

        function drawSelection(x, y, width, height) {
          const selectionCanvas = document.getElementById('selectionCanvas');
          const ctx = selectionCanvas.getContext('2d');
          ctx.clearRect(0, 0, selectionCanvas.width, selectionCanvas.height); // Clear previous selection
          ctx.fillStyle = 'rgba(255, 0, 0, 0.5)'; // Red opaque background
          ctx.fillRect(x, y, width, height);
          console.log("Selection drawn on canvas:", x, y, width, height);
        }

        function showPropertiesPanel(tile) {
          const propertiesPanel = document.getElementById('propertiesPanel');
          propertiesPanel.style.display = 'block'; // Ensure the properties panel is displayed

          const tilesetCanvas = document.getElementById('tilesetCanvas');
          const tilesetCtx = tilesetCanvas.getContext('2d');
          const previewCanvas = document.getElementById('previewCanvas');
          const previewCtx = previewCanvas.getContext('2d');

          // Force a reflow to ensure the panel is properly sized
          propertiesPanel.offsetHeight;

          // Calculate the scale to fit the previewCanvas to the properties panel width
          const propertiesPanelWidth = propertiesPanel.offsetWidth;
          const scale = propertiesPanelWidth / tile.width;

          // Set the preview canvas size to match the properties panel width
          previewCanvas.width = propertiesPanelWidth;
          previewCanvas.height = tile.height * scale;

          // Clear the preview canvas
          previewCtx.clearRect(0, 0, previewCanvas.width, previewCanvas.height);

          previewCtx.imageSmoothingEnabled = false;

          // Draw the selected tile on the preview canvas
          previewCtx.drawImage(
            tilesetCanvas,
            tile.x, tile.y, tile.width, tile.height,
            0, 0, previewCanvas.width, previewCanvas.height
          );

          console.log("Properties panel updated with selected tile:", tile);
        }

        document.getElementById('addToTilesetBtn').addEventListener('click', async function() {
  if (selectedTiles.length === 0) {
    alert('No tiles selected.');
    return;
  }

  const tilesetName = document.querySelector('#tilesetSelect').value.replace('.png', '');
  const objectName = document.getElementById('tileName').value.trim();

  if (!objectName) {
    alert('Please enter a name for the tileset.');
    return;
  }

  try {
    const response = await fetch('assets/json/objectData.json');
    const objectData = await response.json();

    // Find the highest i value in the existing data
    let highestIValue = -1;  // Adjusted to -1 to handle the case of an empty objectData.json
    for (const key in objectData) {
      const tiles = objectData[key];
      for (const tile of tiles) {
        if (tile.i > highestIValue) {
          highestIValue = tile.i;
        }
      }
    }

    // Load the tileset image
    const tilesetImageResponse = await fetch(`/assets/img/tiles/${tilesetName}.png`);
    const tilesetImageBlob = await tilesetImageResponse.blob();
    const tilesetImageUrl = URL.createObjectURL(tilesetImageBlob);
    const img = new Image();

    img.onload = function() {
      const tileSize = 16;
      const maxTilesPerRow = 150;
      const canvas = document.createElement('canvas');
      const ctx = canvas.getContext('2d');

      // Calculate the new canvas dimensions
      const currentRows = img.height / tileSize;
      const currentTilesInRow = Math.floor(img.width / tileSize);
      const totalTiles = highestIValue + 1 + selectedTiles.length; // total tiles considering the highest index and new tiles
      const newRows = Math.ceil(totalTiles / maxTilesPerRow);
      const newHeight = newRows * tileSize;

      canvas.width = maxTilesPerRow * tileSize;
      canvas.height = newHeight;

      // Draw the existing tileset image on the canvas
      ctx.drawImage(img, 0, 0);

      // Calculate the starting index for new tiles
      const startIndex = highestIValue + 1;

      // Sort selectedTiles by y (then x for stable sorting)
      selectedTiles.sort((tile1, tile2) => (tile1.y - tile2.y) || (tile1.x - tile2.x));

      // Calculate minX and minY to normalize a and b values
      const minX = Math.min(...selectedTiles.map(tile => tile.x));
      const minY = Math.min(...selectedTiles.map(tile => tile.y));

      // Update selectedTiles with correct tile coordinates
      const newTiles = selectedTiles.map((tile, index) => {
        const globalIndex = startIndex + index;
        const a = (tile.x - minX) / tileSize;
        const b = (tile.y - minY) / tileSize;
        return {
          t: tilesetName,
          i: globalIndex,
          a: a,
          b: b,
          w: 1,
          s: 1,
          z: 1
        };
      });

      // Log the selected tiles
      console.log('Selected tiles:', selectedTiles);

      // Draw the new tiles on the canvas in a horizontal sequence
      selectedTiles.forEach((tile, index) => {
        const destX = (startIndex + index) % maxTilesPerRow * tileSize;
        const destY = Math.floor((startIndex + index) / maxTilesPerRow) * tileSize;

        // Log each tile's position
        console.log(`Drawing tile at index ${startIndex + index}: source(${tile.x}, ${tile.y}, ${tile.width}, ${tile.height}) dest(${destX}, ${destY}, ${tile.width}, ${tile.height})`);

        ctx.drawImage(
          tilesetCanvas,
          tile.x, tile.y, tile.width, tile.height,
          destX, destY, tile.width, tile.height
        );
      });

      // Convert the canvas to a blob and send it to the server
      canvas.toBlob(async function(blob) {
        const formData = new FormData();
        formData.append('tilesetName', `${tilesetName}.png`);
        formData.append('tilesetImage', blob);

        const uploadResponse = await fetch('modals/editMode/ajax/save_tileset_image.php', {
          method: 'POST',
          body: formData
        });

        const uploadResult = await uploadResponse.json();

        if (uploadResult.success) {
          // Add the new object to the existing data
          objectData[objectName] = newTiles;

          // Save the updated objectData.json file using AJAX
          ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'modals/editMode/ajax/add_objects.php',
            data: JSON.stringify(objectData),
            success: function(data) {
              console.log(data);
              if (data.success) {
                ui.notif("Item added successfully", 'bottom-center');
              } else {
                ui.notif('Error: ' + data.error, 'bottom-center');
              }
            }
          });

          console.log("Updated objectData.json with new tiles:", newTiles);
        } else {
          alert('Error: ' + uploadResult.error);
        }
      });
    };

    img.src = tilesetImageUrl;
  } catch (error) {
    ui.notif('Error updating objectData.json:', error, 'bottom-center');
  }
});


      },

      unmount: function() {
        document.getElementById('tilesetInput').removeEventListener('change');
        document.getElementById('tilesetCanvas').removeEventListener('mousedown');
        document.getElementById('tilesetCanvas').removeEventListener('mousemove');
        document.getElementById('tilesetCanvas').removeEventListener('mouseup');
        document.removeEventListener('keydown');
        document.removeEventListener('keyup');
      }
    }

    // Initialize the window
    editMode_manager_window.start();
  </script>

  <div class='resize-handle'></div>
</div>
