<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config/db.php';
if ($auth) {
    // Retrieve the item ID from the URL
    $itemId = $_GET['id'];

    // JavaScript will take care of retrieving the item details from the game object data
?>
  <div class='window window_bg' style='width: 700px; background: #60975c;'>

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
    <div class='resize-handle'></div>
    </div>

    <script>
tileset_item_editor_window = {
        walkableData: {},
    polygonPoints: [],
    isResizing: false,
    currentlyResizingPoint: null,

    start: function(itemId) {
    console.log("Item Editor plugin Opened for item ID:", itemId);
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

    this.setupLineDrawingHandlers('item_grid_canvas_walkable');
    this.renderPolygonOnLoad(item);
    if (item && Array.isArray(item.a) && Array.isArray(item.b)) {
        this.drawGrid('item_grid_background_canvas_walkable', item, false, 10);
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

    // Populate polygonPoints from walkableData
    this.polygonPoints = [...this.walkableData.polygon];

    document.getElementById('item_id').textContent = itemId;
    document.getElementById('item_name').value = item.n;

    this.renderItemPreview(item, 'item_preview_canvas_walkable', 10);

    // Render the polygon and ensure resize handles are added
    this.renderPolygonOnLoad(item);

    this.drawGrid('item_grid_canvas_walkable', item, false, 10);
    ui.initTabs('item_editor_tabs', 'tab-details');

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

renderPolygonOnLoad: function(item) {
    const canvas = document.getElementById('item_grid_canvas_walkable');
    const ctx = canvas.getContext('2d');
    const padding = 10;

    if (this.walkableData.polygon.length > 0) {
        // Render the polygon
        this.renderPolygon(ctx, [this.walkableData.polygon], padding);

        // Add resize handles explicitly
        this.addResizeHandles(ctx, padding);
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
    const padding = 10; // Margin around the canvas
    let rawX = (event.clientX - canvas.getBoundingClientRect().left) / scaleFactor;
    let rawY = (event.clientY - canvas.getBoundingClientRect().top) / scaleFactor;

    let x = Math.round(rawX) - padding; // Remove margin for recalculated position
    let y = Math.round(rawY) - padding; // Remove margin for recalculated position

    if (this.polygonPoints.length > 0) {
        const lastPoint = this.polygonPoints[this.polygonPoints.length - 1];

        const dx = Math.abs(x - lastPoint.x);
        const dy = Math.abs(y - lastPoint.y);

        if (dx > dy) {
            y = lastPoint.y;
        } else if (dy > dx) {
            x = lastPoint.x;
        } else {
            const signX = x > lastPoint.x ? 1 : -1;
            const signY = y > lastPoint.y ? 1 : -1;
            x = lastPoint.x + signX * dx;
            y = lastPoint.y + signY * dy;
        }
    }

    this.polygonPoints.push({ x, y }); // Add new point
    this.renderPolygon(ctx, [...this.walkableData.polygon, this.polygonPoints], padding); // Render polygon with padding
    this.addResizeHandles(ctx, padding); // Add resize handles

    this.updateGameObjectData(); // Update objectData in real-time
},


removePoint: function(event, canvas, scaleFactor) {
    const padding = 10;
    let rawX = (event.clientX - canvas.getBoundingClientRect().left) / scaleFactor;
    let rawY = (event.clientY - canvas.getBoundingClientRect().top) / scaleFactor;

    let x = Math.round(rawX) - padding;
    let y = Math.round(rawY) - padding;

    // Find the closest point to the cursor
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
        this.polygonPoints.splice(closestPointIndex, 1); // Remove the point
        this.updatePolygon(); // Redraw the polygon
        this.updateGameObjectData(); // Update objectData in real-time
    }
},


addResizeHandles: function(ctx, padding = 0) {
    ctx.fillStyle = 'blue'; // Color of the resize handles
    const handleRadius = 4; // Radius of the resize handles

    this.polygonPoints.forEach(point => {
        ctx.beginPath();
        ctx.arc(point.x + padding, point.y + padding, handleRadius, 0, 2 * Math.PI); // Render with padding
        ctx.fill();
    });
},

startResizing: function(canvas, event) {
    const scaleFactor = this.getScaleFactor(canvas);
    const padding = 10;

    let rawX = (event.clientX - canvas.getBoundingClientRect().left) / scaleFactor;
    let rawY = (event.clientY - canvas.getBoundingClientRect().top) / scaleFactor;

    let x = Math.round(rawX) - padding; // Adjust for margin
    let y = Math.round(rawY) - padding; // Adjust for margin

    // Find the closest point to the cursor (including padding)
    this.currentlyResizingPoint = this.polygonPoints.find(point => {
        return Math.abs(point.x - x) < 5 && Math.abs(point.y - y) < 5; // Adjust the threshold as needed
    });

    if (this.currentlyResizingPoint) {
        this.isResizing = true;
    }
},

resizePoint: function(canvas, event) {
    if (!this.isResizing || !this.currentlyResizingPoint) return;

    const scaleFactor = this.getScaleFactor(canvas);
    const padding = 10;

    let rawX = (event.clientX - canvas.getBoundingClientRect().left) / scaleFactor;
    let rawY = (event.clientY - canvas.getBoundingClientRect().top) / scaleFactor;

    let x = Math.round(rawX) - padding; // Remove margin for recalculated position
    let y = Math.round(rawY) - padding; // Remove margin for recalculated position

    this.currentlyResizingPoint.x = x;
    this.currentlyResizingPoint.y = y;

    this.updatePolygon(); // Redraw the polygon and resize handles
    this.updateGameObjectData(); // Update objectData in real-time
},


stopResizing: function() {
    this.isResizing = false;
    this.currentlyResizingPoint = null;

    // Update the 'w' field in the item data after resizing
    game.objectData[<?php echo json_encode($itemId); ?>][0].w = [...this.walkableData.polygon, ...this.polygonPoints];
},


updatePolygon: function() {
    const canvas = document.getElementById('item_grid_canvas_walkable');
    const ctx = canvas.getContext('2d');
    const padding = 10;

    this.renderPolygon(ctx, [...this.walkableData.polygon, this.polygonPoints], padding);
    this.addResizeHandles(ctx, padding);

    this.updateGameObjectData(); // Update objectData after polygon updates
},


renderPolygon: function(ctx, polygons, padding = 0) {
    ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);

    polygons.forEach(path => {
        if (path.length === 0) return; // Skip empty paths
        ctx.beginPath();
        ctx.moveTo(path[0].x + padding, path[0].y + padding); // Add padding for rendering
        for (let i = 1; i < path.length; i++) {
            ctx.lineTo(path[i].x + padding, path[i].y + padding); // Add padding for rendering
        }
        ctx.closePath();
        ctx.strokeStyle = 'rgba(255, 0, 0, 1)';
        ctx.stroke();
        ctx.fillStyle = 'rgba(255, 0, 0, 0.6)';
        ctx.fill();
    });

    // Add resize handles after drawing the polygon
    this.addResizeHandles(ctx, padding);
},


        getScaleFactor: function(canvas) {
          const pluginWidth = document.querySelector('[data-window="tileset_item_editor_window"]').clientWidth;
          const maxCanvasWidth = pluginWidth - 40;
          return Math.min(5, maxCanvasWidth / canvas.width);
        },

renderItemPreview: function(item, canvasId, padding = 10) {
    var canvas = document.getElementById(canvasId);
    var ctx = canvas.getContext('2d');
    const tileSize = 16;
    const tilesPerRow = 150; // Assuming 150 tiles per row in your tileset image

    // Adjust canvas size based on the number of columns (a) and rows (b) and padding
    canvas.width = (item.a + 1) * tileSize + padding * 2; // Width with padding
    canvas.height = (item.b + 1) * tileSize + padding * 2; // Height with padding

    var tilesetImage = assets.use(item.t);

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
        framesToRender = [item.i]; // Handle single frame
    }

    // Iterate over the frame indices and calculate the X and Y positions in the grid
    framesToRender.forEach((frame, index) => {
        const srcX = (frame % tilesPerRow) * tileSize;
        const srcY = Math.floor(frame / tilesPerRow) * tileSize;

        // Calculate destination X and Y with padding
        const destX = padding + (index % (item.a + 1)) * tileSize; // X (horizontal) based on columns
        const destY = padding + Math.floor(index / (item.a + 1)) * tileSize; // Y (vertical) based on rows

        // Log the destination X and Y for debugging
        console.log(`Tile index ${frame}: destX = ${destX}, destY = ${destY}`);

        // Draw the image tile on the canvas
        ctx.drawImage(
            tilesetImage,
            srcX, srcY, tileSize, tileSize, // Source (tilesheet position)
            destX, destY, tileSize, tileSize // Destination (canvas position)
        );
    });

    // Apply scaling if necessary
    const scaleFactor = this.getScaleFactor(canvas);
    canvas.style.width = (canvas.width * scaleFactor) + 'px';
    canvas.style.height = (canvas.height * scaleFactor) + 'px';
},


drawGrid: function(canvasId, item, skipGridLines = false, padding = 10) {
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

    // Adjust canvas size based on the item's dimensions and padding
    canvas.width = maxCol * tileSize + padding * 2;
    canvas.height = maxRow * tileSize + padding * 2;

    console.log('Canvas dimensions set to:', canvas.width, 'x', canvas.height);

    ctx.clearRect(0, 0, canvas.width, canvas.height);

    // Draw a border around the canvas to show the grid's outline
    ctx.strokeStyle = 'blue'; // Use blue for the border color
    ctx.lineWidth = 3; // Set border width to 3 pixels
    ctx.strokeRect(padding, padding, canvas.width - padding * 2, canvas.height - padding * 2); // Draw the border within the padding

    if (!skipGridLines) {
        ctx.strokeStyle = 'rgba(136, 136, 136, 1)';
        ctx.lineWidth = 1;

        // Draw vertical grid lines
        for (let x = padding + 0.5; x <= canvas.width - padding; x += tileSize) {
            ctx.beginPath();
            ctx.moveTo(x, padding + 0.5);
            ctx.lineTo(x, canvas.height - padding);
            ctx.stroke();
        }

        // Draw horizontal grid lines
        for (let y = padding + 0.5; y <= canvas.height - padding; y += tileSize) {
            ctx.beginPath();
            ctx.moveTo(padding + 0.5, y);
            ctx.lineTo(canvas.width - padding, y);
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
    const itemId = <?php echo json_encode($itemId); ?>;
    const item = game.objectData[itemId][0];

    // Update only the 'w' field with the new polygon data
    item.w = [...this.walkableData.polygon, ...this.polygonPoints];

    console.log('Updated objectData in real-time (preserving other fields):', game.objectData);
},



saveData: function(itemId) {
    var item = game.objectData[itemId][0];

    // Get the item name from the input field
    var itemName = document.getElementById('item_name').value.trim();
    item.n = itemName;

    // Get the walkable polygon data from the points and combine it with the existing polygon data
    item.w = this.walkableData.polygon.concat(this.polygonPoints);
    
    console.log("Walkable data saved:", item.w);

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
        url: 'plugins/renadmin/tileset/ajax/save_item.php',
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
        console.log("Unmounting Item Editor plugin");
        var pluginElement = document.querySelector('[data-window="tileset_item_editor_window"]');
        if (pluginElement) {
            pluginElement.remove();
        }
    }
      };

      tileset_item_editor_window.start(<?php echo json_encode($itemId); ?>);
    </script>
<?php
}
?>
