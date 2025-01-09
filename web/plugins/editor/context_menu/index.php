<div data-close="false">
  <!-- Main Context Menu Container -->
  <div
    id="editor_context_menu_window"
    class="absolute z-50 hidden flex flex-col items-start space-y-2">
    
    <!-- Toolbar Buttons Container -->
    <div
      id="editor_toolbar_buttons"
      class="bg-black text-white rounded-lg shadow-lg p-2 flex gap-2 overflow-x-auto"
      style="margin-bottom: 10px;">
      <button type="button" id="select_button" class="mode-button shadow flex items-center justify-center hover:bg-gray-700 hover:rounded transition hint--top" onclick="edit_mode_window.changeMode('select')" aria-label="Select">
        <div class="ui_icon ui_select"></div>
      </button>
      <button type="button" id="brush_button" class="mode-button shadow flex items-center justify-center hover:bg-gray-700 hover:rounded transition" onclick="edit_mode_window.changeMode('brush')">
        <div class="ui_icon ui_brush"></div>
      </button>
      <button type="button" id="zoom_button" class="mode-button shadow flex items-center justify-center hover:bg-gray-700 hover:rounded transition" onclick="edit_mode_window.changeMode('zoom')">
        <div class="ui_icon ui_magnify"></div>
      </button>
      <button type="button" id="pan_button" class="mode-button shadow flex items-center justify-center hover:bg-gray-700 hover:rounded transition" onclick="edit_mode_window.changeMode('pan')">
        <div class="ui_icon ui_pan"></div>
      </button>
      <button type="button" id="lasso_button" class="mode-button shadow flex items-center justify-center hover:bg-gray-700 hover:rounded transition" onclick="edit_mode_window.changeMode('lasso')">
        <div class="ui_icon ui_lasso"></div>
      </button>
      <button type="button" id="move_button" class="mode-button shadow flex items-center justify-center hover:bg-gray-700 hover:rounded transition" onclick="edit_mode_window.changeMode('move')">
        <div class="ui_icon ui_move"></div>
      </button>
      <button type="button" id="save_button" class="mode-button shadow flex items-center justify-center hover:bg-gray-700 hover:rounded transition" onclick="edit_mode_window.saveRoomData()">
        <div class="ui_icon ui_save"></div>
      </button>
      <button type="button" id="close_button" class="mode-button shadow flex items-center justify-center hover:bg-gray-700 hover:rounded transition" onclick="plugin.close('edit_mode_window');">
        <div class="ui_icon ui_close"></div>
      </button>
    </div>

    <!-- Context Menu Items Container -->
    <div
      id="editor_menu_container"
      class="bg-black text-white rounded-lg shadow-lg"
      style="width: 220px; align-self: flex-start;">
      <ul id="editor_menuItems" class="space-y-1"></ul>
    </div>
  </div>
</div>
</div>



  <script>
