<div data-window="context_menu_window" data-close="false">
  <div
    id="context_menu_window"
    class="bg-black text-white rounded-lg shadow-lg absolute z-50 hidden"
    style="max-height: 400px; min-width: 200px;"
  >
    <ul id="menuItems" class="space-y-1"></ul>
  </div>

  <script>
    var context_menu_window = {
      contextMenuElement: null,
      menuItemsElement: null,
      contextmenuHandler: null,
      clickHandler: null,
      menuItemsConfig: [],

      start: function () {
        this.contextMenuElement = document.getElementById('context_menu_window');
        this.menuItemsElement = document.getElementById('menuItems');

        // Create bound handlers so we can remove them in unmount
        this.contextmenuHandler = (event) => {
          ui.contextMenu.disableDefaultContextMenu(event, (x, y) => {
            this.populateMenuItems(x, y);
            ui.contextMenu.showContextMenu(
              this.contextMenuElement,
              this.menuItemsElement,
              this.menuItemsConfig,
              x,
              y
            );
          });
        };

        this.clickHandler = (event) => {
          ui.contextMenu.hideMenus(event, this.contextMenuElement);
        };

        document.addEventListener('contextmenu', this.contextmenuHandler);
        document.addEventListener('click', this.clickHandler);
      },

      unmount: function () {
        document.removeEventListener('contextmenu', this.contextmenuHandler);
        document.removeEventListener('click', this.clickHandler);
        this.contextMenuElement.classList.add('hidden');
        console.log('First context menu unmounted, events removed.');
      },

      populateMenuItems: function (clientX, clientY) {
        this.menuItemsConfig = [];

        const { mouseX, mouseY, gridX, gridY } = this.getMouseCoordinates(clientX, clientY);
        const selectedObject = utils.findObjectAt(mouseX, mouseY);
        const isTileWalkable = collision.isTileWalkable(gridX, gridY);

        if (isTileWalkable) {
          this.menuItemsConfig.push({
            label: 'Walk Here',
            callback: () => this.walkHere(gridX, gridY),
          });
        }

        if (selectedObject) {
          const objectData = game.objectData[selectedObject.id];
          if (objectData && objectData[0] && objectData[0].n) {
            const tileName = objectData[0].n;
            this.menuItemsConfig.push({
              label: `Edit ${tileName}`,
              callback: () => this.editTile(selectedObject),
            });
          }
        }
      },

      getMouseCoordinates: function (clientX, clientY) {
        const rect = game.canvas.getBoundingClientRect();
        const mouseX = (clientX - rect.left) / game.zoomLevel + camera.cameraX;
        const mouseY = (clientY - rect.top) / game.zoomLevel + camera.cameraY;
        const gridX = Math.floor(mouseX / 16);
        const gridY = Math.floor(mouseY / 16);
        return { mouseX, mouseY, gridX, gridY };
      },

      walkHere: function (gridX, gridY) {
        game.mainSprite.walkToClickedTile(gridX, gridY);
        this.contextMenuElement.classList.add('hidden');
      },

      editTile: function (selectedObject) {
        if (!selectedObject) return;
        const uniqueId = selectedObject.id;
        modal.load({
          id: 'tileset_item_editor_window',
          url: `renadmin/tileset/items.php?id=${uniqueId}`,
          name: 'Item Editor',
          drag: true,
          reload: true,
        });
        this.contextMenuElement.classList.add('hidden');
      },
    };

    context_menu_window.start();
  </script>
</div>
