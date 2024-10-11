<div data-window='click_menu_window' data-close="false">
    <div id="rightClickMenu" class="bg-black bg-opacity-70 border border-red-700 text-white rounded shadow-lg p-2 absolute z-50 hidden" style="overflow-y: auto; max-height: 200px;">
        <ul id="clickMenuItems">
            <li id="moveItemOption" class="px-4 py-2 cursor-pointer hidden" onclick="click_menu_window.hideMenus(); editor.startMovingItem(game.selectedObjects[0]);">Move Item</li>
            <li id="pickUpItemOption" class="px-4 py-2 cursor-pointer block" onclick="click_menu_window.hideMenus(); editor.pickUpSelectedItems();">Pick Up</li>
        </ul>
    </div>

    <script>
var click_menu_window = {
    start: function() {
        document.addEventListener('contextmenu', this.disableDefaultContextMenu.bind(this));
        document.addEventListener('click', this.hideMenus.bind(this));
    },

    disableDefaultContextMenu: function(event) {
        event.preventDefault();
        this.showContextMenu(event.clientX, event.clientY, true);
    },

    showContextMenu: function(clientX, clientY, programmatic = false) {
        const rect = game.canvas.getBoundingClientRect();
        const mouseX = (clientX - rect.left) / game.zoomLevel + camera.cameraX;
        const mouseY = (clientY - rect.top) / game.zoomLevel + camera.cameraY;
        const gridX = Math.floor(mouseX / 16);
        const gridY = Math.floor(mouseY / 16);

        const selectedObject = utils.findObjectAt(mouseX, mouseY);
        const isTileWalkable = collision.isTileWalkable(gridX, gridY);

        const contextMenu = document.getElementById('rightClickMenu');
        const clickMenuItems = document.getElementById('clickMenuItems');

        // Clear previous menu items
        clickMenuItems.innerHTML = '';

        if (isTileWalkable) {
            const walkHereOption = document.createElement('li');
            walkHereOption.textContent = 'Walk Here';
            walkHereOption.classList.add('px-4', 'py-2', 'cursor-pointer');
            walkHereOption.onclick = function() {
                game.mainSprite.walkToClickedTile(gridX, gridY);
                click_menu_window.hideMenus();
            };
            clickMenuItems.appendChild(walkHereOption);
        }

        if (selectedObject) {


            const pickUpItemOption = document.createElement('li');
            pickUpItemOption.textContent = game.selectedObjects.length > 1 ? 'Pick Up Items' : 'Pick Up Item';
            pickUpItemOption.classList.add('px-4', 'py-2', 'cursor-pointer');
            pickUpItemOption.onclick = function() {
                editor.pickUpSelectedItems();
                click_menu_window.hideMenus();
            };
            clickMenuItems.appendChild(pickUpItemOption);

            // Retrieve the object data using selectedObject.id
            const objectData = game.objectData[selectedObject.id];

            if (objectData && objectData[0] && objectData[0].n) {
                const tileName = objectData[0].n;
                const editTileOption = document.createElement('li');
                editTileOption.textContent = `Edit ${tileName}`;
                editTileOption.classList.add('px-4', 'py-2', 'cursor-pointer');
                editTileOption.onclick = function() {
                    click_menu_window.editItem(selectedObject);
                    click_menu_window.hideMenus();
                };
                clickMenuItems.appendChild(editTileOption);
            }
        }

        contextMenu.classList.remove('hidden');
        contextMenu.style.left = `${clientX}px`;
        contextMenu.style.top = `${clientY}px`;

        if (programmatic) {
            console.log("Context menu shown programmatically");
        }
    },

    hideMenus: function() {
        document.getElementById('rightClickMenu').classList.add('hidden');
    },

    editItem: function(selectedObject) {
        if (selectedObject) {
            const uniqueId = selectedObject.id;
            modal.load({ id: 'tileset_item_editor_window', url: `renadmin/tileset/items.php?id=${uniqueId}`, name: 'Item Editor', drag: true, reload: true });
        }
    }
};

click_menu_window.start();

</script>

</div>
