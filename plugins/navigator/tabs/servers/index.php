<div>
    <button id="open_create_server_plugin" class="green_button text-white font-bold py-2 px-3 rounded w-48 mt-2 shadow-md">New Server</button>
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
        document.getElementById('open_create_server_plugin').addEventListener('click', this.openCreateServerplugin.bind(this));
    },
    openCreateServerplugin: function() {
        plugin.load('navigator/tabs/servers/createServer.php', 'server_create_window');
    },
    loadServers: function() {
        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'plugins/navigator/tabs/servers/ajax/getServers.php',
            data: JSON.stringify({}),
            headers: {
                'Content-Type': 'application/json'
            },
            success: function(data) {
                navigator.log('Servers loaded:', data); // Log the response data
                if (data.message === 'success') {
                    ui_servers_tab_window.displayServers(data.servers);
                } else {
                    document.getElementById('server-list').innerHTML = 'Error loading servers.';
                }
            },
            error: function(data) {
                document.getElementById('server-list').innerHTML = 'Error loading servers.';
                navigator.error('Error fetching servers:', data);
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
                        <button class="white_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="ui_servers_tab_window.loadEditServerplugin('${server.id}', '${server.name}')">Edit</button>
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

        navigator.log('Sending request to load scenes for serverId:', serverId);

        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'plugins/navigator/tabs/servers/ajax/getScenes.php',
            data: JSON.stringify({ serverId: serverId }),
            headers: {
                'Content-Type': 'application/json'
            },
            success: function(data) {
                navigator.log('Scenes loaded:', data); // Log the response data
                if (data.message === 'success') {
                    ui_servers_tab_window.displayScenes(data.scenes, serverId);
                } else {
                    navigator.error('Error loading scenes:', data.message, data.error);
                    document.getElementById('scene-list').innerHTML = 'Error loading scenes.';
                }
            },
            error: function(data) {
                navigator.error('Error fetching scenes:', data);
                document.getElementById('scene-list').innerHTML = 'Error loading scenes.';
            }
        });
    },
    displayScenes: function(scenes, serverId) {
        const sceneListDiv = document.getElementById('scene-list');

        let sceneListHTML = `
            <div class="flex justify-between mb-4">
                <button class="white_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="ui_servers_tab_window.loadServers()">« Back</button>
                <button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="ui_servers_tab_window.loadCreateSceneplugin('${serverId}')">New Scene</button>
            </div>`;

        if (scenes.length === 0) {
            sceneListHTML += '<p class="text-white mt-4">No scenes found.</p>';
        } else {
            sceneListHTML += '<ul>' + scenes.map((scene, index) => `
                <li class="flex justify-between items-center p-2 ${index % 2 === 0 ? 'bg-gray-700' : 'bg-gray-800'} ${index === 0 ? 'rounded-t' : ''} ${index === scenes.length - 1 ? 'rounded-b' : ''} shadow">
                    <span class="text-white text-lg">${scene.name}</span>
                    <div class="flex space-x-2">
                        <button class="white_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="ui_servers_tab_window.loadEditSceneplugin('${scene._id}', '${scene.name}', '${serverId}')">Edit</button>
                        <button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="game.loadScene('${scene._id}')">Go</button>
                    </div>
                </li>
            `).join('') + '</ul>';
        }

        sceneListDiv.innerHTML = sceneListHTML;
    },
    loadEditServerplugin: function(serverId, serverName) {
        plugin.load(`navigator/tabs/servers/editServer.php?id=${serverId}&name=${encodeURIComponent(serverName)}`, 'server_edit_window');
    },
    loadEditSceneplugin: function(sceneId, sceneName, serverId) {
        plugin.load(`navigator/tabs/servers/editScene.php?id=${sceneId}&name=${encodeURIComponent(sceneName)}&serverId=${serverId}`, 'scene_edit_window');
    },
    loadCreateSceneplugin: function(serverId) {
        navigator.log('Loading create scene plugin for serverId:', serverId);
        plugin.load(`navigator/tabs/servers/createScene.php?id=${serverId}`, 'scene_create_window');
    },
    unmount: function() {
        document.getElementById('open_create_server_plugin').removeEventListener('click', this.openCreateServerplugin.bind(this));
        // Detach other event listeners if needed
    }
};

ui_servers_tab_window.init();
</script>
