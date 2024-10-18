<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
    // Retrieve the item ID from the URL
    $itemId = $_GET['id'];

    // JavaScript will take care of retrieving the item details from the game object data
?>
  <div data-window='tileset_item_editor_window' class='window window_bg' style='width: 700px; background: #60975c;'>

<div data-part='handle' class='window_title' style='background-image: radial-gradient(#406d3d 1px, transparent 0) !important;'>
  <div class='float-right'>
    <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" onclick="tileset_item_editor_window.unmount()" data-close></button>
  </div>
  <div data-part='title' class='title_bg window_border' style='background: #60975c; color: #ede8d6;'>Item Editor</div>
</div>
<div class='clearfix'></div>
<div class='relative'>
  <div class='container text-light window_body p-2'>
    <div id="item_editor_tabs">
      <div id="tabs" class="flex border-b border-gray-300">
        <button class="tab text-gray-800 p-2" data-tab="tab-details">Details</button>
        <button class="tab text-gray-800 p-2" data-tab="tab-walkable">Walkable</button>
        <button class="tab text-gray-800 p-2" data-tab="tab-stack">Stack</button>
        <button class="tab text-gray-800 p-2" data-tab="tab-zindex">zIndex</button>
        <button class="tab text-gray-800 p-2" data-tab="tab-effects">Effects</button>
        <button class="tab text-gray-800 p-2" data-tab="tab-scripts">Scripts</button>
      </div>

      <div class="tab-content p-4 hidden" data-tab-content="tab-details">
        <div class="mb-4">
          <label class="block text-gray-800">Item ID:</label>
          <div id="item_id" class="text-gray-600"></div>
        </div>
        <div class="mb-4">
          <label for="item_name" class="block text-gray-800">Item Name:</label>
          <input type="text" id="item_name" class="border p-1 w-full" />
        </div>
      </div>

      <div class="tab-content p-4 hidden" data-tab-content="tab-walkable">
        <div class="mb-2 text-gray-800">Click on the grid to draw straight lines. Connect the lines to form a polygon.</div>

        <button id="clear_polygon_button" class="p-2 bg-red-500 text-white rounded mb-4">Clear Polygon</button>
        <div class="mt-4 relative flex justify-center items-center">
          <div class="relative">
            <!-- New canvas for grid background -->
            <canvas id="item_grid_background_canvas_walkable" class="absolute inset-0 pointer-events-none"></canvas>
            <canvas id="item_preview_canvas_walkable" class="block mx-auto"></canvas>
            <canvas id="item_grid_canvas_walkable" class="absolute inset-0"></canvas>
          </div>
        </div>
      </div>

          <div class="tab-content p-4 hidden" data-tab-content="tab-stack">
            <!-- Content removed as requested -->
            <div class="mt-4 relative flex justify-center items-center">
              <div class="relative">
                <canvas id="item_preview_canvas_stack" class="block mx-auto"></canvas>
                <canvas id="item_grid_canvas_stack" class="absolute inset-0 pointer-events-none"></canvas>
              </div>
            </div>
          </div>

          <div class="tab-content p-4 hidden" data-tab-content="tab-zindex">
            <div id="zindex_controls" class="flex justify-between mb-2 hidden">
              <label>
                zIndex: <input type="text" id="zindex_input" class="border p-1 w-12">
              </label>
            </div>

            <!-- Content removed as requested -->
            <div class="mt-4 relative flex justify-center items-center">
              <div class="relative">
                <canvas id="item_preview_canvas_zindex" class="block mx-auto"></canvas>
                <canvas id="item_grid_canvas_zindex" class="absolute inset-0 pointer-events-none"></canvas>
              </div>
            </div>
          </div>

          <div class="tab-content p-4 hidden" data-tab-content="tab-effects">
            <!-- Content removed as requested -->
            <div class="mt-4 relative flex justify-center items-center">
              <div class="relative">
                <canvas id="item_preview_canvas_effects" class="block mx-auto"></canvas>
                <canvas id="item_grid_canvas_effects" class="absolute inset-0 pointer-events-none"></canvas>
              </div>
            </div>
          </div>

          <div class="tab-content p-4 hidden" data-tab-content="tab-scripts">
            <!-- New content for Scripts -->
            <div class="mb-4">
    <textarea id="item_scripts" class="border p-2 w-full h-96 bg-gray-800 text-gray-200 font-mono border-gray-700"></textarea>
  </div>
          </div>

        </div>
      </div>
      <button id="save_button" class="mt-4 p-2 bg-green-500 text-white rounded">Save</button>
    </div>

    <script>
      var tileset_item_editor_window = {
        walkableData: {},
    zIndexData: {},
    polygonPoints: [],
    isResizing: false,
    currentlyResizingPoint: null,

    start: function(itemId) {
    console.log("Item Editor Modal Opened for item ID:", itemId);
    if (!game.objectData.hasOwnProperty(itemId)) {
        console.error("Item ID not found in game.objectData:", itemId);
        return;
    }
    var item = game.objectData[itemId][0];
    console.log("Loaded item data:", item);
    if (!item) {
        console.error("Item not found or is invalid!");
        return;
    }

    // Log the current collision data (w) before any modifications
    console.log("Current 'w' (collision) data:", item.w || "No collision data available");

    this.initializeOtherData(itemId, item);
    
    // Call the function to initialize zIndexData
    this.initializeZIndexData(item);

    this.setupLineDrawingHandlers('item_grid_canvas_walkable');
    this.renderPolygonOnLoad(item);
    if (item && Array.isArray(item.a) && Array.isArray(item.b)) {
        this.drawGrid('item_grid_background_canvas_walkable', item);
    } else {
        console.warn("Skipping background grid drawing due to invalid item data");
    }

    if (item.script) {
        // Load YAML script directly into the textarea
        const yamlScript = item.script;
        document.getElementById('item_scripts').value = yamlScript; // Display YAML script in editor
    } else {
        document.getElementById('item_scripts').value = '';
    }

    this.setupScriptInputHandlers();
},

setupScriptInputHandlers: function() {
    const textarea = document.getElementById('item_scripts');
    const tabSize = 4; // Define the tab size as 4 spaces

    textarea.addEventListener('keydown', function(event) {
        // Handle Tab key for indentation
        if (event.key === 'Tab') {
            event.preventDefault();
            const start = textarea.selectionStart;
            const end = textarea.selectionEnd;

            // Insert four spaces for YAML indentation
            const indent = ' '.repeat(tabSize); // Four spaces for YAML indentation
            textarea.value = textarea.value.substring(0, start) + indent + textarea.value.substring(end);

            // Move the cursor forward by the length of the indent
            textarea.selectionStart = textarea.selectionEnd = start + indent.length;
        }

        // Handle Backspace key to remove a full tab (four spaces)
        if (event.key === 'Backspace') {
            const start = textarea.selectionStart;
            const textBeforeCursor = textarea.value.substring(0, start);
            if (textBeforeCursor.endsWith(' '.repeat(tabSize))) {
                event.preventDefault();
                textarea.value = textBeforeCursor.slice(0, -tabSize) + textarea.value.substring(start);
                textarea.selectionStart = textarea.selectionEnd = start - tabSize;
            }
        }

        // Handle Enter key for creating a new line with proper indentation
        if (event.key === 'Enter') {
            event.preventDefault(); // Prevent the default action of the Enter key

            const start = textarea.selectionStart; // Cursor position
            const textBeforeCursor = textarea.value.substring(0, start); // Text before the cursor
            const currentLine = textBeforeCursor.substring(textBeforeCursor.lastIndexOf('\n') + 1); // Current line content
            const indentMatch = currentLine.match(/^\s*/); // Match leading spaces for indentation

            let indent = '';
            if (indentMatch) {
                indent = indentMatch[0]; // Preserve the current indentation level
            }

            const textAfterCursor = textarea.value.substring(textarea.selectionEnd); // Text after the cursor

            // Check if the current line ends with a colon (YAML structure indicator)
            if (currentLine.trim().endsWith(':')) {
                indent += ' '.repeat(tabSize); // Add an additional level of indentation if the line ends with ":"
            }

            // Insert a new line with the correct level of indentation
            textarea.value = textBeforeCursor + '\n' + indent + textAfterCursor;

            // Move the cursor to the end of the new line with the correct indentation
            textarea.selectionStart = textarea.selectionEnd = start + 1 + indent.length;

            // Ensure the textarea scrolls to show the new line
            requestAnimationFrame(() => {
                textarea.scrollTop = textarea.scrollHeight;
            });
        }
    });
},

    initializeOtherData: function(itemId, item) {
        this.walkableData = this.initializeWalkableData(item);
        this.zIndexData = this.initializeZIndexData(item);

        this.setupCanvasClickHandlersZIndex(item, 'item_preview_canvas_zindex');

        document.getElementById('item_id').textContent = itemId;

        // Prefill the item name
        document.getElementById('item_name').value = item.n;

        this.renderItemPreview(item, 'item_preview_canvas_walkable');
        this.renderItemPreview(item, 'item_preview_canvas_stack');
        this.renderItemPreview(item, 'item_preview_canvas_zindex');
        this.renderItemPreview(item, 'item_preview_canvas_effects');

        this.drawGrid('item_grid_canvas_walkable', item, true);
        this.drawGrid('item_grid_canvas_stack', item);
        this.drawGrid('item_grid_canvas_zindex', item);
        this.drawGrid('item_grid_canvas_effects', item);

        ui.initTabs('item_editor_tabs', 'tab-details');

        this.setupZIndexInputHandlers();

        document.getElementById('save_button').addEventListener('click', this.saveData.bind(this, itemId));

        document.getElementById('clear_polygon_button').addEventListener('click', this.clearPolygon.bind(this));
    },

    initializeWalkableData: function(item) {
        let walkableData = {};

        if (Array.isArray(item.walkablePolygon)) {
            walkableData.polygon = item.walkablePolygon.map(p => ({ x: p[0], y: p[1] }));
        } else if (item.w && Array.isArray(item.w)) {
            walkableData.polygon = item.w;
        } else {
            walkableData.polygon = [];
        }

        return walkableData;
    },

    initializeZIndexData: function(item) {
    let zIndexData = {};

    // Calculate total number of tiles based on grid size (a * b)
    const tileCount = (item.a + 1) * (item.b + 1);

    // Handle case where z is a single number (apply the same zIndex to all tiles)
    if (typeof item.z === 'number') {
        for (let index = 0; index < tileCount; index++) {
            let x = index % (item.a + 1);  // X coordinate
            let y = Math.floor(index / (item.a + 1));  // Y coordinate
            let key = `${x},${y}`;  // Tile key for zIndexData
            zIndexData[key] = item.z.toString();  // Apply the same zIndex to all tiles
        }
    }

    // Handle case where z is an array (each tile gets its own zIndex value)
    else if (Array.isArray(item.z)) {
        item.z.forEach((zValue, index) => {
            let x = index % (item.a + 1);  // X coordinate
            let y = Math.floor(index / (item.a + 1));  // Y coordinate
            let key = `${x},${y}`;  // Tile key for zIndexData
            zIndexData[key] = zValue.toString();  // Apply respective zIndex value
        });
    }

    // Default case if z is not defined (initialize zIndex to 0 for all tiles)
    else {
        for (let index = 0; index < tileCount; index++) {
            let x = index % (item.a + 1);  // X coordinate
            let y = Math.floor(index / (item.a + 1));  // Y coordinate
            let key = `${x},${y}`;  // Tile key for zIndexData
            zIndexData[key] = '0';  // Default zIndex is 0
        }
    }

    // Assign the zIndexData to the editor window
    tileset_item_editor_window.zIndexData = zIndexData;
    console.log("Initialized zIndexData:", tileset_item_editor_window.zIndexData);
},

renderPolygonOnLoad: function(item) {
        const canvas = document.getElementById('item_grid_canvas_walkable');
        const ctx = canvas.getContext('2d');

        if (this.walkableData.polygon.length > 0) {
            this.renderPolygon(ctx, [this.walkableData.polygon]);
            this.addResizeHandles(ctx); // Add this line to show the resize handles on load
        }
    },


    clearPolygon: function() {
        this.walkableData.polygon = [];
        this.polygonPoints = [];

        const canvas = document.getElementById('item_grid_canvas_walkable');
        const ctx = canvas.getContext('2d');
        ctx.clearRect(0, 0, canvas.width, canvas.height);

        // Clear the 'w' field in the item data
        game.objectData[<?php echo json_encode($itemId); ?>][0].w = [];

        console.log('Polygon data cleared');
    },

    setupLineDrawingHandlers: function(canvasId) {
    var canvas = document.getElementById(canvasId);
    var ctx = canvas.getContext('2d');
    const tileSize = 16;

    const scaleFactor = this.getScaleFactor(canvas);

    this.polygonPoints = [];
    this.isDrawing = false;
    this.isResizing = false;
    this.isDragging = false;
    this.mouseDownPosition = { x: 0, y: 0 };

    // Right-click to remove point
    canvas.addEventListener('contextmenu', (event) => {
        event.preventDefault();
        this.removePoint(event, canvas, scaleFactor);
    });

    canvas.addEventListener('mousedown', (event) => {
        this.mouseDownPosition = {
            x: event.clientX,
            y: event.clientY
        };
        this.isDragging = false;
        this.startResizing(canvas, event);
    });

    canvas.addEventListener('mousemove', (event) => {
        const deltaX = Math.abs(event.clientX - this.mouseDownPosition.x);
        const deltaY = Math.abs(event.clientY - this.mouseDownPosition.y);
        const movementThreshold = 0;

        if (deltaX > movementThreshold || deltaY > movementThreshold) {
            this.isDragging = true;
            this.resizePoint(canvas, event);
        }
    });

    canvas.addEventListener('mouseup', (event) => {
        this.stopResizing();

        if (!this.isDragging) {
            // This is a click, not a drag
            this.addPoint(event, canvas, ctx, scaleFactor);
        }
    });
},

addPoint: function(event, canvas, ctx, scaleFactor) {
    let rawX = (event.clientX - canvas.getBoundingClientRect().left) / scaleFactor;
    let rawY = (event.clientY - canvas.getBoundingClientRect().top) / scaleFactor;

    let x = Math.round(rawX);
    let y = Math.round(rawY);

    // If there is already a point, adjust the new point to align it
    if (this.polygonPoints.length > 0) {
        const lastPoint = this.polygonPoints[this.polygonPoints.length - 1];

        // Calculate the differences in x and y axes
        const dx = Math.abs(x - lastPoint.x);
        const dy = Math.abs(y - lastPoint.y);

        // If horizontal movement is larger, align vertically
        if (dx > dy) {
            y = lastPoint.y;
        }
        // If vertical movement is larger, align horizontally
        else if (dy > dx) {
            x = lastPoint.x;
        }
        // If the movement is diagonal, make it a perfect diagonal
        else {
            const signX = x > lastPoint.x ? 1 : -1;
            const signY = y > lastPoint.y ? 1 : -1;
            x = lastPoint.x + signX * dx;
            y = lastPoint.y + signY * dy;
        }
    }

    // Now push the adjusted point to the polygon points array
    this.polygonPoints.push({ x, y });
    this.renderPolygon(ctx, [...this.walkableData.polygon, this.polygonPoints]);

    // Update the 'w' field in the item data in real-time
    this.updateGameObjectData();

    if (this.polygonPoints.length > 1) {
        const prevPoint = this.polygonPoints[this.polygonPoints.length - 2];
        ctx.beginPath();
        ctx.moveTo(prevPoint.x, prevPoint.y);
        ctx.lineTo(x, y);
        ctx.stroke();
    }

    this.addResizeHandles(ctx);
},

removePoint: function(event, canvas, scaleFactor) {
    let rawX = (event.clientX - canvas.getBoundingClientRect().left) / scaleFactor;
    let rawY = (event.clientY - canvas.getBoundingClientRect().top) / scaleFactor;

    let x = Math.round(rawX);
    let y = Math.round(rawY);

    // Find the closest point and remove it
    let closestPointIndex = null;
    let minDistance = Infinity;

    for (let i = 0; i < this.polygonPoints.length; i++) {
        const point = this.polygonPoints[i];
        const distance = Math.sqrt(Math.pow(point.x - x, 2) + Math.pow(point.y - y, 2));

        if (distance < minDistance) {
            minDistance = distance;
            closestPointIndex = i;
        }
    }

    if (closestPointIndex !== null && minDistance < 10) { // Adjust threshold as needed
        this.polygonPoints.splice(closestPointIndex, 0);
        this.updatePolygon();
        this.updateGameObjectData();
    }
},

addResizeHandles: function(ctx) {
    ctx.fillStyle = 'blue';
    const handleRadius = 2; // Adjust the size of the resize handles

    this.polygonPoints.forEach(point => {
        ctx.beginPath();
        ctx.arc(point.x, point.y, handleRadius, 0, 2 * Math.PI);
        ctx.fill();
    });
},


    startResizing: function(canvas, event) {
        const scaleFactor = this.getScaleFactor(canvas);
        let rawX = (event.clientX - canvas.getBoundingClientRect().left) / scaleFactor;
        let rawY = (event.clientY - canvas.getBoundingClientRect().top) / scaleFactor;

        let x = Math.round(rawX);
        let y = Math.round(rawY);

        this.currentlyResizingPoint = this.polygonPoints.find(point => {
            return Math.abs(point.x - x) < 5 && Math.abs(point.y - y) < 5;
        });

        if (this.currentlyResizingPoint) {
            this.isResizing = true;
        }
    },

    resizePoint: function(canvas, event) {
    if (!this.isResizing || !this.currentlyResizingPoint) return;

    const scaleFactor = this.getScaleFactor(canvas);
    let rawX = (event.clientX - canvas.getBoundingClientRect().left) / scaleFactor;
    let rawY = (event.clientY - canvas.getBoundingClientRect().top) / scaleFactor;

    let x = Math.round(rawX);
    let y = Math.round(rawY);

    // Update the current point position
    this.currentlyResizingPoint.x = x;
    this.currentlyResizingPoint.y = y;

    // Update the polygon and the game object data in real-time
    this.updatePolygon();
},

stopResizing: function() {
    this.isResizing = false;
    this.currentlyResizingPoint = null;

    // Update the 'w' field in the item data after resizing
    game.objectData[<?php echo json_encode($itemId); ?>][0].w = [...this.walkableData.polygon];
},

updatePolygon: function() {
    const canvas = document.getElementById('item_grid_canvas_walkable');
    const ctx = canvas.getContext('2d');

    // Update the polygon rendering and handles
    this.renderPolygon(ctx, [...this.walkableData.polygon, this.polygonPoints]);
    this.addResizeHandles(ctx);

    // Update game object data in real-time during dragging
    this.updateGameObjectData();
},

    renderPolygon: function(ctx, polygons) {
    ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);

    polygons.forEach(path => {
        ctx.beginPath();
        ctx.moveTo(path[0].x, path[0].y);
        for (let i = 1; i < path.length; i++) {
            ctx.lineTo(path[i].x, path[i].y);
        }
        ctx.closePath();
        ctx.strokeStyle = 'rgba(255, 0, 0, 1)';
        ctx.stroke();
        ctx.fillStyle = 'rgba(255, 0, 0, 0.6)';
        ctx.fill();
    });

    this.addResizeHandles(ctx); // Ensure handles are added after each polygon render
},

        getScaleFactor: function(canvas) {
          const modalWidth = document.querySelector('[data-window="tileset_item_editor_window"]').clientWidth;
          const maxCanvasWidth = modalWidth - 40;
          return Math.min(5, maxCanvasWidth / canvas.width);
        },

        setupCanvasClickHandlersZIndex: function(item, canvasId) {
          var canvas = document.getElementById(canvasId);
          const tileSize = 16;

          canvas.addEventListener('click', (event) => {
            const rect = canvas.getBoundingClientRect();
            const modalWidth = document.querySelector('[data-window="tileset_item_editor_window"]').clientWidth;
            const maxCanvasWidth = modalWidth - 40;
            const scaleFactor = Math.min(5, maxCanvasWidth / canvas.width);

            const clickX = (event.clientX - rect.left) / scaleFactor;
            const clickY = (event.clientY - rect.top) / scaleFactor;

            const x = Math.floor(clickX / tileSize);
            const y = Math.floor(clickY / tileSize);
            const tileKey = `${x},${y}`;

            console.log(`Tile clicked for zIndex: (${x}, ${y})`);

            document.getElementById('zindex_input').value = this.zIndexData[tileKey];
            document.getElementById('zindex_controls').classList.remove('hidden');
            document.getElementById('zindex_controls').dataset.tileKey = tileKey;
            document.getElementById('zindex_input').focus();

            this.updateCanvasZIndex(tileKey);
          });
        },

        setupZIndexInputHandlers: function() {
          const input = document.getElementById('zindex_input');
          input.addEventListener('input', (event) => {
            const value = event.target.value;
            const tileKey = document.getElementById('zindex_controls').dataset.tileKey;

            this.zIndexData[tileKey] = value;

            console.log('Updated zIndex data:', this.zIndexData);
            this.updateCanvasZIndex(tileKey);
          });
        },

        updateCanvasZIndex: function(tileKey) {
    var item = game.objectData[<?php echo json_encode($itemId); ?>][0];

    // Construct the zIndex array
    const zIndexArray = item.i.map((tileIndex, index) => {
        let x = item.a[index];
        let y = item.b[index];
        let key = `${x},${y}`;
        return this.zIndexData[key] ? parseInt(this.zIndexData[key], 10) : 0;
    });

    // Check if all z-index values are the same
    const allSameZ = zIndexArray.every((z, _, arr) => z === arr[0]);

    if (allSameZ) {
        item.z = zIndexArray[0]; // Save as a single integer if all are the same
    } else {
        item.z = zIndexArray; // Save as an array if they differ
    }

    console.log('Updated zIndex array:', item.z); // Properly log the array or value
    this.renderItemPreview(item, 'item_preview_canvas_zindex');
    this.drawGrid('item_grid_canvas_zindex', item);
},


