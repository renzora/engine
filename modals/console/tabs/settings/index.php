<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="console_window.loadTabContent('debug/world.php');">World Editor</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="console_window.loadTabContent('audio');">Audio Manager</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="modal.load({ id: 'tileset_window', url: 'renadmin/tileset/index.php', name: 'Tileset Manager', drag: true, reload: false });">Tileset</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="modal.load({ id: 'renadmin_particles_window', url: 'renadmin/particles.php', name: 'Particles Manager', drag: true, reload: false });">Particles</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="modal.load({ id: 'debug_gamepad_window', url: 'debug/gamepad.php', name: 'Gamepads', drag: true, reload: false });">Gamepads</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="modal.load({ id: 'signout_window', url: 'auth/signout.php', name: 'Sign Out', drag: true, reload: false });">Signout</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="modal.load({ id: 'template_window', url: 'template/index.php', name: 'Template', drag: true, reload: false });">Default Modal Template</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="console_window.loadTabContent('debug');">Debugging Utilities</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="modal.load({ id: 'synth_window', url: 'synth/index.php', name: 'Synth', drag: true, reload: false });">Music Synth</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="game.spawnRandomItems(1000);">Item Spawn</button><br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="modal.load({ id: 'fps_monitor_window', url: 'debug/fps.php', name: 'FPS monitor', drag: true, reload: true });">FPS Monitor</button>