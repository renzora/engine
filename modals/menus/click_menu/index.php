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
            },

            showContextMenu: function(clientX, clientY, programmatic = false) {
                const rect = game.canvas.getBoundingClientRect();
                const mouseX = (clientX - rect.left) / game.zoomLevel + game.cameraX;
                const mouseY = (clientY - rect.top) / game.zoomLevel + game.cameraY;
                const selectedObject = game.findObjectAt(mouseX, mouseY);

                if (selectedObject && !game.selectedObjects.includes(selectedObject)) {
                    game.selectedObjects.push(selectedObject);
                    if (!game.selectedCache.some(cache => cache.id === selectedObject.id)) {
                        game.selectedCache.push({ id: selectedObject.id, image: game.drawAndOutlineObjectImage(selectedObject) });
                    }
                }

                const contextMenu = document.getElementById('rightClickMenu');
                if (game.selectedObjects.length > 1) {
                    document.getElementById('pickUpItemOption').textContent = 'Pick Up Items';
                } else {
                    document.getElementById('pickUpItemOption').textContent = 'Pick Up Item';
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
            }
        };

        click_menu_window.start();
    </script>
</div>
