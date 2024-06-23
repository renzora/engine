<div data-window='renadmin_tileset_manager' class='window window_bg' style='width: 900px; height: 800px; background: #242d39;'>

  <div data-part='handle' class='window_title' style='background-image: radial-gradient(#424b59 1px, transparent 0) !important;'>
    <div class='float-right'>
      <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
    </div>
    <div data-part='title' class='title_bg window_border' style='background: #242d39; color: #ede8d6;'>Tileset Manager</div>
  </div>
  <div class='clearfix'></div>
  
  <div class="tab">
    <button class="tablinks" onclick="renadmin_tileset_manager.openTab(event, 'Upload')">Upload</button>
    <button class="tablinks" onclick="renadmin_tileset_manager.openTab(event, 'EditItems')">Items</button>
  </div>

  <div id="Upload" class="tabcontent">
    <div class='relative container text-white window_body p-2' style="display: flex; height: calc(100% - 40px);">
      <div style="width: 70%; overflow-y: auto; overflow-x: hidden; height: 100%;">
        <form id="tilesetForm">
          <input type="file" id="tilesetInput" class="form-control" accept="image/*">
          <div id="tilesetDisplay" style="margin-top: 20px; position: relative; width: 100%;">
            <canvas id="gridCanvas"></canvas>
            <canvas id="tilesetCanvas"></canvas>
            <canvas id="selectionCanvas"></canvas>
          </div>
        </form>
      </div>
      <div style="width: 30%; padding-left: 10px; overflow-y: auto; height: 100%;">
        <div id="propertiesPanel" style="display: none; padding: 10px;">
          <div>
            <canvas id="previewCanvas" style="border: 1px solid #ffffff; width: 100%;"></canvas>
          </div>
          <div style="margin-top: 10px;">
            <label for="tileName">Name:</label>
            <input type="text" id="tileName" class="form-control p-2" style="color: #000; width: 100%; overflow-x: hidden;">
          </div>
          <div style="margin-top: 10px;">
            <label for="walkable">Walkable:</label>
            <input type="checkbox" id="walkable" class="form-control p-2" style="color: #000;">
          </div>
          <div style="margin-top: 10px;">
            <label for="nConstraint">N Constraint:</label>
            <input type="number" id="nConstraint" class="form-control p-2" style="color: #000; width: 100%; overflow-x: hidden;" value="0">
          </div>
          <div style="margin-top: 10px;">
            <label for="eConstraint">E Constraint:</label>
            <input type="number" id="eConstraint" class="form-control p-2" style="color: #000; width: 100%; overflow-x: hidden;" value="0">
          </div>
          <div style="margin-top: 10px;">
            <label for="sConstraint">S Constraint:</label>
            <input type="number" id="sConstraint" class="form-control p-2" style="color: #000; width: 100%; overflow-x: hidden;" value="0">
          </div>
          <div style="margin-top: 10px;">
            <label for="wConstraint">W Constraint:</label>
            <input type="number" id="wConstraint" class="form-control p-2" style="color: #000; width: 100%; overflow-x: hidden;" value="0">
          </div>
          <div style="margin-top: 10px;">
            <label for="zIndex">Z Index:</label>
            <input type="number" id="zIndex" class="form-control p-2" style="color: #000; width: 100%; overflow-x: hidden;" value="1">
          </div>
          <div style="margin-top: 10px;">
            <label for="tilesetSelect">Tileset:</label><br />
            <select id="tilesetSelect" class="bg-gray-800 text-white py-2 px-4 mr-1 rounded" style="width: 100%; overflow-x: hidden;">
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
    <div class='relative container text-white window_body p-2' style="display: flex; flex-direction: column; height: calc(100% - 40px);">
      <div style="margin-bottom: 10px;">
        <label for="editTilesetSelect">Tileset:</label><br />
        <select id="editTilesetSelect" class="bg-gray-800 text-white py-2 px-4 mr-1 rounded" onchange="renadmin_tileset_manager.loadTilesetObjects()" style="width: 100%; overflow-x: hidden;">
          <?php
          foreach ($tilesets as $tileset) {
              if (is_file("$tilesetDir/$tileset")) {
                  echo "<option value='$tileset'>$tileset</option>";
              }
          }
          ?>
        </select>
      </div>
      <div id="gridView" style="display: flex; height: 100%; overflow-y: auto;">
        <div id="objectGrid" class="grid grid-cols-12 w-full"></div>
      </div>
      <div id="propertiesView" style="display: none; height: 100%; width: 100%; padding-left: 10px; overflow-y: auto;">
        <div id="editPropertiesPanel" style="display: flex; padding: 10px; width: 100%;">
          <div style="flex: 8; padding-right: 10px;">
            <div style="margin-bottom: 10px;">
              <button id="backButton" class="bg-blue-500 text-white font-bold py-2 px-4 rounded" onclick="renadmin_tileset_manager.showGridView()">Back</button>
            </div>
            <div style="margin-top: 10px;">
              <label for="editTileName">Name:</label>
              <input type="text" id="editTileName" class="form-control p-2" style="color: #000; width: 100%; overflow-x: hidden;">
            </div>
            <div style="margin-top: 10px;">
              <label for="editWalkable">Walkable:</label>
              <input type="checkbox" id="editWalkable" class="form-control p-2" style="color: #000;">
            </div>
            <div style="margin-top: 10px;">
              <label for="editNConstraint">N Constraint:</label>
              <input type="number" id="editNConstraint" class="form-control p-2" style="color: #000; width: 100%; overflow-x: hidden;" value="0">
            </div>
            <div style="margin-top: 10px;">
              <label for="editEConstraint">E Constraint:</label>
              <input type="number" id="editEConstraint" class="form-control p-2" style="color: #000; width: 100%; overflow-x: hidden;" value="0">
            </div>
            <div style="margin-top: 10px;">
              <label for="editSConstraint">S Constraint:</label>
              <input type="number" id="editSConstraint" class="form-control p-2" style="color: #000; width: 100%; overflow-x: hidden;" value="0">
            </div>
            <div style="margin-top: 10px;">
              <label for="editWConstraint">W Constraint:</label>
              <input type="number" id="editWConstraint" class="form-control p-2" style="color: #000; width: 100%; overflow-x: hidden;" value="0">
            </div>
            <div style="margin-top: 10px;">
              <label for="editZIndex">Z Index:</label>
              <input type="number" id="editZIndex" class="form-control p-2" style="color: #000; width: 100%; overflow-x: hidden;" value="1">
            </div>
            <div style="margin-top: 10px;">
              <button id="updateTilesetBtn" class="bg-green-500 text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md">Update Tileset</button>
            </div>
          </div>
          <div style="flex: 4;">
            <canvas id="editPreviewCanvas" style="border: 1px solid #ffffff; width: 100%;"></canvas>
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
      overflow-x: hidden;
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
      overflow-x: hidden;
      white-space: nowrap;
      text-overflow: ellipsis;
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
      overflow-x: hidden;
      white-space: nowrap;
      text-overflow: ellipsis;
    }
  </style>