window[id] = {
      id: id,
      isGridEnabled: true,
      isSnapEnabled: true,
      isNightfilterEnabled: false,

      contextMenuElement: null,
      menuItemsElement: null,
      contextmenuHandler: null,
      clickHandler: null,
      initialClickX: null,
      initialClickY: null,
      menuItemsConfig: [],

      start: function () {
    this.contextMenuElement = document.getElementById('editor_context_menu_window');
    this.menuItemsElement = document.getElementById('editor_menuItems');

    this.contextmenuHandler = (event) => {
    event.preventDefault(); // Always prevent default browser context menu

    this.initialClickX = event.clientX;
    this.initialClickY = event.clientY;

    this.populateMenuItems(event.clientX, event.clientY);

    if (this.menuItemsConfig.length > 0) {
        // Show context menu only if there are valid menu items
        ui.contextMenu.showContextMenu(
            this.contextMenuElement,
            this.menuItemsElement,
            this.menuItemsConfig,
            event.clientX,
            event.clientY
        );
    } else {
        // Hide menu if no valid options
        this.contextMenuElement.classList.add('hidden');
    }
};

    this.clickHandler = (e) => {
      ui.contextMenu.hideMenus(e, this.contextMenuElement);
    };

    document.addEventListener('contextmenu', this.contextmenuHandler);
    document.addEventListener('click', this.clickHandler);
  },

      unmount: function () {
        // remove event listeners
        document.removeEventListener('contextmenu', this.contextmenuHandler);
        document.removeEventListener('click', this.clickHandler);

        // hide the menu if open
        this.contextMenuElement.classList.add('hidden');
        console.log("Editor context menu unmounted, all events removed.");
      },

      populateMenuItems: function (clientX, clientY) {
  const clickedObject = this.getSelectedObject(clientX, clientY);

  console.log('Clicked Object:', clickedObject); // Debug
  if (clickedObject) {
    console.log('Object ID:', clickedObject.id); // Debug
  }

  if (clickedObject && edit_mode_window.selectedObjects.includes(clickedObject)) {
    const objectId = clickedObject.id;

    // Fetch object details from game.objectData
    console.log('Object ID in game.objectData:', objectId, game.objectData[objectId]); // Debug
    const objectDetails = game.objectData[objectId];

    // Use the name or fallback
    const objectName = objectDetails && objectDetails[0]?.n
      ? objectDetails[0].n
      : 'Unnamed Object';

    // Hide the buttons for the object menu
    document.getElementById('editor_toolbar_buttons').classList.add('hidden');

    this.menuItemsConfig = [
      {
        label: `${objectName}`,
        subMenu: [
          { label: "Properties", callback: () => this.editLighting(clickedObject) },
          { label: "Lighting", callback: () => this.editLighting(clickedObject) },
          { label: "Scripting", callback: () => this.editScripting(clickedObject) },
          { label: "Collision", callback: () => this.editCollision(clickedObject) },
          { label: "Stacking", callback: () => this.editCollision(clickedObject) },
          { label: "Animation", callback: () => this.editLighting(clickedObject) },
          { label: "Effects", callback: () => this.editLighting(clickedObject) },
        ]
      },
      {
        label: `move to layer`,
        subMenu: [
          { label: "Properties", callback: () => this.editLighting(clickedObject) },
          { label: "Lighting", callback: () => this.editLighting(clickedObject) },
          { label: "Scripting", callback: () => this.editScripting(clickedObject) },
          { label: "Collision", callback: () => this.editCollision(clickedObject) },
          { label: "Stacking", callback: () => this.editCollision(clickedObject) },
          { label: "Animation", callback: () => this.editLighting(clickedObject) },
          { label: "Effects", callback: () => this.editLighting(clickedObject) },
        ]
      },
      {
        label: "Rotate",
        callback: () => this.rotateObject(clickedObject)
      },
      {
        label: "Delete",
        callback: () => this.rotateObject(clickedObject)
      }
    ];
  } else {
    // Show the buttons for other menus
    document.getElementById('editor_toolbar_buttons').classList.remove('hidden');

    this.menuItemsConfig = [
      { label: "Scene", subMenu: [
        { label: "Properties", callback: function (x, y) { editor_context_menu_window.openSceneProperties(x, y) } },
          { label: "viewport size", callback: function (x, y) { editor_context_menu_window.openSceneProperties(x, y) } },
          { label: "Background", callback: function (x, y) { editor_context_menu_window.openSceneProperties(x, y) } },
          { label: "Weather settings", callback: function (x, y) { editor_context_menu_window.openSceneProperties(x, y) } }
        ]
      },
      { label: "Sprites", subMenu: [
          { label: "Add sprite", callback: function (x, y) { editor_context_menu_window.spriteSetStartingPosition(x, y) } },
          { label: "Set Starting Position", callback: function (x, y) { editor_context_menu_window.spriteSetStartingPosition(x, y) } },
          { label: "Allow Movement", type: "checkbox", id: "sprite_active_checkbox", initialValue: true, callback: function (checked) { editor_context_menu_window.spriteActiveToggle(checked) } }
        ]
      },
      { label: "Tools", subMenu: [
          { label: "Terrain Editor", callback: function () { editor_context_menu_window.openTerrainEditor() } },
          { label: "Tileset Manager", callback: function () { editor_context_menu_window.openTilesetManager() } },
          { label: "Sprites Manager", callback: function () { editor_context_menu_window.openTerrainEditor() } },
          { label: "Scene Debugging", callback: function () { editor_context_menu_window.openTerrainEditor() } }
        ]
      },
      { label: "Editor", subMenu: [
          { label: "Grid", type: "checkbox", id: "toggle_grid_checkbox", initialValue: true, callback: function (checked) { editor_context_menu_window.utilsToggleGrid(checked) } },
          { label: "Day/Time", callback: function () { editor_context_menu_window.openTerrainEditor() } },
          { label: "Manual Camera", type: "checkbox", id: "manual_camera_checkbox", initialValue: true, callback: function (checked) { editor_context_menu_window.cameraManualToggle(checked) } }
        ]
      }
    ];
  }
},

  getSelectedObject: function (clientX, clientY) {
    const rect = game.canvas.getBoundingClientRect();
    const mouseX = (clientX - rect.left) / game.zoomLevel + camera.cameraX;
    const mouseY = (clientY - rect.top) / game.zoomLevel + camera.cameraY;

    return edit_mode_window.selectedObjects.find((item) => {
        const minX = Math.min(...item.x) * 16;
        const minY = Math.min(...item.y) * 16;
        const maxX = Math.max(...item.x) * 16 + 16;
        const maxY = Math.max(...item.y) * 16 + 16;

        // Ensure object has a valid ID
        if (!item.id) {
            console.warn('Object missing ID:', item); // Debug
            return false;
        }

        return mouseX >= minX && mouseX <= maxX && mouseY >= minY && mouseY <= maxY;
    });
},


      editObject: function (selectedObject) {
        if (!selectedObject) return;
        const uniqueId = selectedObject.id;
        plugin.load({
          id: 'tileset_item_editor_window',
          url: `editor/tileset_manager/items.php?id=${uniqueId}`,
          name: 'Item Editor',
          drag: true,
          reload: true,
        });
        this.contextMenuElement.classList.add('hidden');
      },

      // Custom editor callbacks:
      cameraSnapToggle: function (checked) {
        console.log("cameraSnapToggle:", checked);
      },

      cameraManualToggle: function (checked) {
        if (checked) {
          camera.lerpEnabled = false;
          camera.manual = true;
          console.log("Manual Camera Enabled");
        } else {
          camera.lerpEnabled = true;
          camera.manual = false;
          console.log("Manual Camera Disabled");
        }
      },

      spriteActiveToggle: function (checked) {
        if (checked) {
          camera.lerpEnabled = false;
          camera.manual = true;
          game.allowControls = true;
          console.log("Sprite Enabled");
        } else {
          camera.lerpEnabled = true;
          camera.manual = false;
          game.allowControls = false;
          console.log("Sprite Disabled");
        }
      },

      spriteSetStartingPosition: function () {
      if (this.initialClickX === null || this.initialClickY === null) {
        console.error("Initial click position is not set.");
        return;
      }

      const rect = game.canvas.getBoundingClientRect();
      const mouseX = (this.initialClickX - rect.left) / game.zoomLevel + camera.cameraX;
      const mouseY = (this.initialClickY - rect.top) / game.zoomLevel + camera.cameraY;
      const gridX = Math.floor(mouseX / 16);
      const gridY = Math.floor(mouseY / 16);

      this.updateStartingPosition(gridX, gridY);
      this.contextMenuElement.classList.add('hidden');

      // Reset initial click position
      this.initialClickX = null;
      this.initialClickY = null;
    },

    updateStartingPosition: function (gx, gy) {
      const sceneId = game.sceneid;
      if (!sceneId) {
        alert('No scene loaded!');
        return;
      }
      fetch('/plugins/editor/context_menu/ajax/setSpritePosition.php', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ sceneId: sceneId, startingX: gx, startingY: gy }),
      })
        .then((response) => response.json())
        .then((data) => {
          if (data.error) {
            alert("Error: " + data.message);
          } else {
            game.roomData.startingX = gx;
            game.roomData.startingY = gy;
            const playerSprite = game.sprites[game.playerid];
            if (playerSprite) {
              game.x = gx * 16;
              game.y = gy * 16;
              playerSprite.x = gx * 16;
              playerSprite.y = gy * 16;
            }
          }
        })
        .catch((error) => {
          console.error(error);
          alert('An error occurred.');
        });
    },

      openSceneProperties: function (x, y) {
        plugin.load({
          id: 'editor_scene_properties_window',
          url: 'editor/scene_properties/index.php',
          name: 'Scene Properties',
          drag: true,
          reload: true,
        });
        this.contextMenuElement.classList.add('hidden');
      },

      openTerrainEditor: function () {
        console.log("Open Terrain Editor...");
        this.contextMenuElement.classList.add('hidden');
      },

      openTilesetManager: function () {
        plugin.load({
          id: 'tileset_window',
          url: 'editor/tileset_manager/index.php',
          drag: true,
          reload: false,
        });
      },

      advancedOption1: function () {
        console.log("Advanced Option 1 triggered");
      },

      nestedOption1: function () {
        console.log("Nested Option 1 triggered");
      },

      deepNestedOption1: function () {
        console.log("Deep Nested Option 1 triggered");
      },

      deepNestedOption2: function () {
        console.log("Deep Nested Option 2 triggered");
      },

      effectsConsoleToggle: function (checked) {
        console.log("Console toggled =>", checked);
      },

      effectsBrushSize: function (val) {
        console.log("Brush Size =>", val);
      },

      utilsToggleGrid: function (checked) {
        this.toggleGrid(checked);
      },

      toggleGrid: function (checked) {
        this.isGridEnabled = checked;
        if (checked) {
          console.log("Grid enabled.");
          this.render2dGrid();
        } else {
          console.log("Grid disabled.");
        }
      },

      render2dGrid: function () {
    if (!this.isGridEnabled) return;

    // Set grid line style to be dark and subtle
    game.ctx.strokeStyle = 'rgba(0, 0, 0, 0.1)';
    game.ctx.lineWidth = 1;

    // Draw vertical lines
    for (let x = 0; x < game.worldWidth; x += 16) {
        game.ctx.beginPath();
        game.ctx.moveTo(x, 0);
        game.ctx.lineTo(x, game.worldHeight);
        game.ctx.stroke();
    }

    // Draw horizontal lines
    for (let y = 0; y < game.worldHeight; y += 16) {
        game.ctx.beginPath();
        game.ctx.moveTo(0, y);
        game.ctx.lineTo(game.worldWidth, y);
        game.ctx.stroke();
    }

    // Draw a red border around the outer edge of the grid
    game.ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
    game.ctx.lineWidth = 2; // Make the border slightly thicker
    game.ctx.strokeRect(0, 0, game.worldWidth, game.worldHeight);
},