renderItemPreview: function(item, canvasId) {
    var canvas = document.getElementById(canvasId);
    var ctx = canvas.getContext('2d');
    const tileSize = 16;
    const tilesPerRow = 150;  // Assuming 150 tiles per row in your tileset image

    // Adjust canvas size based on the number of columns (a) and rows (b)
    canvas.width = (item.a + 1) * tileSize; // Width is determined by columns (X-axis)
    canvas.height = (item.b + 1) * tileSize; // Height is determined by rows (Y-axis)

    var tilesetImage = assets.load(item.t);

    ctx.clearRect(0, 0, canvas.width, canvas.height);

    // Helper function to parse ranges like "2009-2218"
    function parseRange(range) {
        const [start, end] = range.split('-').map(Number);
        const frames = [];
        for (let i = start; i <= end; i++) {
            frames.push(i);
        }
        return frames;
    }

    // Determine which frames to render (from tilesheet indices)
    let framesToRender = [];

    if (Array.isArray(item.i)) {
        framesToRender = item.i.flatMap(frame => typeof frame === 'string' && frame.includes('-') ? parseRange(frame) : frame);
    } else {
        framesToRender = [item.i];  // Handle single frame
    }

    // Iterate over the frame indices and calculate the X and Y positions in the grid
    framesToRender.forEach((frame, index) => {
        const srcX = (frame % tilesPerRow) * tileSize;
        const srcY = Math.floor(frame / tilesPerRow) * tileSize;

        // Calculate destination X and Y based on object's dimensions (a and b)
        const destX = (index % (item.a + 1)) * tileSize; // X (horizontal) based on columns
        const destY = Math.floor(index / (item.a + 1)) * tileSize; // Y (vertical) based on rows

        // Log the destination X and Y for debugging
        console.log(`Tile index ${frame}: destX = ${destX}, destY = ${destY}`);

        // Draw the image tile on the canvas
        ctx.drawImage(
            tilesetImage,
            srcX, srcY, tileSize, tileSize,  // Source (tilesheet position)
            destX, destY, tileSize, tileSize  // Destination (canvas position)
        );
    });

    // Apply scaling if necessary
    const scaleFactor = this.getScaleFactor(canvas);
    canvas.style.width = (canvas.width * scaleFactor) + 'px';
    canvas.style.height = (canvas.height * scaleFactor) + 'px';
},