</div>


<script>
var renadmin_tileset_manager = {
  selectedTiles: [],
  highestIValue: 0,
  currentEditingObject: null,
  currentEditingTileIndex: -1,
  currentTilesetName: '',

  start: function() {
    console.log('Tileset manager started');
    let isDragging = false;
    let isShiftPressed = false;
    let startX, startY, endX, endY;

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
      console.log("Tab opened:", tabName);
    }

    document.querySelectorAll(".tab button")[1].click();

    document.addEventListener('keydown', function(event) {
      if (event.key === 'Shift') {
        isShiftPressed = true;
      }
      console.log("Shift key pressed:", isShiftPressed);
    });

    document.addEventListener('keyup', function(event) {
      if (event.key === 'Shift') {
        isShiftPressed = false;
      }
      console.log("Shift key released:", isShiftPressed);
    });

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

      renadmin_tileset_manager.selectedTiles = [];

      isDragging = true;

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

      const tileSize = 16; 
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

      const tileSize = 16;
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
          renadmin_tileset_manager.selectedTiles.push({ x: x + i, y: y + j, width: tileSize, height: tileSize });
        }
      }

      drawSelection(x, y, width, height);
      showPropertiesPanel({ x, y, width, height });

      console.log("Mouse up at:", endX, endY);
      console.log("Selected tiles:", renadmin_tileset_manager.selectedTiles);

      isDragging = false;
    });

    function drawGrid(ctx, width, height) {
      const tileSize = 16;
      ctx.strokeStyle = 'rgba(0, 0, 0, 0.3)';
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
      ctx.clearRect(0, 0, selectionCanvas.width, selectionCanvas.height);
      ctx.fillStyle = 'rgba(255, 0, 0, 0.5)';
      ctx.fillRect(x, y, width, height);
      console.log("Selection drawn on canvas:", x, y, width, height);
    }

    function showPropertiesPanel(tile) {
      const propertiesPanel = document.getElementById('propertiesPanel');
      propertiesPanel.style.display = 'block';

      const tilesetCanvas = document.getElementById('tilesetCanvas');
      const tilesetCtx = tilesetCanvas.getContext('2d');
      const previewCanvas = document.getElementById('previewCanvas');
      const previewCtx = previewCanvas.getContext('2d');

      propertiesPanel.offsetHeight;

      const propertiesPanelWidth = propertiesPanel.offsetWidth;
      const scale = propertiesPanelWidth / tile.width;

      previewCanvas.width = propertiesPanelWidth;
      previewCanvas.height = tile.height * scale;

      previewCtx.clearRect(0, 0, previewCanvas.width, previewCanvas.height);

      previewCtx.imageSmoothingEnabled = false;

      previewCtx.drawImage(
        tilesetCanvas,
        tile.x, tile.y, tile.width, tile.height,
        0, 0, previewCanvas.width, previewCanvas.height
      );

      console.log("Properties panel updated with selected tile:", tile);
    }

    document.getElementById('addToTilesetBtn').addEventListener('click', async function() {
      if (renadmin_tileset_manager.selectedTiles.length === 0) {
        alert('No tiles selected.');
        return;
      }

      const tilesetName = document.querySelector('#tilesetSelect').value.replace('.png', '');
      const objectName = document.getElementById('tileName').value.trim();
      const isWalkable = document.getElementById('walkable').checked;
      const nConstraint = document.getElementById('nConstraint').value;
      const eConstraint = document.getElementById('eConstraint').value;
      const sConstraint = document.getElementById('sConstraint').value;
      const wConstraint = document.getElementById('wConstraint').value;
      const zIndex = document.getElementById('zIndex').value;

      if (!objectName) {
        alert('Please enter a name for the tileset.');
        return;
      }

      try {
        const response = await fetch('assets/json/objectData.json');
        const objectData = await response.json();

        renadmin_tileset_manager.highestIValue = Object.values(objectData).flat().reduce((max, obj) => {
          return Math.max(max, ...obj.i);
        }, 0);

        const tilesetImageResponse = await fetch(`/assets/img/tiles/${tilesetName}.png`);
        const tilesetImageBlob = await tilesetImageResponse.blob();
        const tilesetImageUrl = URL.createObjectURL(tilesetImageBlob);
        const img = new Image();

        img.onload = function() {
          const tileSize = 16;
          const maxTilesPerRow = 150;
          const canvas = document.createElement('canvas');
          const ctx = canvas.getContext('2d');

          const currentRows = img.height / tileSize;
          const currentTilesInRow = Math.floor(img.width / tileSize);
          const totalTiles = renadmin_tileset_manager.highestIValue + 1 + renadmin_tileset_manager.selectedTiles.length;
          const newRows = Math.ceil(totalTiles / maxTilesPerRow);
          const newHeight = newRows * tileSize;

          canvas.width = maxTilesPerRow * tileSize;
          canvas.height = newHeight;

          ctx.drawImage(img, 0, 0);

          const startIndex = renadmin_tileset_manager.highestIValue + 1;

          renadmin_tileset_manager.selectedTiles.sort((tile, tile2) => (tile.y - tile2.y) || (tile.x - tile2.x));

          const minX = Math.min(...renadmin_tileset_manager.selectedTiles.map(tile => tile.x));
          const minY = Math.min(...renadmin_tileset_manager.selectedTiles.map(tile => tile.y));

          const newTiles = {
            t: tilesetName,
            i: [],
            a: [],
            b: [],
            w: [],
            s: 1,
            z: parseInt(zIndex)
          };

          renadmin_tileset_manager.selectedTiles.forEach((tile, index) => {
            const globalIndex = startIndex + index;
            newTiles.i.push(globalIndex);
            newTiles.a.push((tile.x - minX) / tileSize);
            newTiles.b.push((tile.y - minY) / tileSize);
            if (isWalkable) {
              newTiles.w.push(1);
            } else {
              newTiles.w.push([parseInt(nConstraint), parseInt(eConstraint), parseInt(sConstraint), parseInt(wConstraint)]);
            }
          });

          renadmin_tileset_manager.selectedTiles.forEach((tile, index) => {
            const destX = (startIndex + index) % maxTilesPerRow * tileSize;
            const destY = Math.floor((startIndex + index) / maxTilesPerRow) * tileSize;

            ctx.drawImage(
              tilesetCanvas,
              tile.x, tile.y, tile.width, tile.height,
              destX, destY, tile.width, tile.height
            );
          });

          canvas.toBlob(async function(blob) {
            const formData = new FormData();
            formData.append('tilesetName', `${tilesetName}.png`);
            formData.append('tilesetImage', blob);

            const uploadResponse = await fetch('modals/renadmin/ajax/save_tileset_image.php', {
              method: 'POST',
              body: formData
            });

            const uploadResult = await uploadResponse.json();

            if (uploadResult.success) {
              if (!objectData[objectName]) {
                objectData[objectName] = [];
              }
              objectData[objectName].push(newTiles);

              const result = await fetch('modals/renadmin/ajax/add_objects.php', {
                method: 'POST',
                headers: {
                  'Content-Type': 'application/json',
                },
                body: JSON.stringify(objectData),
              });

              const resultData = await result.json();

              if (resultData.success) {
                ui.notif("Item added successfully", 'bottom-center');
              } else {
                ui.notif('Error: ' + resultData.error, 'bottom-center');
              }

              renadmin_tileset_manager.selectedTiles = [];
              renadmin_tileset_manager.highestIValue += newTiles.i.length;

              const selectionCtx = document.getElementById('selectionCanvas').getContext('2d');
              selectionCtx.clearRect(0, 0, selectionCanvas.width, selectionCanvas.height);

              document.getElementById('tileName').value = '';
              document.getElementById('walkable').checked = false;
              document.getElementById('nConstraint').value = 0;
              document.getElementById('eConstraint').value = 0;
              document.getElementById('sConstraint').value = 0;
              document.getElementById('wConstraint').value = 0;
              document.getElementById('zIndex').value = 1;
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

    document.getElementById('updateTilesetBtn').addEventListener('click', async function() {
  const newObjectName = document.getElementById('editTileName').value.trim();
  const isWalkable = document.getElementById('editWalkable').checked;
  const nConstraint = document.getElementById('nConstraint').value;
  const eConstraint = document.getElementById('eConstraint').value;
  const sConstraint = document.getElementById('sConstraint').value;
  const wConstraint = document.getElementById('wConstraint').value;
  const zIndex = document.getElementById('zIndex').value;
  const tilesetName = document.querySelector('#editTilesetSelect').value.replace('.png', '');

  console.log('Button clicked');
  console.log('New Object Name:', newObjectName);
  console.log('Is Walkable:', isWalkable);
  console.log('N Constraint:', nConstraint);
  console.log('E Constraint:', eConstraint);
  console.log('S Constraint:', sConstraint);
  console.log('W Constraint:', wConstraint);
  console.log('Z Index:', zIndex);
  console.log('Tileset Name:', tilesetName);

  if (!newObjectName) {
    alert('Please enter a name for the tileset.');
    return;
  }

  try {
    const response = await fetch('assets/json/objectData.json');
    const objectData = await response.json();

    console.log('Loaded object data:', objectData);

    if (!objectData[renadmin_tileset_manager.currentEditingObject]) {
      alert('Object not found.');
      console.log('Object not found:', renadmin_tileset_manager.currentEditingObject);
      return;
    }

    console.log('Editing object:', renadmin_tileset_manager.currentEditingObject);

    const updatedObject = {
      t: tilesetName,
      w: isWalkable ? 1 : [parseInt(nConstraint), parseInt(eConstraint), parseInt(sConstraint), parseInt(wConstraint)],
      z: parseInt(zIndex)
    };

    const existingObjectIndex = objectData[renadmin_tileset_manager.currentEditingObject].findIndex(obj => obj.t === tilesetName);
    if (existingObjectIndex !== -1) {
      console.log('Updating existing object at index:', existingObjectIndex);
      objectData[renadmin_tileset_manager.currentEditingObject][existingObjectIndex] = updatedObject;
    } else {
      console.log('Adding new object to the current editing object.');
      objectData[renadmin_tileset_manager.currentEditingObject].push(updatedObject);
    }

    if (renadmin_tileset_manager.currentEditingObject !== newObjectName) {
      console.log('Renaming object from', renadmin_tileset_manager.currentEditingObject, 'to', newObjectName);
      objectData[newObjectName] = objectData[renadmin_tileset_manager.currentEditingObject];
      delete objectData[renadmin_tileset_manager.currentEditingObject];
    }

    const updatedData = { [newObjectName]: objectData[newObjectName] };

    console.log('Updated data to be sent:', updatedData);

    const result = await fetch('modals/renadmin/ajax/edit_object.php', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(updatedData),
    });

    const resultText = await result.text();
    console.log('Raw server response:', resultText);

    try {
      const resultData = JSON.parse(resultText);
      console.log('Server response:', resultData);

      if (resultData.success) {
        ui.notif("Item updated successfully", 'bottom-center');
        renadmin_tileset_manager.showGridView();
        renadmin_tileset_manager.loadTilesetObjects();
        drawUpdatedConstraints(); // Call to redraw constraints on the tile
      } else {
        alert('Error: ' + resultData.error);
      }
    } catch (error) {
      console.error('Error parsing server response:', error);
      alert('Error parsing server response');
    }
  } catch (error) {
    alert('Error updating object: ' + error.message);
    console.log('Error:', error);
  }
});

function drawUpdatedConstraints() {
  if (renadmin_tileset_manager.selectedTiles.length === 0) {
    console.error('No tiles selected');
    return;
  }

  const tile = renadmin_tileset_manager.selectedTiles[0]; // Assuming a single tile is selected for simplicity

  if (!tile || !tile.width) {
    console.error('Invalid tile object:', tile);
    return;
  }

  const previewCanvas = document.getElementById('previewCanvas');
  const ctx = previewCanvas.getContext('2d');

  // Clear the previous selection
  ctx.clearRect(0, 0, previewCanvas.width, previewCanvas.height);

  // Redraw the tile
  const tilesetCanvas = document.getElementById('tilesetCanvas');
  const scale = previewCanvas.width / tile.width;
  previewCanvas.height = tile.height * scale;
  ctx.drawImage(
    tilesetCanvas,
    tile.x, tile.y, tile.width, tile.height,
    0, 0, previewCanvas.width, previewCanvas.height
  );

  // Apply the red background for the constraints
  ctx.fillStyle = 'rgba(255, 0, 0, 0.5)';
  const constraints = [
    parseInt(document.getElementById('nConstraint').value),
    parseInt(document.getElementById('eConstraint').value),
    parseInt(document.getElementById('sConstraint').value),
    parseInt(document.getElementById('wConstraint').value)
  ];
  ctx.fillRect(0, 0, previewCanvas.width, constraints[0] * scale); // North
  ctx.fillRect(previewCanvas.width - constraints[1] * scale, 0, constraints[1] * scale, previewCanvas.height); // East
  ctx.fillRect(0, previewCanvas.height - constraints[2] * scale, previewCanvas.width, constraints[2] * scale); // South
  ctx.fillRect(0, 0, constraints[3] * scale, previewCanvas.height); // West
}

// Event listener to update the tile constraints immediately on input change
document.getElementById('nConstraint').addEventListener('input', drawUpdatedConstraints);
document.getElementById('eConstraint').addEventListener('input', drawUpdatedConstraints);
document.getElementById('sConstraint').addEventListener('input', drawUpdatedConstraints);
document.getElementById('wConstraint').addEventListener('input', drawUpdatedConstraints);

    renadmin_tileset_manager.loadTilesetObjects = async function() {
      const tilesetName = document.getElementById('editTilesetSelect').value.replace('.png', '');

      try {
        const response = await fetch('assets/json/objectData.json');
        const objectData = await response.json();
        console.log('Loaded object data:', objectData);

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
            img.src = `/assets/img/tiles/${tilesetName}.png`;
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
              renadmin_tileset_manager.showPropertiesForObject(key, obj, tilesetName);
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

    function showPropertiesForObject(name, objectGroup, tilesetName) {
      document.getElementById('gridView').style.display = 'none';
      document.getElementById('propertiesView').style.display = 'flex';

      document.getElementById('editTileName').value = name;
      renadmin_tileset_manager.currentEditingObject = name; // Set the correct object name
      renadmin_tileset_manager.currentTilesetName = tilesetName;
      renadmin_tileset_manager.currentEditingTileIndex = -1;

      drawPreviewGrid(objectGroup, tilesetName);

      console.log('Properties shown for object:', name, objectGroup);
    }

    const tileSize = 16;
    const previewCanvas = document.getElementById('editPreviewCanvas');
    const previewCtx = previewCanvas.getContext('2d');

    function drawPreviewGrid(objectGroup, tilesetName) {
      const minX = Math.min(...objectGroup.a);
      const minY = Math.min(...objectGroup.b);
      const maxX = Math.max(...objectGroup.a) + 1;
      const maxY = Math.max(...objectGroup.b) + 1;

      previewCanvas.width = (maxX - minX) * tileSize;
      previewCanvas.height = (maxY - minY) * tileSize;

      previewCtx.clearRect(0, 0, previewCanvas.width, previewCanvas.height);

      const img = new Image();
      img.src = `/assets/img/tiles/${tilesetName}.png`;
      img.onload = function() {
        objectGroup.a.forEach((a, index) => {
          const b = objectGroup.b[index];
          const sourceX = (objectGroup.i[index] % (img.width / tileSize)) * tileSize;
          const sourceY = Math.floor(objectGroup.i[index] / (img.width / tileSize)) * tileSize;

          previewCtx.drawImage(
            img,
            sourceX, sourceY, tileSize, tileSize,
            (a - minX) * tileSize, (b - minY) * tileSize, tileSize, tileSize
          );
        });

        drawGridLines(previewCanvas, previewCtx);
      };
    }

    function drawGridLines(canvas, ctx) {
      ctx.strokeStyle = 'yellow';
      ctx.lineWidth = 1;
      for (let x = 0; x <= canvas.width; x += tileSize) {
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, canvas.height);
        ctx.stroke();
      }
      for (let y = 0; y <= canvas.height; y += tileSize) {
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(canvas.width, y);
        ctx.stroke();
      }
    }

    function highlightTile(ctx, x, y) {
      ctx.strokeStyle = 'green';
      ctx.lineWidth = 2;
      ctx.strokeRect(x * tileSize, y * tileSize, tileSize, tileSize);
    }

    function highlightConstraint(ctx, x, y, n, e, s, w) {
      ctx.fillStyle = 'rgba(255, 0, 0, 0.5)';
      
      // Top constraint
      ctx.fillRect(x * tileSize, y * tileSize, tileSize, n);
      
      // Right constraint
      ctx.fillRect((x + 1) * tileSize - e, y * tileSize, e, tileSize);
      
      // Bottom constraint
      ctx.fillRect(x * tileSize, (y + 1) * tileSize - s, tileSize, s);
      
      // Left constraint
      ctx.fillRect(x * tileSize, y * tileSize, w, tileSize);
    }

    previewCanvas.addEventListener('click', function(event) {
      const rect = previewCanvas.getBoundingClientRect();
      const scaleX = previewCanvas.width / rect.width;
      const scaleY = previewCanvas.height / rect.height;
      const x = (event.clientX - rect.left) * scaleX;
      const y = (event.clientY - rect.top) * scaleY;

      const clickedTileX = Math.floor(x / tileSize);
      const clickedTileY = Math.floor(y / tileSize);

      if (renadmin_tileset_manager.currentEditingObject && renadmin_tileset_manager.currentEditingObject.a && renadmin_tileset_manager.currentEditingObject.b) {
        const tileIndex = renadmin_tileset_manager.currentEditingObject.a.findIndex((a, index) => a === clickedTileX && renadmin_tileset_manager.currentEditingObject.b[index] === clickedTileY);

        if (tileIndex !== -1) {
          renadmin_tileset_manager.currentEditingTileIndex = tileIndex;

          // Retrieve the walkable value or constraints for the selected tile
          let walkableValue;
          if (Array.isArray(renadmin_tileset_manager.currentEditingObject.w)) {
            if (renadmin_tileset_manager.currentEditingObject.w.length > tileIndex && Array.isArray(renadmin_tileset_manager.currentEditingObject.w[tileIndex])) {
              // Individual constraints for each tile
              walkableValue = renadmin_tileset_manager.currentEditingObject.w[tileIndex];
            } else {
              // Single walkable value for all tiles
              walkableValue = [0, 0, 0, 0];
              renadmin_tileset_manager.currentEditingObject.w[tileIndex] = walkableValue; // Initialize constraints for each tile if not already present
            }
          } else {
            // Single walkable value for all tiles
            walkableValue = [0, 0, 0, 0];
            renadmin_tileset_manager.currentEditingObject.w = [];
            renadmin_tileset_manager.currentEditingObject.a.forEach(() => {
              renadmin_tileset_manager.currentEditingObject.w.push([0, 0, 0, 0]);
            });
          }

          document.getElementById('editWalkable').checked = walkableValue === 1;
          if (Array.isArray(walkableValue)) {
            document.getElementById('editNConstraint').value = walkableValue[0];
            document.getElementById('editEConstraint').value = walkableValue[1];
            document.getElementById('editSConstraint').value = walkableValue[2];
            document.getElementById('editWConstraint').value = walkableValue[3];
          } else {
            // If walkableValue is not an array, reset the constraints to default values
            document.getElementById('editNConstraint').value = 0;
            document.getElementById('editEConstraint').value = 0;
            document.getElementById('editSConstraint').value = 0;
            document.getElementById('editWConstraint').value = 0;
          }

          // Redraw the preview with the highlighted tile
          drawPreviewGrid(renadmin_tileset_manager.currentEditingObject, renadmin_tileset_manager.currentTilesetName);
          highlightTile(previewCtx, clickedTileX, clickedTileY);
          highlightConstraint(previewCtx, clickedTileX, clickedTileY, ...walkableValue);
        }
      }
    });

    document.getElementById('editNConstraint').addEventListener('input', updateTileConstraints);
    document.getElementById('editEConstraint').addEventListener('input', updateTileConstraints);
    document.getElementById('editSConstraint').addEventListener('input', updateTileConstraints);
    document.getElementById('editWConstraint').addEventListener('input', updateTileConstraints);

    function updateTileConstraints() {
      if (renadmin_tileset_manager.currentEditingTileIndex !== -1) {
        const n = parseInt(document.getElementById('editNConstraint').value, 10);
        const e = parseInt(document.getElementById('editEConstraint').value, 10);
        const s = parseInt(document.getElementById('editSConstraint').value, 10);
        const w = parseInt(document.getElementById('editWConstraint').value, 10);

        if (Array.isArray(renadmin_tileset_manager.currentEditingObject.w)) {
          if (renadmin_tileset_manager.currentEditingObject.w.length > renadmin_tileset_manager.currentEditingTileIndex && Array.isArray(renadmin_tileset_manager.currentEditingObject.w[renadmin_tileset_manager.currentEditingTileIndex])) {
            // Update constraints for the specific tile
            renadmin_tileset_manager.currentEditingObject.w[renadmin_tileset_manager.currentEditingTileIndex] = [n, e, s, w];
          }
        }

        // Redraw the preview and highlight the constraints
        drawPreviewGrid(renadmin_tileset_manager.currentEditingObject, renadmin_tileset_manager.currentTilesetName);
        const x = renadmin_tileset_manager.currentEditingObject.a[renadmin_tileset_manager.currentEditingTileIndex];
        const y = renadmin_tileset_manager.currentEditingObject.b[renadmin_tileset_manager.currentEditingTileIndex];
        highlightTile(previewCtx, x, y);
        highlightConstraint(previewCtx, x, y, n, e, s, w);
      }
    }

    renadmin_tileset_manager.showPropertiesForObject = showPropertiesForObject;

    renadmin_tileset_manager.showGridView = function() {
      document.getElementById('gridView').style.display = 'flex';
      document.getElementById('propertiesView').style.display = 'none';
      console.log('Switched to grid view');
    }

    this.loadTilesetObjects();
  },

  unmount: function() {
    document.getElementById('tilesetInput').removeEventListener('change');
    document.getElementById('tilesetCanvas').removeEventListener('mousedown');
    document.getElementById('tilesetCanvas').removeEventListener('mousemove');
    document.getElementById('tilesetCanvas').removeEventListener('mouseup');
    document.removeEventListener('keydown');
    document.removeEventListener('keyup');
    console.log('Unmounted event listeners');
  }
}

renadmin_tileset_manager.start();

</script>

<div class='resize-handle'></div>
</div>
