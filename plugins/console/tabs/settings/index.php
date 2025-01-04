<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="console_window.loadTabContent('debug/world.php');">World Editor</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="console_window.loadTabContent('audio');">Audio Manager</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="plugin.load({ id: 'tileset_window', url: 'renadmin/tileset/index.php', name: 'Tileset Manager', drag: true, reload: false });">Tileset</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="plugin.load({ id: 'renadmin_particles_window', url: 'renadmin/particles.php', name: 'Particles Manager', drag: true, reload: false });">Particles</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="plugin.load({ id: 'debug_gamepad_window', url: 'debug/gamepad.php', name: 'Gamepads', drag: true, reload: false });">Gamepads</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="plugin.load({ id: 'signout_window', url: 'auth/signout.php', name: 'Sign Out', drag: true, reload: false });">Signout</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="plugin.load({ id: 'template_window', url: 'template/index.php', name: 'Template', drag: true, reload: false });">Default plugin Template</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="console_window.loadTabContent('debug');">Debugging Utilities</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="plugin.load({ id: 'synth_window', url: 'synth/index.php', name: 'Synth', drag: true, reload: false });">Music Synth</button>
<br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="game.spawnRandomItems(1000);">Item Spawn</button><br />

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="plugin.load({ id: 'fps_monitor_window', url: 'debug/fps.php', name: 'FPS monitor', drag: true, reload: true });">FPS Monitor</button>

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="plugin.load({ id: 'yamltojson_window', url: 'testing/yamltojson.php', name: 'yamltojson', drag: true, reload: true });">Yaml to json</button>

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="plugin.load({ id: 'colorflash_window', url: 'minigames/colorflash.php', name: 'colorflash', drag: true, reload: true });">colorflash</button>

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="actions.mountHorse(game.playerid, 'horse1');">mount horse</button>

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="actions.dismountHorse('horse1');">dismount horse</button>

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="plugin.load({ id: 'dvd_window', url: 'silly/dvd/index.php', name: 'dvd window', drag: true, reload: true });">dvd</button>

<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="game.toggleWebGL(true);">Turn on WebGL</button>
<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="game.toggleWebGL(false);">Turn off WebGL</button>
