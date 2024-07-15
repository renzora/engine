<div data-window='click_menu_window' data-close="false">
    <div id="rightClickMenu" class="dark-menu shadow" style="overflow-y: auto; max-height: 200px;">
        <ul id="clickMenuItems">
            <li id="walkHereOption" onclick="click_menu_window.hideMenus(); game.sprites[game.playerid].walkToClickedTile(game.x, game.y);">Walk Here</li>
            <li id="moveItemOption" style="display: none;" onclick="click_menu_window.hideMenus(); editor.startMovingItem(game.selectedObjects[0]);">Move Item</li>
            <li id="pickUpItemOption" style="display: block;" onclick="click_menu_window.hideMenus(); editor.pickUpSelectedItems();">Pick Up</li>
        </ul>
    </div>

    <style>
        .dark-menu {
            background-color: #333;
            color: #fff;
            border-radius: 5px;
            padding: 5px;
            position: absolute;
            z-index: 1000;
            display: none;
        }

        .dark-menu li {
            padding: 5px 10px;
            cursor: pointer;
        }

        .dark-menu li:hover {
            background-color: #555;
        }

        .shadow {
            box-shadow: 0 4px 8px rgba(0, 0, 0, 0.3);
        }
    </style>

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
        contextMenu.style.display = 'block';
        contextMenu.style.left = `${clientX}px`;
        contextMenu.style.top = `${clientY}px`;

        if (programmatic) {
            console.log("Context menu shown programmatically");
        }
    },

    hideMenus: function() {
        document.getElementById('rightClickMenu').style.display = 'none';
    }
};

click_menu_window.start();

    </script>
</div>