drawGrid: function(canvasId, item, skipGridLines = false) {
    var canvas = document.getElementById(canvasId);
    var ctx = canvas.getContext('2d');
    const tileSize = 16;

    // Validate item and its properties
    if (!item || (!Array.isArray(item.a) && typeof item.a !== 'number') || (!Array.isArray(item.b) && typeof item.b !== 'number')) {
        console.error('Invalid item data:', item);
        return;
    }

    console.log('Drawing grid for item:', item);

    // Check if item.a and item.b are arrays, otherwise treat them as single values
    const maxCol = Array.isArray(item.a) ? Math.max(...item.a) + 1 : item.a + 1; // Ensure we cover the entire width of the object
    const maxRow = Array.isArray(item.b) ? Math.max(...item.b) + 1 : item.b + 1; // Ensure we cover the entire height of the object

    // Adjust canvas size based on the item's dimensions
    canvas.width = maxCol * tileSize;
    canvas.height = maxRow * tileSize;

    console.log('Canvas dimensions set to:', canvas.width, 'x', canvas.height);

    ctx.clearRect(0, 0, canvas.width, canvas.height);

    // Draw a border around the canvas to show the grid's outline
    ctx.strokeStyle = 'blue';  // Use blue for the border color
    ctx.lineWidth = 3;  // Set border width to 3 pixels
    ctx.strokeRect(0, 0, canvas.width, canvas.height);  // Draw the border

    if (!skipGridLines) {
        ctx.strokeStyle = 'rgba(136, 136, 136, 1)';
        ctx.lineWidth = 1;

        // Draw vertical grid lines
        for (let x = 0.5; x <= canvas.width; x += tileSize) {
            ctx.beginPath();
            ctx.moveTo(x, 0.5);
            ctx.lineTo(x, canvas.height);
            ctx.stroke();
        }

        // Draw horizontal grid lines
        for (let y = 0.5; y <= canvas.height; y += tileSize) {
            ctx.beginPath();
            ctx.moveTo(0.5, y);
            ctx.lineTo(canvas.width, y);
            ctx.stroke();
        }
    }

    const scaleFactor = this.getScaleFactor(canvas);

    canvas.style.width = (canvas.width * scaleFactor) + 'px';
    canvas.style.height = (canvas.height * scaleFactor) + 'px';

    console.log('Grid drawn successfully on canvas:', canvasId);
},




        initializeWalkableData: function(item) {
  let walkableData = {};

  if (Array.isArray(item.walkablePolygon)) {
    // Convert the old format to the new polygon format
    walkableData.polygon = item.walkablePolygon.map(p => ({ x: p[0], y: p[1] }));
  } else if (item.w && Array.isArray(item.w)) {
    // Support the new format if it's already in place
    walkableData.polygon = item.w;
  } else {
    walkableData.polygon = [];
  }

  return walkableData;
},

