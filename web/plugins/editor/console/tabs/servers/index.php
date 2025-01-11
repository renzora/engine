<div>
    <button
        id="open_create_server_plugin"
        class="green_button text-white font-bold py-2 px-3 rounded w-48 mt-2 shadow-md"
    >
        New Server
    </button>

    <div id="ui_console_tab_window_tabs">
        <div class="flex mt-4 space-x-2">
            <button class="tab text-white p-2 rounded" data-tab="public">Public</button>
            <button class="tab text-white p-2 rounded" data-tab="private">Private</button>
            <button class="tab text-white p-2 rounded" data-tab="events">Events</button>
            <button class="tab text-white p-2 rounded" data-tab="me">Me</button>
            <button class="tab text-white p-2 rounded" data-tab="favs">Favs</button>
            <button class="tab text-white p-2 rounded" data-tab="search">Search</button>
        </div>

        <div class="tab-content" data-tab-content="public">
            <div id="public-server-list" class="mt-4">
                Loading servers...
            </div>
        </div>

        <div class="tab-content" data-tab-content="private">
            <div id="private-server-list" class="mt-4">
                Loading servers...
            </div>
        </div>

        <div class="tab-content" data-tab-content="events">
            <div id="events-server-list" class="mt-4">
                Loading servers...
            </div>
        </div>

        <div class="tab-content" data-tab-content="me">
            <div id="me-server-list" class="mt-4">
                Loading servers...
            </div>
        </div>

        <div class="tab-content" data-tab-content="favs">
            <p>Content for Favs</p>
        </div>

        <div class="tab-content" data-tab-content="search">
            <p>Content for Search</p>
        </div>
    </div>
</div>

<style>
    .no-scenes-message {
        padding: 16px; /* Add padding to the message */
        color: #ffffff; /* Set the text color */
        text-align: center; /* Center the text */
    }
    .tab.active {
  background-color: #2d4c7d;
  color: #fff;
}

.tab-content.active {
  display: block;
}

.tab-content {

}
</style>

