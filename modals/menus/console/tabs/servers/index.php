<div>
    <button id="open_create_server_modal" class="green_button text-white font-bold py-2 px-3 rounded w-48 mt-2 shadow-md">
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

        <div class="tab-content hidden" data-tab-content="public">
            <div id="public-server-list" class="mt-4">
                Loading servers...
            </div>
        </div>

        <div class="tab-content hidden" data-tab-content="private">
            <div id="private-server-list" class="mt-4">
                Loading servers...
            </div>
        </div>

        <div class="tab-content hidden" data-tab-content="events">
            <div id="events-server-list" class="mt-4">
                Loading servers...
            </div>
        </div>

        <div class="tab-content hidden" data-tab-content="me">
            <div id="me-server-list" class="mt-4">
                Loading servers...
            </div>
        </div>

        <div class="tab-content hidden" data-tab-content="favs">
            <p>Content for Favs</p>
        </div>

        <div class="tab-content hidden" data-tab-content="search">
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
</style>



<script>
var ui_console_tab_window = {
    init: function () {
        this.attachEventListeners();
        ui.initTabs('ui_console_tab_window_tabs', 'public'); // Set the "Public" tab as default
        this.loadServers('public'); // Load "Public" tab by default
    },
    attachEventListeners: function () {
        document.getElementById('open_create_server_modal').addEventListener('click', this.openCreateServerModal.bind(this));

        // Attach event listeners for each tab
        document.querySelectorAll('.tab').forEach((tab) => {
            tab.addEventListener('click', this.handleTabClick.bind(this));
        });
    },
    handleTabClick: function (event) {
        const tabType = event.target.getAttribute('data-tab');
        
        // Update the visible tab content
        document.querySelectorAll('.tab-content').forEach((content) => {
            if (content.getAttribute('data-tab-content') === tabType) {
                content.classList.remove('hidden');
            } else {
                content.classList.add('hidden');
            }
        });

        // Load servers based on the tab type
        this.loadServers(tabType);
    },
    openCreateServerModal: function () {
        modal.load({
            id: 'server_create_window',
            url: 'menus/console/tabs/servers/createServer.php',
            name: 'Create Server',
            drag: true,
            reload: false
        });
    },
    loadServers: function (tabType) {
        // Show loading message while fetching servers
        const serverListDiv = document.getElementById(`${tabType}-server-list`);
        serverListDiv.innerHTML = 'Loading servers...';

        // Log the tabType for debugging
        console.log('Tab Type Passed to Data:', tabType);

        // Make AJAX call to get servers based on the tab type
        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'modals/menus/console/tabs/servers/ajax/getServers.php',
            data: JSON.stringify({ tabType: tabType }), // Pass the tab type to the server
            headers: {
                'Content-Type': 'application/json'
            },
            success: function (data) {
                // Log the response data for debugging
                console.log('AJAX Success:', data);

                if (data.message === 'success') {
                    ui_console_tab_window.displayServers(data.servers, serverListDiv);
                } else {
                    serverListDiv.innerHTML = 'Error loading servers.';
                    // Log any errors returned in the response
                    console.error('Error message from server:', data.message, 'Error details:', data.error);
                }
            },
            error: function (xhr, status, error) {
                serverListDiv.innerHTML = 'Error loading servers.';
                // Log the error for debugging
                console.error('AJAX Error:', {
                    xhr: xhr,
                    status: status,
                    error: error
                });
            }
        });
    },
    displayServers: function (servers, serverListDiv) {
        serverListDiv.classList.remove('hidden');

        if (servers.length === 0) {
            serverListDiv.innerHTML = 'No servers found.';
        } else {
            serverListDiv.innerHTML = '<ul>' + servers.map((server, index) => `
                <li class="server-item ${index === 0 ? 'rounded-t' : ''} ${index === servers.length - 1 ? 'rounded-b' : ''} ${index % 2 === 0 ? 'bg-gray-700' : 'bg-gray-800'} text-white shadow-md cursor-pointer" data-server-id="${server.id}">
                    <div class="flex justify-between items-center pl-4 pr-2 py-2">
                        <span class="text-lg font-semibold">${server.name}</span>
                        <div class="flex space-x-2">
                            <button class="white_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="ui_console_tab_window.loadEditServerModal('${server.id}', '${server.name}')">Edit</button>
                        </div>
                    </div>
                    <div id="scenes-${server.id}" class="scenes-list hidden transition-all ease-in-out duration-300 overflow-hidden max-h-0">
                        <!-- Scenes will be dynamically inserted here -->
                    </div>
                </li>
            `).join('') + '</ul>';

            // Add event listeners to server items
            serverListDiv.querySelectorAll('.server-item').forEach((item, index) => {
                item.addEventListener('click', function (event) {
                    // Prevent the event from triggering on button clicks
                    if (event.target.tagName.toLowerCase() !== 'button') {
                        const serverId = this.getAttribute('data-server-id');
                        ui_console_tab_window.toggleScenes(serverId, this);
                    }
                });

                // Open the first item by default
                if (index === 0) {
                    const serverId = item.getAttribute('data-server-id');
                    ui_console_tab_window.toggleScenes(serverId, item);
                }
            });
        }
    },
    toggleScenes: function (serverId, serverItem) {
        const sceneListDiv = document.getElementById(`scenes-${serverId}`);
        const allScenes = document.querySelectorAll('.scenes-list');

        if (!sceneListDiv) {
            console.error('Scene list element not found for serverId:', serverId);
            return;
        }

        const isVisible = !sceneListDiv.classList.contains('hidden');

        // Close all other open scene lists
        allScenes.forEach(scene => {
            if (scene !== sceneListDiv) {
                scene.classList.add('hidden');
                scene.style.maxHeight = '0';

                // Remove the New Scene button when other server items are closed
                const newSceneButton = scene.previousElementSibling.querySelector('.new-scene-button');
                if (newSceneButton) {
                    newSceneButton.remove();
                }
            }
        });

        // Toggle the clicked scene list
        if (isVisible) {
            sceneListDiv.classList.add('hidden');
            sceneListDiv.style.maxHeight = '0';

            // Remove the New Scene button when the current server item is closed
            const newSceneButton = serverItem.querySelector('.new-scene-button');
            if (newSceneButton) {
                newSceneButton.remove();
            }
        } else {
            sceneListDiv.classList.remove('hidden');

            // Add the New Scene button dynamically
            const controlDiv = serverItem.querySelector('.flex.space-x-2');
            if (!controlDiv.querySelector('.new-scene-button')) {
                const newSceneButton = document.createElement('button');
                newSceneButton.className = 'green_button text-white font-bold py-1 px-2 rounded shadow-md new-scene-button';
                newSceneButton.textContent = 'New Scene';
                newSceneButton.onclick = () => ui_console_tab_window.loadCreateSceneModal(serverId);
                controlDiv.appendChild(newSceneButton);
            }

            if (!sceneListDiv.getAttribute('data-loaded')) {
                this.loadScenes(serverId, sceneListDiv);
            } else {
                // Apply sliding effect for already loaded scenes
                sceneListDiv.style.maxHeight = sceneListDiv.scrollHeight + "px";
            }
        }
    },
    loadScenes: function (serverId, sceneListDiv) {
        sceneListDiv.innerHTML = 'Loading scenes...';

        console.log('Sending request to load scenes for serverId:', serverId);

        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'modals/menus/console/tabs/servers/ajax/getScenes.php',
            data: JSON.stringify({ serverId: serverId }),
            headers: {
                'Content-Type': 'application/json'
            },
            success: function (data) {
                console.log('Scenes loaded:', data); // Log the response data
                if (data.message === 'success') {
                    ui_console_tab_window.displayScenes(data.scenes, sceneListDiv);
                    sceneListDiv.setAttribute('data-loaded', 'true'); // Mark scenes as loaded
                    // Ensure the sliding effect works after scenes are loaded
                    sceneListDiv.style.maxHeight = sceneListDiv.scrollHeight + "px";
                } else {
                    console.error('Error loading scenes:', data.message, data.error);
                    sceneListDiv.innerHTML = 'Error loading scenes.';
                }
            },
            error: function (data) {
                console.error('Error fetching scenes:', data);
                sceneListDiv.innerHTML = 'Error loading scenes.';
            }
        });
    },
    displayScenes: function (scenes, sceneListDiv) {
        if (scenes.length === 0) {
            // Apply the 'no-scenes-message' class for styling
            sceneListDiv.innerHTML = '<p class="no-scenes-message mt-4">No scenes found.</p>';
        } else {
            sceneListDiv.innerHTML = '<ul>' + scenes.map((scene, index) => `
                <li class="flex justify-between items-center pl-4 pr-2 py-2 hover:bg-blue-600 transition-colors">
                    <span class="text-lg text-gray-200">${scene.name}</span>
                    <div class="flex space-x-2">
                        <button class="white_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="ui_console_tab_window.loadEditSceneModal('${scene._id}', '${scene.name}', '${scene.server_id}')">Edit</button>
                        <button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="game.loadScene('${scene._id}')">Go</button>
                    </div>
                </li>
            `).join('') + '</ul>';
        }
    },
    addServerToList: function (server) {
        const serverListDiv = document.getElementById('public-server-list');

        // Ensure the server list is a <ul> element
        let serverListUl = serverListDiv.querySelector('ul');
        if (!serverListUl) {
            serverListUl = document.createElement('ul');
            serverListUl.classList.add('list-none', 'p-0', 'm-0'); // Add class to remove default list styles
            serverListDiv.appendChild(serverListUl);
        }

        // Insert the new server item at the top
        const currentItems = serverListUl.querySelectorAll('.server-item');
        const index = 0; // Always add to the top

        // Determine the background color
        const bgColor = index % 2 === 0 ? 'bg-gray-700' : 'bg-gray-800';

        // Construct the server item HTML
        const serverItem = `
            <li class="server-item ${bgColor} text-white shadow-md cursor-pointer" data-server-id="${server.id}">
                <div class="flex justify-between items-center pl-4 pr-2 py-2">
                    <span class="text-lg font-semibold">${server.name}</span>
                    <div class="flex space-x-2">
                        <button class="white_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="ui_console_tab_window.loadEditServerModal('${server.id}', '${server.name}')">Edit</button>
                    </div>
                </div>
                <div id="scenes-${server.id}" class="scenes-list hidden transition-all ease-in-out duration-300 overflow-hidden max-h-0"></div>
            </li>
        `;

        // Prepend the new server item to the list
        serverListUl.insertAdjacentHTML('afterbegin', serverItem);

        // Update the styles for the rest of the list to maintain the alternating pattern
        const updatedItems = serverListUl.querySelectorAll('.server-item');
        updatedItems.forEach((item, i) => {
            const newBgColor = i % 2 === 0 ? 'bg-gray-700' : 'bg-gray-800';
            item.classList.remove('bg-gray-700', 'bg-gray-800');
            item.classList.add(newBgColor);
        });

        const newServerElement = serverListUl.firstElementChild;
        newServerElement.addEventListener('click', function (event) {
            if (event.target.tagName.toLowerCase() !== 'button') {
                const serverId = this.getAttribute('data-server-id');
                ui_console_tab_window.toggleScenes(serverId, this);
            }
        });

        // Automatically open the scenes for the newly created server
        ui_console_tab_window.toggleScenes(server.id, newServerElement);
    },
    addSceneToList: function (scene, serverId) {
        console.log('Adding scene to server:', serverId); // Debug log
        const sceneListDiv = document.getElementById(`scenes-${serverId}`);

        if (sceneListDiv) {
            if (sceneListDiv.innerHTML.includes('No scenes found.')) {
                sceneListDiv.innerHTML = ''; // Clear the 'No scenes found.' message
            }

            const sceneItem = `
                <li class="flex justify-between items-center pl-4 pr-2 py-2 hover:bg-blue-600 transition-colors">
                    <span class="text-lg text-gray-200">${scene.name}</span>
                    <div class="flex space-x-2">
                        <button class="white_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="ui_console_tab_window.loadEditSceneModal('${scene.id}', '${scene.name}', '${scene.server_id}')">Edit</button>
                        <button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="game.loadScene('${scene.id}')">Go</button>
                    </div>
                </li>
            `;

            sceneListDiv.insertAdjacentHTML('beforeend', sceneItem);
            sceneListDiv.style.maxHeight = sceneListDiv.scrollHeight + "px"; // Adjust height for sliding effect
        } else {
            console.error('Scene list element not found for serverId:', serverId);
        }
    },
    loadEditServerModal: function (serverId, serverName) {
        modal.load({
            id: 'server_edit_window',
            url: `menus/console/tabs/servers/editServer.php?id=${serverId}&name=${encodeURIComponent(serverName)}`,
            name: 'Edit Server',
            drag: true,
            reload: false
        });
    },
    loadEditSceneModal: function (sceneId, sceneName, serverId) {
        modal.load({
            id: 'scene_edit_window',
            url: `menus/console/tabs/servers/editScene.php?id=${sceneId}&name=${encodeURIComponent(sceneName)}&serverId=${serverId}`,
            name: 'Edit Scene',
            drag: true,
            reload: false
        });
    },
    loadCreateSceneModal: function (serverId) {
        console.log('Loading create scene modal for serverId:', serverId);
        modal.load({
            id: 'scene_create_window',
            url: `menus/console/tabs/servers/createScene.php?id=${serverId}`,
            name: 'Create Scene',
            drag: true,
            reload: false
        });
    },
    unmount: function () {
        document.getElementById('open_create_server_modal').removeEventListener('click', this.openCreateServerModal.bind(this));
        // Detach other event listeners if needed
    }
};

ui_console_tab_window.init();
</script>