updateGameObjectData: function() {
    // Directly update the game object data with the current polygon points
    const itemId = <?php echo json_encode($itemId); ?>;
    const item = game.objectData[itemId][0];
    
    item.w = [...this.walkableData.polygon, this.polygonPoints];
    
    console.log('Updated objectData in real-time:', item.w);
},

saveData: function(itemId) {
    var item = game.objectData[itemId][0];

    // Get the item name from the input field
    var itemName = document.getElementById('item_name').value.trim();
    item.n = itemName;

    // Get the walkable polygon data from the points and combine it with the existing polygon data
    item.w = this.walkableData.polygon.concat(this.polygonPoints);
    
    console.log("Walkable data saved:", item.w);

    // Initialize zIndex array
    const zIndexArray = [];

    // Collect zIndex values from tileset_item_editor_window.zIndexData
    for (let key in this.zIndexData) {
        if (this.zIndexData.hasOwnProperty(key)) {
            let zIndexValue = parseInt(this.zIndexData[key], 10) || 0; // Default to 0 if empty or invalid
            zIndexArray.push(zIndexValue);
            console.log(`zIndex for tile ${key} is: ${zIndexValue}`);
        }
    }

    // Save the zIndex array to the item
    item.z = zIndexArray;

    console.log("zIndexArray:", zIndexArray);
    console.log("item.z:", item.z);

    // Collect the script from the editor in YAML format
    var itemScripts = document.getElementById('item_scripts').value.trim();

    if (itemScripts) {
        // Clean the YAML script before saving
        var cleanYamlScript = itemScripts
            .replace(/,+\n(\s*)/g, '\n$1')      // Remove one or more commas before newlines, preserving indentation
            .replace(/,(\s*[\}\]])/g, '$1')    // Remove commas before closing braces/brackets
            .replace(/,(\s*[\}\]]\s*)/g, '$1') // Ensure multiple consecutive commas before closing braces/brackets are removed
            .replace(/,+\s*$/g, '');           // Remove any trailing commas at the end of the script

        item.script = cleanYamlScript;
    } else {
        delete item.script; // Remove the script if it's empty
    }

    console.log("Cleaned script with proper indentation:", item.script);

    // Perform an AJAX request to save the data on the server
    ui.ajax({
        method: 'POST',
        url: 'modals/renadmin/tileset/ajax/save_item.php',
        data: JSON.stringify(game.objectData), // Send the whole object data as JSON
        outputType: 'json',
        success: function(response) {
            if (response.success) {
                ui.notif("Data saved successfully!");
            } else {
                ui.notif("Error saving data: " + response.message, "error");
                console.error('Server returned an error:', response.message);
            }
        },
        error: function(err) {
            console.error('Failed to save data:', err);
            err.text().then(text => {
                console.error('Response from server:', text);
                ui.notif("Failed to save data. See console for details.", "error");
            });
        }
    });
},

unmount: function() {
        console.log("Unmounting Item Editor Modal");
        var modalElement = document.querySelector('[data-window="tileset_item_editor_window"]');
        if (modalElement) {
            modalElement.remove();
        }
    }
      };

      tileset_item_editor_window.start(<?php echo json_encode($itemId); ?>);
    </script>

    <div class='resize-handle'></div>
  </div>
<?php
} else {
    echo "Unauthorized access.";
}
?>
