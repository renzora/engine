<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
    ?>
    <div data-window='tileset_manager_window' class='window window_bg' style='width: 800px; height: 540px; background: #bba229;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>Tileset Manager</div>
    </div>
    
    <div class='clearfix'></div>
    
    <div class='relative' style='height: calc(100% - 40px); overflow-y: auto;'> <!-- Adjusted height and added overflow-y -->
      <div class='container text-light window_body p-2' style='height: 100%;'>
        <div id="tileset_manager_tabs">
          <div id="tabs" class="flex border-b border-gray-300">
            <button class="tab text-gray-800 p-3" data-tab="tab1">Upload</button>
            <button class="tab text-gray-800 p-3" data-tab="tab2">Items</button>
            <button class="tab text-gray-800 p-3" data-tab="tab3">Search Tiles</button>
          </div>
  
          <div class="tab-content p-4 hidden" data-tab-content="tab1">
            <form id="uploadTileForm">
              <input type="file" id="uploadImage" accept="image/*" class="mb-2"/>
              <canvas id="tilesetCanvas" style="border: 1px solid #ede8d6; image-rendering: pixelated; max-width: 100%;"></canvas>
            </form>
          </div>
  
          <div class="tab-content p-4 hidden" data-tab-content="tab2">
            <form id="editTileForm">
              <label for="editTileName">Tile Name:</label>
              <input type="text" id="editTileName" name="editTileName">
              
              <label for="editTileCategory">Category:</label>
              <input type="text" id="editTileCategory" name="editTileCategory">
              
              <label for="editTileImage">Tile Image:</label>
              <input type="file" id="editTileImage" name="editTileImage" accept="image/*">
              
              <button type="submit">Edit Tile</button>
            </form>
          </div>
  
          <div class="tab-content p-4 hidden" data-tab-content="tab3">
            <input type="text" id="searchTilesInput" placeholder="Search Tiles...">
            <div id="tilesList"></div>
          </div>
        </div>
      </div>
    </div>
  
    <div class='resize-handle'></div>
  </div>
  
  <script>
    var tileset_manager_window = {
      start: function() {
        this.setupElements();
        this.setupEventListeners();
        ui.initTabs('tileset_manager_tabs', 'tab1');
      },
      
      setupElements: function() {
        this.uploadImage = document.getElementById('uploadImage');
        this.canvas = document.getElementById('tilesetCanvas');
        this.context = this.canvas.getContext('2d');
        
        // Disable image smoothing to preserve pixel art quality
        this.context.imageSmoothingEnabled = false;
  
        this.tileSize = 16;
        this.dragging = false;
        this.selectedTiles = [];
        this.dragThreshold = 5; // Threshold to differentiate between click and drag
      },
  
      setupEventListeners: function() {
        const self = this;
        
        this.canvas.addEventListener('mousedown', function(event) {
          self.handleMouseDown(event);
        });
  
        this.canvas.addEventListener('mousemove', function(event) {
          self.handleMouseMove(event);
        });
  
        this.canvas.addEventListener('mouseup', function(event) {
          self.handleMouseUp(event);
        });
  
        this.canvas.addEventListener('mouseleave', function() {
          self.handleMouseLeave();
        });
  
        this.uploadImage.addEventListener('change', function(event) {
          self.handleImageUpload(event);
        });
  
        document.getElementById('searchTilesInput').addEventListener('input', function() {
          self.handleSearch(this.value);
        });
  
        document.getElementById('editTileForm').addEventListener('submit', function(event) {
          event.preventDefault();
          self.handleEditTile();
        });

        // Drag and drop events for canvas
        this.canvas.addEventListener('dragover', function(event) {
          event.preventDefault();
          event.dataTransfer.dropEffect = 'copy';
        });

        this.canvas.addEventListener('drop', function(event) {
          event.preventDefault();
          self.handleDrop(event);
        });
      },
  
      drawGrid: function() {
        const width = this.canvas.width;
        const height = this.canvas.height;
  
        this.context.strokeStyle = 'rgba(0, 0, 0, 0.2)'; // Opaque black lines
        this.context.lineWidth = 0.5;
  
        // Vertical lines
        for (let x = 0; x <= width; x += this.tileSize) {
          this.context.beginPath();
          this.context.moveTo(x, 0);
          this.context.lineTo(x, height);
          this.context.stroke();
        }
  
        // Horizontal lines
        for (let y = 0; y <= height; y += this.tileSize) {
          this.context.beginPath();
          this.context.moveTo(0, y);
          this.context.lineTo(width, y);
          this.context.stroke();
        }
      },
  
      highlightSelection: function() {
        this.context.fillStyle = 'rgba(255, 0, 0, 0.5)';
        for (let tile of this.selectedTiles) {
          this.context.fillRect(tile.col * this.tileSize, tile.row * this.tileSize, this.tileSize, this.tileSize);
        }
      },
  
      redrawImageAndGrid: function() {
        this.context.clearRect(0, 0, this.canvas.width, this.canvas.height);
        this.context.drawImage(this.img, 0, 0, this.canvas.width, this.canvas.height);
        this.drawGrid();
        this.highlightSelection();
      },
  
      getMousePos: function(event) {
        const rect = this.canvas.getBoundingClientRect();
        return {
          x: Math.floor((event.clientX - rect.left) * (this.canvas.width / rect.width)),
          y: Math.floor((event.clientY - rect.top) * (this.canvas.height / rect.height))
        };
      },
  
      handleMouseDown: function(event) {
        const pos = this.getMousePos(event);
        this.startX = pos.x;
        this.startY = pos.y;
        this.dragging = true;
        this.selectedTiles = []; // Clear previous selection
      },
  
      handleMouseMove: function(event) {
        if (this.dragging) {
          const pos = this.getMousePos(event);
          this.endX = pos.x;
          this.endY = pos.y;
          this.redrawImageAndGrid();
          const startRow = Math.floor(this.startY / this.tileSize);
          const startCol = Math.floor(this.startX / this.tileSize);
          const endRow = Math.floor(this.endY / this.tileSize);
          const endCol = Math.floor(this.endX / this.tileSize);
  
          this.context.fillStyle = 'rgba(255, 0, 0, 0.5)';
          for (let row = Math.min(startRow, endRow); row <= Math.max(startRow, endRow); row++) {
            for (let col = Math.min(startCol, endCol); col <= Math.max(startCol, endCol); col++) {
              this.context.fillRect(col * this.tileSize, row * this.tileSize, this.tileSize, this.tileSize);
            }
          }
        }
      },
  
      handleMouseUp: function(event) {
        if (this.dragging) {
          this.dragging = false;
          const pos = this.getMousePos(event);
          this.endX = pos.x;
          this.endY = pos.y;
          if (Math.abs(this.endX - this.startX) < this.dragThreshold && Math.abs(this.endY - this.startY) < this.dragThreshold) {
            // Click action
            const row = Math.floor(this.endY / this.tileSize);
            const col = Math.floor(this.endX / this.tileSize);
            this.selectedTiles = [{row: row, col: col}]; // Clear previous selection and add the clicked tile
          } else {
            // Drag action
            const startRow = Math.floor(this.startY / this.tileSize);
            const startCol = Math.floor(this.startX / this.tileSize);
            const endRow = Math.floor(this.endY / this.tileSize);
            const endCol = Math.floor(this.endX / this.tileSize);
  
            for (let row = Math.min(startRow, endRow); row <= Math.max(startRow, endRow); row++) {
              for (let col = Math.min(startCol, endCol); col <= Math.max(startCol, endCol); col++) {
                this.selectedTiles.push({row: row, col: col});
              }
            }
          }
          this.redrawImageAndGrid();
        }
      },
  
      handleMouseLeave: function() {
        this.dragging = false;
        this.redrawImageAndGrid();
      },
  
      handleImageUpload: function(event) {
        const self = this;
        const file = event.target.files[0];
        if (file) {
          const reader = new FileReader();
          reader.onload = function(e) {
            self.img = new Image();
            self.img.onload = function() {
              // Calculate canvas dimensions to be multiples of 16
              const aspectRatio = self.img.width / self.img.height;
              const maxCanvasWidth = 800;
              let canvasWidth = Math.min(maxCanvasWidth, self.img.width);
              let canvasHeight = canvasWidth / aspectRatio;
  
              // Ensure the dimensions are multiples of 16
              canvasWidth = Math.floor(canvasWidth / self.tileSize) * self.tileSize;
              canvasHeight = Math.floor(canvasHeight / self.tileSize) * self.tileSize;
  
              self.canvas.width = canvasWidth;
              self.canvas.height = canvasHeight;
  
              self.context.clearRect(0, 0, self.canvas.width, self.canvas.height);
              self.context.drawImage(self.img, 0, 0, self.canvas.width, self.canvas.height);
  
              // Draw the grid on top of the image
              self.drawGrid();
            }
            self.img.src = e.target.result;
          }
          reader.readAsDataURL(file);
        }
      },

      handleDrop: function(event) {
        const self = this;
        const file = event.dataTransfer.files[0];
        if (file && file.type.startsWith('image/')) {
          const reader = new FileReader();
          reader.onload = function(e) {
            self.img = new Image();
            self.img.onload = function() {
              // Calculate canvas dimensions to be multiples of 16
              const aspectRatio = self.img.width / self.img.height;
              const maxCanvasWidth = 800;
              let canvasWidth = Math.min(maxCanvasWidth, self.img.width);
              let canvasHeight = canvasWidth / aspectRatio;
  
              // Ensure the dimensions are multiples of 16
              canvasWidth = Math.floor(canvasWidth / self.tileSize) * self.tileSize;
              canvasHeight = Math.floor(canvasHeight / self.tileSize) * self.tileSize;
  
              self.canvas.width = canvasWidth;
              self.canvas.height = canvasHeight;
  
              self.context.clearRect(0, 0, self.canvas.width, self.canvas.height);
              self.context.drawImage(self.img, 0, 0, self.canvas.width, self.canvas.height);
  
              // Draw the grid on top of the image
              self.drawGrid();
            }
            self.img.src = e.target.result;
          }
          reader.readAsDataURL(file);
        }
      },
  
      handleSearch: function(query) {
        // Implement search functionality to filter tiles
        console.log('Searching for:', query);
      },
  
      handleEditTile: function() {
        // Implement edit tile functionality
        console.log('Editing tile...');
      },
  
      unmount: function() {
        this.canvas.removeEventListener('mousedown', this.mouseDownHandler);
        this.canvas.removeEventListener('mousemove', this.mouseMoveHandler);
        this.canvas.removeEventListener('mouseup', this.mouseUpHandler);
        this.canvas.removeEventListener('mouseleave', this.mouseLeaveHandler);
        this.uploadImage.removeEventListener('change', this.imageUploadHandler);
  
        // Clean up other elements if needed
        this.context.clearRect(0, 0, this.canvas.width, this.canvas.height);
        this.uploadImage.value = '';
        this.selectedTiles = [];
      }
    }
    tileset_manager_window.start();
  </script>
  
  <?php
  }
  ?>
