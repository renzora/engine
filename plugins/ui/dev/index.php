<div data-close="false">
  <div class='fixed bottom-0 right-2 z-10 text-sm mb-1 flex space-x-4 tracking-tight'>
    <a href="https://github.com/renzora/web/commit/7eb3abef7a45ac66e0869e23410701fbbf605110" 
       target="_blank" 
       class="text-white rounded-md">Renzora v0.3.6+dev-build-139</a>
    <span class="text-white" id="input_method" onclick=" plugin.load({ id: 'gamepads_window', url: 'utils/gamepads/index.php', name: 'gamepads', drag: true, reload: true });">Input: keyboard</span>
    <span class="text-white rounded-md cursor-pointer" id="gameFps" onclick="plugin.load({ id: 'fps_monitor_window', url: 'debug/fps.php', name: 'FPS monitor', drag: true, reload: true });"></span>
    <span id="tiles_rendered" class="text-white rounded-md"></span>
    <span id="background_rendered" class="text-white rounded-md"></span>
    <span id="lights_rendered" class="text-white rounded-md"></span>
    <span id="effects_rendered" class="text-white rounded-md"></span>
    <span id="animations_rendered" class="text-white rounded-md"></span>
  </div>
  </div>

<script>
window[id] = {
    id: id,
    updateUI: function () {
        var tilesRenderedDisplay = document.getElementById('tiles_rendered');

        if (tilesRenderedDisplay) {
            tilesRenderedDisplay.innerHTML = `Tiles: ${render.tileCount}`;
        }

        var background_rendered = document.getElementById('background_rendered');

        if (background_rendered) {
            background_rendered.innerHTML = `Background: ${render.backgroundTileCount}`;
        }
    
        var lightsRenderedDisplay = document.getElementById('lights_rendered');
        if (lightsRenderedDisplay) {
            lightsRenderedDisplay.innerHTML = `Lights: ${lighting.lights.length}`;
        }
    
        var effectsRenderedDisplay = document.getElementById('effects_rendered');
        if (effectsRenderedDisplay) {
            effectsRenderedDisplay.innerHTML = `Effects: ${Object.keys(particles.activeEffects).length}`;
        }
    
        var animationsRenderedDisplay = document.getElementById('animations_rendered');
        if (animationsRenderedDisplay) {
            animationsRenderedDisplay.innerHTML = `Animations: ${render.animationCount}`;
        }
    }
  };
</script>