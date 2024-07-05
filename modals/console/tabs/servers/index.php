<div>
    <button id="open_create_server_modal" class="green_button text-white font-bold py-2 px-3 rounded w-48 mt-2 shadow-md">New Server</button>
    <div id="server-list" class="mt-4">
        Loading servers...
    </div>
    <div id="scene-list" class="mt-4 hidden">
        <!-- Scene list will be dynamically inserted here -->
    </div>
</div>

<script>
var ui_servers_tab_window = {
    init: function() {
        this.loadServers();
        this.attachEventListeners();
    },
    attachEventListeners: function() {
        document.getElementById('open_create_server_modal').addEventListener('click', this.openCreateServerModal.bind(this));
    },
    openCreateServerModal: function() {
        modal.load('console/tabs/servers/createServer.php', 'server_create_window');
    },
    loadServers: function() {
        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'modals/console/tabs/servers/ajax/getServers.php',
            data: JSON.stringify({}),
            headers: {
                'Content-Type': 'application/json'
            },
            success: function(data) {
                console.log('Servers loaded:', data); // Log the response data
                if (data.message === 'success') {
                    ui_servers_tab_window.displayServers(data.servers);
                } else {
                    document.getElementById('server-list').innerHTML = 'Error loading servers.';
                }
            },
            error: function(data) {
                document.getElementById('server-list').innerHTML = 'Error loading servers.';
                console.error('Error fetching servers:', data);
            }
        });
    },
    displayServers: function(servers) {
        const serverListDiv = document.getElementById('server-list');
        const sceneListDiv = document.getElementById('scene-list');

        serverListDiv.classList.remove('hidden');
        sceneListDiv.classList.add('hidden');

        if (servers.length === 0) {
            serverListDiv.innerHTML = 'No servers found.';
        } else {
            serverListDiv.innerHTML = '<ul>' + servers.map((server, index) => `
                <li class="flex justify-between items-center p-2 ${index % 2 === 0 ? 'bg-gray-700' : 'bg-gray-800'} ${index === 0 ? 'rounded-t' : ''} ${index === servers.length - 1 ? 'rounded-b' : ''} shadow">
                    <span class="text-white text-lg">${server.name}</span>
                    <div class="flex space-x-2">
                        <button class="white_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="ui_servers_tab_window.loadEditServerModal('${server.id}', '${server.name}')">Edit</button>
                        <button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="ui_servers_tab_window.loadScenes('${server.id}')">Go</button>
                    </div>
                </li>
            `).join('') + '</ul>';
        }
    },
    loadScenes: function(serverId) {
        const serverListDiv = document.getElementById('server-list');
        const sceneListDiv = document.getElementById('scene-list');

        serverListDiv.classList.add('hidden');
        sceneListDiv.classList.remove('hidden');
        sceneListDiv.innerHTML = 'Loading scenes...';

        console.log('Sending request to load scenes for serverId:', serverId);

        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'modals/console/tabs/servers/ajax/getScenes.php',
            data: JSON.stringify({ serverId: serverId }),
            headers: {
                'Content-Type': 'application/json'
            },
            success: function(data) {
                console.log('Scenes loaded:', data); // Log the response data
                if (data.message === 'success') {
                    ui_servers_tab_window.displayScenes(data.scenes, serverId);
                } else {
                    console.error('Error loading scenes:', data.message, data.error);
                    document.getElementById('scene-list').innerHTML = 'Error loading scenes.';
                }
            },
            error: function(data) {
                console.error('Error fetching scenes:', data);
                document.getElementById('scene-list').innerHTML = 'Error loading scenes.';
            }
        });
    },
    displayScenes: function(scenes, serverId) {
        const sceneListDiv = document.getElementById('scene-list');

        let sceneListHTML = `
            <div class="flex justify-between mb-4">
                <button class="white_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="ui_servers_tab_window.loadServers()">« Back</button>
                <button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="ui_servers_tab_window.loadCreateSceneModal('${serverId}')">New Scene</button>
            </div>`;

        if (scenes.length === 0) {
            sceneListHTML += '<p class="text-white mt-4">No scenes found.</p>';
        } else {
            sceneListHTML += '<ul>' + scenes.map((scene, index) => `
                <li class="flex justify-between items-center p-2 ${index % 2 === 0 ? 'bg-gray-700' : 'bg-gray-800'} ${index === 0 ? 'rounded-t' : ''} ${index === scenes.length - 1 ? 'rounded-b' : ''} shadow">
                    <span class="text-white text-lg">${scene.name}</span>
                    <div class="flex space-x-2">
                        <button class="white_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="ui_servers_tab_window.loadEditSceneModal('${scene._id}', '${scene.name}', '${serverId}')">Edit</button>
                        <button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="game.loadScene('${scene._id}')">Go</button>
                    </div>
                </li>
            `).join('') + '</ul>';
        }

        sceneListDiv.innerHTML = sceneListHTML;
    },
    loadEditServerModal: function(serverId, serverName) {
        modal.load(`console/tabs/servers/editServer.php?id=${serverId}&name=${encodeURIComponent(serverName)}`, 'server_edit_window');
    },
    loadEditSceneModal: function(sceneId, sceneName, serverId) {
        modal.load(`console/tabs/servers/editScene.php?id=${sceneId}&name=${encodeURIComponent(sceneName)}&serverId=${serverId}`, 'scene_edit_window');
    },
    loadCreateSceneModal: function(serverId) {
        console.log('Loading create scene modal for serverId:', serverId);
        modal.load(`console/tabs/servers/createScene.php?id=${serverId}`, 'scene_create_window');
    },
    unmount: function() {
        document.getElementById('open_create_server_modal').removeEventListener('click', this.openCreateServerModal.bind(this));
        // Detach other event listeners if needed
    }
};

ui_servers_tab_window.init();
</script>
