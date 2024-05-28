<div data-window='editMode_manager_window' class='window window_bg' style='width: 900px; height: 550px; background: #242d39;'>

  <div data-part='handle' class='window_title' style='background-image: radial-gradient(#424b59 1px, transparent 0) !important;'>
    <div class='float-right'>
      <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
    </div>
    <div data-part='title' class='title_bg window_border' style='background: #242d39; color: #ede8d6;'>Tileset Manager</div>
  </div>
  <div class='clearfix'></div>
  
  <!-- Top Bar Menu (Replaced with Tab System) -->
  <div class="tab">
    <button class="tablinks" onclick="editMode_manager_window.openTab(event, 'Upload')">Upload</button>
    <button class="tablinks" onclick="editMode_manager_window.openTab(event, 'EditItems')">Items</button>
  </div>

  <!-- Tab content -->
  <div id="Upload" class="tabcontent">
    <div class='relative container text-white window_body p-2' style="display: flex; height: 100%;">
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
            <button id="addToTilesetBtn" class="bg-green-500 text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md">Add to Tileset</button>
          </div>
        </div>
      </div>
    </div>
  </div>

  <div id="EditItems" class="tabcontent">
    <div class='relative container text-white window_body p-2' style="display: flex; flex-direction: column; height: 100%;">
      <div style="margin-bottom: 10px;">
        <label for="editTilesetSelect">Tileset:</label><br />
        <select id="editTilesetSelect" class="bg-gray-800 text-white py-2 px-4 mr-1 rounded" onchange="editMode_manager_window.loadTilesetObjects()">
          <?php
          foreach ($tilesets as $tileset) {
              if (is_file("$tilesetDir/$tileset")) {
                  echo "<option value='$tileset'>$tileset</option>";
              }
          }
          ?>
        </select>
      </div>
      <div style="display: flex; height: 100%;">
        <div id="objectGrid" class="grid grid-cols-8 overflow-y-auto w-2/3"></div>
        <div style="padding-left: 10px; overflow-y: scroll;">
          <div id="editPropertiesPanel" style="display: none; padding: 10px;">
            <div>
              <canvas id="editPreviewCanvas" style="border: 1px solid #ffffff; width: 100%;"></canvas>
            </div>
            <div style="margin-top: 10px;">
              <label for="editTileName">Name:</label>
              <input type="text" id="editTileName" class="form-control p-2" style="color: #000; width: 100%;">
            </div>
            <div style="margin-top: 10px;">
              <label for="editTileDescription">Description:</label>
              <textarea id="editTileDescription" class="form-control p-2" style="width: 100%; color: #000;"></textarea>
            </div>
            <div style="margin-top: 10px;">
              <label for="editTilesetSelect">Tileset:</label><br />
              <select id="editTilesetSelect" class="bg-gray-800 text-white py-2 px-4 mr-1 rounded">
                <?php
                foreach ($tilesets as $tileset) {
                    if (is_file("$tilesetDir/$tileset")) {
                        echo "<option value='$tileset'>$tileset</option>";
                    }
                }
                ?>
              </select>
            </div>
            <div style="margin-top: 10px;">
              <button id="updateTilesetBtn" class="bg-green-500 text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md">Update Tileset</button>
            </div>
          </div>
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

    /* Tab styling */
    .tab {
      overflow: hidden;
      border-bottom: 1px solid #2d3748;
      background-color: #242d39;
    }

    .tab button {
      background-color: inherit;
      float: left;
      border: none;
      outline: none;
      cursor: pointer;
      padding: 10px 16px;
      transition: 0.3s;
      font-size: 16px;
      color: #cbd5e0;
    }

    .tab button:hover {
      background-color: #2d3748;
    }

    .tab button.active {
      background-color: #4a5568;
      color: #fff;
    }

    .tabcontent {
      display: none;
      padding: 6px 12px;
      border-top: none;
      height: calc(100% - 78px);
    }

    .object-grid-item {
      border: 1px solid #ccc;
      margin: 5px;
      height: 70px;
      position: relative;
      cursor: pointer;
      display: flex;
      justify-content: center;
      align-items: center;
    }

    .object-grid-item canvas {
      width: 100%;
      height: 100%;
    }

    .object-grid-item span {
      position: absolute;
      bottom: 0;
      left: 0;
      background: rgba(0, 0, 0, 0.5);
      color: white;
      width: 100%;
      text-align: center;
      font-size: 10px;
    }

    .object-group {
      border: 1px solid #444;
      margin: 10px;
      padding: 10px;
      width: 100%;
    }

    .object-group-title {
      font-weight: bold;
      margin-bottom: 5px;
    }
  </style>

  <script>
