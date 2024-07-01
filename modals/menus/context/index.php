<div data-window='context_menu_window' data-close="false">
    <div id="rightClickMenu" class="custom-menu shadow" style="overflow-y: auto; max-height: 200px;">
        <ul id="contextMenuItems">
            <li id="walkHereOption" onclick="game.sprites[game.playerid].walkToClickedTile(game.x, game.y);">Walk Here</li>
            <li id="moveItemOption" style="display: none;" onclick="editor.startMovingItem(game.selectedObject);">Move Item</li>
        </ul>
    </div>
    <style>
        .custom-menu {
            display: none;
            position: fixed;
            z-index: 5;
            background: rgba(0,0,0,0.9);
            color: #FFF;
            border: 1px solid #000;
            border-radius: 5px;
            font-size: 16px;
        }

        .custom-menu ul {
            list-style: none;
            margin: 0;
            padding: 0;
        }

        .custom-menu li {
            padding: 8px;
            cursor: pointer;
        }

        .custom-menu li:hover {
            background-color: #0450ad;
            border-radius: 3px;
        }
    </style>

    <script>
        var context_menu_window = {

            start: function() {
                document.addEventListener('contextmenu', this.contextMenu);
                document.addEventListener('click', this.hideMenu);
            },

            unmount: function() {
                document.removeEventListener('contextmenu', this.contextMenu);
                document.removeEventListener('click', this.hideMenu);
            },

            contextMenu: function(event) {
    event.preventDefault();
    const menu = document.getElementById('rightClickMenu');
    const moveItemOption = document.getElementById('moveItemOption');
    const selectedObject = game.selectedObject;

    // Calculate the click position relative to the entire document
    const menuX = event.clientX + window.scrollX;
    const menuY = event.clientY + window.scrollY;

    game.handleCanvasClick(event);

    // Update the selectedObject after handling the canvas click
    const updatedSelectedObject = game.selectedObject;

    // Show or hide the "Move Item" option based on the updated selectedObject
    if (updatedSelectedObject) {
        moveItemOption.style.display = 'block';
        moveItemOption.textContent = `Move ${updatedSelectedObject.id}`;
    } else {
        moveItemOption.style.display = 'none';
    }

    // Position and display the menu
    menu.style.display = 'block';

    // Get the size of the menu
    const menuWidth = menu.offsetWidth;
    const menuHeight = menu.offsetHeight;

    // Get the viewport dimensions
    const screenWidth = window.innerWidth;
    const screenHeight = window.innerHeight;

    // Adjust the position if the menu goes off the right or bottom edge of the screen
    const adjustedX = (menuX + menuWidth > screenWidth) ? (screenWidth - menuWidth + window.scrollX) : menuX;
    const adjustedY = (menuY + menuHeight > screenHeight) ? (screenHeight - menuHeight + window.scrollY) : menuY;

    // Set the adjusted position
    menu.style.left = `${adjustedX}px`;
    menu.style.top = `${adjustedY}px`;
},

            hideMenu: function() {
                const menu = document.getElementById('rightClickMenu');
                menu.style.display = 'none';
            }
        };

        context_menu_window.start();
    </script>
</div>