renderIsometricGrid: function () {
    if (!this.isGridEnabled) return;

    const tileWidth = 32;
    const tileHeight = 16;
    
    // Instead of halfWorldWidth / halfWorldHeight, use the full game.worldWidth / game.worldHeight
    // so we can overshoot and ensure the diagonal lines fill the entire canvas
    const maxY = game.worldHeight;
    const minY = -game.worldHeight;

    // Light grid lines
    game.ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
    game.ctx.lineWidth = 1;

    // Draw lines slanted to the right
    for (let y = minY; y <= maxY; y += tileHeight) {
        game.ctx.beginPath();
        // Move from left edge down
        game.ctx.moveTo(0, game.worldHeight / 2 + y);
        // Draw to right edge, shifted up by half the width
        game.ctx.lineTo(game.worldWidth, game.worldHeight / 2 + y - game.worldWidth / 2);
        game.ctx.stroke();
    }

    // Draw lines slanted to the left
    for (let y = minY; y <= maxY; y += tileHeight) {
        game.ctx.beginPath();
        // Move from right edge down
        game.ctx.moveTo(game.worldWidth, game.worldHeight / 2 + y);
        // Draw to left edge, shifted up by half the width
        game.ctx.lineTo(0, game.worldHeight / 2 + y - game.worldWidth / 2);
        game.ctx.stroke();
    }

    // Border (optional)
    game.ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
    game.ctx.lineWidth = 2;
    game.ctx.strokeRect(0, 0, game.worldWidth, game.worldHeight);
},

      utilsToggleNightFilter: function (checked) {
        this.isNightfilterEnabled = checked;
        if (checked) {
          lighting.nightFilterActive = true;
          utils.gameTime.hours = 0;
          console.log("Night filter enabled.");
        } else {
          lighting.nightFilterActive = false;
          lighting.timeBasedUpdatesEnabled = false;
          utils.gameTime.hours = 12;
          console.log("Night filter disabled.");
        }
      },
    };

  </script>