<script>
var ui_console_tab_window = {
    eventListeners: [],

    init: function() {
    this.attachEventListeners();

    // After attaching listeners, manually click the "public" tab
    const publicTab = document.querySelector('[data-tab="public"]');
    if (publicTab) {
        publicTab.click();
    }
},

    attachEventListeners: function() {
        // "New Server" button
        const openCreateServerButton = document.getElementById('open_create_server_plugin');
        if (openCreateServerButton) {
            const listener = this.openCreateServerplugin.bind(this);
            openCreateServerButton.addEventListener('click', listener);
            this.eventListeners.push({
                element: openCreateServerButton,
                event: 'click',
                handler: listener
            });
        }

        // Attach event listeners for each tab button
        document.querySelectorAll('.tab').forEach((tab) => {
            const listener = this.handleTabClick.bind(this);
            tab.addEventListener('click', listener);
            this.eventListeners.push({
                element: tab,
                event: 'click',
                handler: listener
            });
        });
    },

    // When a tab is clicked, show that tab's content and load the corresponding server list
    handleTabClick: function(event) {
    const tabType = event.target.getAttribute('data-tab');

    // Show/hide tab contents
    document.querySelectorAll('.tab-content').forEach((content) => {
        if (content.getAttribute('data-tab-content') === tabType) {
            content.classList.remove('hidden');
        } else {
            content.classList.add('hidden');
        }
    });

    // Load servers for THIS tab
    this.loadServers(tabType);
},

    openCreateServerplugin: function() {
        plugin.load({
            id: 'server_create_window',
            url: 'editor/console/tabs/servers/createServer.php',
            drag: true,
            reload: false
        });
    },

    loadServers: function(tabType) {
        const serverListDiv = document.getElementById(`${tabType}-server-list`);
        if (!serverListDiv) return;

        serverListDiv.innerHTML = 'Loading servers...';

        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'plugins/editor/console/tabs/servers/ajax/servers/getServers.php',
            data: JSON.stringify({ tabType: tabType }),
            headers: {
                'Content-Type': 'application/json'
            },
            success: function(data) {
                if (data.message === 'success') {
                    ui_console_tab_window.displayServers(data.servers, serverListDiv);
                } else {
                    serverListDiv.innerHTML = 'Error loading servers.';
                }
            },
            error: function(xhr, status, error) {
                serverListDiv.innerHTML = 'Error loading servers.';
            }
        });
    },

    displayServers: function(servers, serverListDiv) {
        serverListDiv.classList.remove('hidden');

        if (servers.length === 0) {
            serverListDiv.innerHTML = 'No servers found.';
        } else {
            serverListDiv.innerHTML = `
                <ul>
                    ${servers.map((server, index) => `
                        <li
                            class="server-item ${index === 0 ? 'rounded-t' : ''} ${index === servers.length - 1 ? 'rounded-b' : ''} ${index % 2 === 0 ? 'bg-gray-700' : 'bg-gray-800'} text-white shadow-md cursor-pointer"
                            data-server-id="${server.id}"
                        >
                            <div class="flex justify-between items-center pl-4 pr-2 py-2">
                                <span class="text-lg font-semibold">${server.name}</span>
                                <div class="flex space-x-2">
                                    <button
                                        class="white_button text-white font-bold py-1 px-2 rounded shadow-md"
                                        onclick="ui_console_tab_window.loadEditServerplugin('${server.id}', '${server.name}')"
                                    >
                                        Edit
                                    </button>
                                    <button
                                        class="green_button text-white font-bold py-1 px-2 rounded shadow-md"
                                        onclick="plugin.load({
                                            id: 'scene_create_window',
                                            url: 'editor/console/tabs/servers/createScene.php',
                                            drag: true,
                                            reload: false,
                                            onAfterLoad: function() {
                                                scene_create_window.server = '${server.id}';
                                            }
                                        });"
                                    >
                                        New
                                    </button>
                                </div>
                            </div>
                            <div
                                id="scenes-${server.id}"
                                class="scenes-list hidden transition-all ease-in-out duration-300 overflow-hidden max-h-0"
                            ></div>
                        </li>
                    `).join('')}
                </ul>
            `;

            // Add click handlers to each server item to toggle scenes
            serverListDiv.querySelectorAll('.server-item').forEach((item, index) => {
                const listener = (event) => {
                    // Ignore clicks on the Edit/New buttons themselves
                    if (event.target.tagName.toLowerCase() !== 'button') {
                        const serverId = item.getAttribute('data-server-id');
                        ui_console_tab_window.toggleScenes(serverId, item);
                    }
                };
                item.addEventListener('click', listener);
                this.eventListeners.push({
                    element: item,
                    event: 'click',
                    handler: listener
                });

                // Auto-toggle the first server by default
                if (index === 0) {
                    const serverId = item.getAttribute('data-server-id');
                    ui_console_tab_window.toggleScenes(serverId, item);
                }
            });
        }
    },

    toggleScenes: function(serverId, serverItem) {
        const sceneListDiv = document.getElementById(`scenes-${serverId}`);
        const allScenes = document.querySelectorAll('.scenes-list');

        if (!sceneListDiv) return;

        const isVisible = !sceneListDiv.classList.contains('hidden');

        // Hide all other scene lists
        allScenes.forEach(scene => {
            if (scene !== sceneListDiv) {
                scene.classList.add('hidden');
                scene.style.maxHeight = '0';
            }
        });

        // Toggle the target scene list
        if (isVisible) {
            sceneListDiv.classList.add('hidden');
            sceneListDiv.style.maxHeight = '0';
        } else {
            sceneListDiv.classList.remove('hidden');
            sceneListDiv.style.maxHeight = sceneListDiv.scrollHeight + "px";

            // Load scenes only once
            if (!sceneListDiv.getAttribute('data-loaded')) {
                this.loadScenes(serverId, sceneListDiv);
            }
        }
    },

    loadScenes: function(serverId, sceneListDiv) {
        sceneListDiv.innerHTML = 'Loading scenes...';

        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'plugins/editor/console/tabs/servers/ajax/scenes/getScenes.php',
            data: JSON.stringify({ serverId: serverId }),
            headers: {
                'Content-Type': 'application/json'
            },
            success: function(data) {
                if (data.message === 'success') {
                    ui_console_tab_window.displayScenes(data.scenes, sceneListDiv);
                    sceneListDiv.setAttribute('data-loaded', 'true');
                    sceneListDiv.style.maxHeight = sceneListDiv.scrollHeight + "px";
                } else {
                    sceneListDiv.innerHTML = 'Error loading scenes.';
                }
            },
            error: function(data) {
                sceneListDiv.innerHTML = 'Error loading scenes.';
            }
        });
    },

    displayScenes: function(scenes, sceneListDiv) {
        if (scenes.length === 0) {
            sceneListDiv.innerHTML = '<p class="no-scenes-message mt-4">No scenes found.</p>';
        } else {
            sceneListDiv.innerHTML = `
                <ul>
                    ${scenes.map((scene) => `
                        <li class="flex justify-between items-center pl-4 pr-2 py-2 hover:bg-blue-600 transition-colors">
                            <span class="text-lg text-gray-200">${scene.name}</span>
                            <div class="flex space-x-2">
                                <button
                                    class="white_button text-white font-bold py-1 px-2 rounded shadow-md"
                                    onclick="game.scene('${scene._id}')"
                                >
                                    Edit
                                </button>
                            </div>
                        </li>
                    `).join('')}
                </ul>
            `;
        }
    },

    loadEditServerplugin: function(serverId, serverName) {
        plugin.load({
            id: 'server_edit_window',
            url: `editor/console/tabs/servers/editServer.php?id=${serverId}&name=${encodeURIComponent(serverName)}`,
            name: 'Edit Server',
            drag: true,
            reload: false
        });
    },

    unmount: function() {
        // Remove all event listeners
        this.eventListeners.forEach(({ element, event, handler }) => {
            element.removeEventListener(event, handler);
        });
        this.eventListeners = [];
        console.log("All event listeners have been removed.");
    }
};

ui_console_tab_window.init();
</script>
