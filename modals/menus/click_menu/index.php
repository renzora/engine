<div data-window='click_menu_window' data-close="false">
    <div id="rightClickMenu" class="dark-menu shadow" style="overflow-y: auto; max-height: 200px;">
        <ul id="clickMenuItems">
            <li id="walkHereOption" onclick="click_menu_window.hideMenus(); game.sprites[game.playerid].walkToClickedTile(game.x, game.y);">Walk Here</li>
            <li id="moveItemOption" style="display: none;" onclick="click_menu_window.hideMenus(); editor.startMovingItem(game.selectedObjects[0]);">Move Item</li>
        </ul>
    </div>

    <div id="itemMenu" class="item-menu dark-menu shadow" style="display: none;">
        <ul id="itemMenuItems" style="display: flex; flex-direction: row; padding: 0; margin: 0; list-style: none;">
            <li onclick="click_menu_window.hideMenus(); editor.startMovingItem(game.selectedObjects[0]);" style="margin-right: 10px; cursor: pointer;">Move</li>
            <li onclick="click_menu_window.hideMenus(); game.pickUpItem(game.selectedObjects[0]);" style="margin-right: 10px; cursor: pointer;">Pick Up</li>
            <li onclick="click_menu_window.hideMenus(); game.rotateItem(game.selectedObjects[0]);" style="cursor: pointer;">Rotate</li>
        </ul>
    </div>

    <style>
        .item-menu {
            position: absolute;
            z-index: 1000;
        }

        .dark-menu {
            background-color: #333;
            color: #fff;
            border-radius: 5px;
            padding: 5px;
        }

        .dark-menu li {
            padding: 5px 10px;
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

            },

            unmount: function() {

            },
        };

        click_menu_window.start();
    </script>
</div>