var editMode_manager_window = {
  selectedTiles: [],
  highestIValue: 0, // Initialize with 0 for the first tile

  start: function() {
    let isDragging = false;
    let isShiftPressed = false;
    let startX, startY, endX, endY;
    let mouseDownTime;

    this.openTab = function(evt, tabName) {
      var i, tabcontent, tablinks;
      tabcontent = document.getElementsByClassName("tabcontent");
      for (i = 0; i < tabcontent.length; i++) {
        tabcontent[i].style.display = "none";
      }
      tablinks = document.getElementsByClassName("tablinks");
      for (i = 0; i < tablinks.length; i++) {
        tablinks[i].className = tablinks[i].className.replace(" active", "");
      }
      document.getElementById(tabName).style.display = "block";
      evt.currentTarget.className += " active";
    }

    // Set default tab
    document.querySelectorAll(".tab button")[1].click();

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
      editMode_manager_window.selectedTiles = [];

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
          editMode_manager_window.selectedTiles.push({ x: x + i, y: y + j, width: tileSize, height: tileSize });
        }
      }

      drawSelection(x, y, width, height);
      showPropertiesPanel({ x, y, width, height });

      console.log("Mouse up at:", endX, endY);
      console.log("Selected tiles:", editMode_manager_window.selectedTiles);

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
      if (editMode_manager_window.selectedTiles.length === 0) {
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
        const response = await fetch(network.noCache('assets/json/objectData.json'));
        const objectData = await response.json();

        // Update the highestIValue if the objectData.json is not empty
        editMode_manager_window.highestIValue = Object.values(objectData).flat().reduce((max, obj) => {
          return Math.max(max, ...obj.i);
        }, 0);

        // Load the tileset image
        const tilesetImageResponse = await fetch(network.noCache(`/assets/img/tiles/${tilesetName}.png`));
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
          const totalTiles = editMode_manager_window.highestIValue + 1 + editMode_manager_window.selectedTiles.length; // total tiles considering the highest index and new tiles
          const newRows = Math.ceil(totalTiles / maxTilesPerRow);
          const newHeight = newRows * tileSize;

          canvas.width = maxTilesPerRow * tileSize;
          canvas.height = newHeight;

          // Draw the existing tileset image on the canvas
          ctx.drawImage(img, 0, 0);

          // Calculate the starting index for new tiles
          const startIndex = editMode_manager_window.highestIValue + 1;

          // Sort selectedTiles by y (then x for stable sorting)
          editMode_manager_window.selectedTiles.sort((tile, tile2) => (tile.y - tile2.y) || (tile.x - tile2.x));

          // Calculate minX and minY to normalize a and b values
          const minX = Math.min(...editMode_manager_window.selectedTiles.map(tile => tile.x));
          const minY = Math.min(...editMode_manager_window.selectedTiles.map(tile => tile.y));

          // Update selectedTiles with correct tile coordinates
          const newTiles = {
            t: tilesetName,
            i: [],
            a: [],
            b: [],
            w: [],
            s: 1,
            z: 1
          };

          editMode_manager_window.selectedTiles.forEach((tile, index) => {
            const globalIndex = startIndex + index;
            newTiles.i.push(globalIndex);
            newTiles.a.push((tile.x - minX) / tileSize);
            newTiles.b.push((tile.y - minY) / tileSize);
            newTiles.w.push([4, 0, 0, 0]); // Replace with actual values if needed
          });

          // Log the selected tiles
          console.log('Selected tiles:', editMode_manager_window.selectedTiles);

          // Draw the new tiles on the canvas in a horizontal sequence
          editMode_manager_window.selectedTiles.forEach((tile, index) => {
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
              if (!objectData[objectName]) {
                objectData[objectName] = [];
              }
              objectData[objectName].push(newTiles);

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

              // Clear the tile array and index
              editMode_manager_window.selectedTiles = [];
              editMode_manager_window.highestIValue += newTiles.i.length;

              // Remove red opaque background and deselect any values
              const selectionCtx = document.getElementById('selectionCanvas').getContext('2d');
              selectionCtx.clearRect(0, 0, selectionCanvas.width, selectionCanvas.height);

              // Clear input fields
              document.getElementById('tileName').value = '';
              document.getElementById('tileDescription').value = '';
              propertiesPanel.style.display = 'none';

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

    editMode_manager_window.loadTilesetObjects = async function() {
      const tilesetName = document.getElementById('editTilesetSelect').value.replace('.png', '');

      try {
        const response = await fetch(network.noCache('assets/json/objectData.json'));
        const objectData = await response.json();
        const objectGrid = document.getElementById('objectGrid');
        objectGrid.innerHTML = '';

        const groupedObjects = {};

        for (const key in objectData) {
          const objects = objectData[key];

          objects.forEach(obj => {
            if (obj.t === tilesetName) {
              if (!groupedObjects[key]) {
                groupedObjects[key] = [];
              }
              groupedObjects[key].push(obj);
            }
          });
        }

        for (const key in groupedObjects) {
          const group = groupedObjects[key];

          group.forEach(obj => {
            const minX = Math.min(...obj.a);
            const minY = Math.min(...obj.b);
            const maxX = Math.max(...obj.a) + 1;
            const maxY = Math.max(...obj.b) + 1;

            const canvas = document.createElement('canvas');
            const ctx = canvas.getContext('2d');

            canvas.width = (maxX - minX) * 16;
            canvas.height = (maxY - minY) * 16;

            const img = new Image();
            img.src = network.noCache(`/assets/img/tiles/${tilesetName}.png`);
            img.onload = function() {
              obj.a.forEach((a, index) => {
                const b = obj.b[index];
                const sourceX = (obj.i[index] % (img.width / 16)) * 16;
                const sourceY = Math.floor(obj.i[index] / (img.width / 16)) * 16;

                ctx.drawImage(
                  img,
                  sourceX, sourceY, 16, 16,
                  (a - minX) * 16, (b - minY) * 16, 16, 16
                );
              });
            };

            img.onerror = function() {
              console.error('Failed to load image:', img.src);
            };

            const gridItem = document.createElement('div');
            gridItem.className = 'object-grid-item';
            gridItem.appendChild(canvas);

            gridItem.addEventListener('click', () => {
              editMode_manager_window.showPropertiesForObject(key, obj, tilesetName);
            });

            const label = document.createElement('span');
            label.textContent = key;
            gridItem.appendChild(label);

            objectGrid.appendChild(gridItem);
          });
        }
      } catch (error) {
        console.error('Error loading object data:', error);
      }
    };

    editMode_manager_window.showPropertiesForObject = function(name, objectGroup, tilesetName) {
      const propertiesPanel = document.getElementById('editPropertiesPanel');
      propertiesPanel.style.display = 'block';

      document.getElementById('editTileName').value = name;
      document.getElementById('editTileDescription').value = '';

      const previewCanvas = document.getElementById('editPreviewCanvas');
      const previewCtx = previewCanvas.getContext('2d');

      const minX = Math.min(...objectGroup.a);
      const minY = Math.min(...objectGroup.b);
      const maxX = Math.max(...objectGroup.a) + 1;
      const maxY = Math.max(...objectGroup.b) + 1;

      previewCanvas.width = (maxX - minX) * 16;
      previewCanvas.height = (maxY - minY) * 16;

      previewCtx.clearRect(0, 0, previewCanvas.width, previewCanvas.height);

      const img = new Image();
      img.src = network.noCache(`/assets/img/tiles/${tilesetName}.png`);
      img.onload = function() {
        objectGroup.a.forEach((a, index) => {
          const b = objectGroup.b[index];
          const sourceX = (objectGroup.i[index] % (img.width / 16)) * 16;
          const sourceY = Math.floor(objectGroup.i[index] / (img.width / 16)) * 16;

          previewCtx.drawImage(
            img,
            sourceX, sourceY, 16, 16,
            (a - minX) * 16, (b - minY) * 16, 16, 16
          );
        });
      };

      document.getElementById('gridView').style.display = 'none';
      document.getElementById('propertiesView').style.display = 'flex';
    };

    editMode_manager_window.showGridView = function() {
      document.getElementById('gridView').style.display = 'flex';
      document.getElementById('propertiesView').style.display = 'none';
    };

    this.loadTilesetObjects();
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

