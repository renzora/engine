<div data-window="editor_context_menu_window" data-close="false">
    <div id="contextMenu" class="bg-gray-800 border border-gray-700 text-white rounded-lg shadow-lg absolute z-50 hidden" style="overflow-y: auto; max-height: 200px;">
        <ul id="menuItems" class="space-y-1"></ul>
    </div>

    <script>
var editor_context_menu_window = {
    start: function() {
        document.addEventListener('contextmenu', this.disableDefaultContextMenu.bind(this));
        document.addEventListener('click', this.hideMenus.bind(this));
    },

    disableDefaultContextMenu: function(event) {
        event.preventDefault();
        this.showContextMenu(event.clientX, event.clientY);
    },

    showContextMenu: function(clientX, clientY) {
        const contextMenu = document.getElementById('contextMenu');
        const menuItems = document.getElementById('menuItems');

        // Clear previous menu items
        menuItems.innerHTML = '';

        // Add "Set Starting Position" menu item
        const setStartingPosition = document.createElement('li');
        setStartingPosition.textContent = 'Set Starting Position';
        setStartingPosition.classList.add('px-4', 'py-2', 'cursor-pointer', 'rounded', 'hover:bg-gray-700');
        setStartingPosition.onclick = () => {
            const rect = game.canvas.getBoundingClientRect();
            const mouseX = (clientX - rect.left) / game.zoomLevel + camera.cameraX;
            const mouseY = (clientY - rect.top) / game.zoomLevel + camera.cameraY;
            const gridX = Math.floor(mouseX / 16);
            const gridY = Math.floor(mouseY / 16);

            // API call to update the starting position
            editor_context_menu_window.updateStartingPosition(gridX, gridY);
            editor_context_menu_window.hideMenus();
        };
        menuItems.appendChild(setStartingPosition);

        // Position and display the context menu
        contextMenu.style.left = `${clientX}px`;
        contextMenu.style.top = `${clientY}px`;
        contextMenu.classList.remove('hidden');
    },

    hideMenus: function() {
        const contextMenu = document.getElementById('contextMenu');
        contextMenu.classList.add('hidden');
    },

    updateStartingPosition: function(gridX, gridY) {
        const sceneId = game.sceneid;
        if (!sceneId) {
            alert('No scene loaded!');
            return;
        }

        console.log(sceneId);

        fetch('/modals/editor/ajax/setSpritePosition.php', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                sceneId: sceneId,
                startingX: gridX,
                startingY: gridY
            }),
        })
        .then(response => response.json())
        .then(data => {
            if (data.error) {
                alert(`Error: ${data.message}`);
            } else {
                alert('Starting position updated successfully.');
                game.roomData.startingX = gridX;
                game.roomData.startingY = gridY;
            }
        })
        .catch(error => {
            console.error('Error updating starting position:', error);
            alert('An error occurred. Check the console for details.');
        });
    }
};

// Initialize the context menu
editor_context_menu_window.start();
    </script>
</div>
