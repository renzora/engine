<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config/db.php';
if ($auth) {
    $serverId = $_GET['id'] ?? null;
?>
  <div class='window window_bg' style='width: 330px; background: #bba229;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
    <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>Create New Scene</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2'>
        <div>
          <label for="scene_name" class="text-white">Scene Name:</label>
          <input type="text" id="scene_name" class="w-full p-2 mt-1 mb-4 border border-gray-300 rounded" placeholder="Enter scene name">
          <button id="create_scene_btn" class="green_button text-white font-bold py-2 px-3 rounded w-48 mt-2 shadow-md">Create Scene</button>
        </div>
      </div>
    </div>
    <div class='resize-handle'></div>
    </div>

    <script>
window[id] = {
    id: id,
    server: null,
    start: function() {
        document.getElementById('create_scene_btn').addEventListener('click', this.createScene.bind(this));
    },
    createScene: function() {
        var sceneName = document.getElementById('scene_name').value.trim();
        console.log('Scene name entered:', sceneName);
        console.log('Server ID for scene creation:', this.server); // Log server ID for debugging

        if (!sceneName) {
            sceneName = 'default scene';
            console.log('No scene name entered. Using default:', sceneName);
        }

        console.log('Sending request to create scene with name:', sceneName, ' and serverId:', this.serverId);
        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'plugins/editor/console/tabs/servers/ajax/scenes/createScene.php',
            data: JSON.stringify({ serverId: this.server, name: sceneName }),
            headers: {
                'Content-Type': 'application/json'
            },
            success: function(data) {
                console.log('Response from createScene.php:', data);
                if (data.message === 'success') {
                    console.log('Scene created successfully.');
                    plugin.close('scene_create_window');
                    ui_servers_tab_window.addSceneToList(data.scene, data.server_id); // Pass serverId from response
                    game.loadScene(data.scene.id); // Enter the newly created scene
                } else {
                    console.error('Error creating scene:', data.message, data.error);
                    alert('Error creating scene: ' + data.message + ' (' + data.error + ')');
                }
            }.bind(this),
            error: function(data) {
                console.error('Error creating scene:', data);
                alert('Error creating scene.');
            }
        });
    },
    unmount: function() {
        var createBtn = document.getElementById('create_scene_btn');
        if (createBtn) createBtn.removeEventListener('click', this.createScene.bind(this));
        console.log('Unmounting scene create window.');
    }
};

    </script>
<?php
}
?>
