<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if($auth) {
?>
  <div data-window='debug_window' class='window position-fixed bottom-0 right-2' style='width: 250px;background: #bba229; margin-bottom: 10px;'>
  
    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>Scene Debug</div>
    </div>
    <div class='clearfix'></div>
    <div class='position-relative'>
      <div class='container text-light window_body p-2'>
        <button id='register_connect' onclick="modal.load('debug/sprite_debug.php', 'sprite_debug_window');" class="green_button text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md">Sprite Debug</button>

        <button class="green_button text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md" onclick="network.sendReloadRequest();">Reload Game Data</button>

        <button id='vibrate_test' onclick="debug_window.vibrateTest();" class="green_button text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md">Vibrate Test</button>

        <div class="debug-controls mt-2">
        <!-- Increase/Decrease Energy Buttons -->
        <button id='increase_energy' onclick="debug_window.changeEnergy(10);" class="green_button text-white font-bold py-2 px-3 rounded w-48 mt-2 shadow-md">Energy++</button>
        <button id='decrease_energy' onclick="debug_window.changeEnergy(-10);" class="green_button text-white font-bold py-2 px-3 rounded w-48 mt-2 shadow-md">Energy--</button>
        
        <!-- Increase/Decrease Health Buttons -->
        <button id='increase_health' onclick="debug_window.changeHealth(10);" class="green_button text-white font-bold py-2 px-3 rounded w-48 mt-2 shadow-md">Health++</button>
        <button id='decrease_health' onclick="debug_window.changeHealth(-10);" class="green_button text-white font-bold py-2 px-3 rounded w-48 mt-2 shadow-md">Health--</button>
    </div>

        <div class="clearfix mt-2"></div>
        <div class="debug-controls mt-2">
          <label><input type="checkbox" id="toggleGrid"> Show Grid</label><br>
          <label><input type="checkbox" id="toggleCollision"> Show Collision</label><br>
          <label><input type="checkbox" id="toggleSnow"> Show Snow</label><br>
          <label><input type="checkbox" id="toggleRain"> Show Rain</label><br>
          <label><input type="checkbox" id="toggleFog"> Show Fog</label><br>
          <label><input type="checkbox" id="toggleStars"> Show Stars</label><br>
          <label><input type="checkbox" id="toggleNight"> Show Night</label>
        </div>
      </div>
    </div>

    <script>
      var debug_window = {
        start: function() {
          this.bindControls();
          this.initializeCheckboxes();
        },
        unmount: function() {
          cancelAnimationFrame(this.fpsAnimationFrame);
          this.removeControls();
        },
        bindControls: function() {
          document.getElementById('toggleFPS').addEventListener('change', this.toggleFPS.bind(this));
          document.getElementById('toggleGrid').addEventListener('change', this.toggleGrid.bind(this));
          document.getElementById('toggleCollision').addEventListener('change', this.toggleCollision.bind(this));
          document.getElementById('toggleSnow').addEventListener('change', this.toggleSnow.bind(this));
          document.getElementById('toggleRain').addEventListener('change', this.toggleRain.bind(this));
          document.getElementById('toggleFog').addEventListener('change', this.toggleFog.bind(this));
          document.getElementById('toggleStars').addEventListener('change', this.toggleStars.bind(this));
          document.getElementById('toggleNight').addEventListener('change', this.toggleNight.bind(this));
        },
        removeControls: function() {
          document.getElementById('toggleFPS').removeEventListener('change', this.toggleFPS.bind(this));
          document.getElementById('toggleGrid').removeEventListener('change', this.toggleGrid.bind(this));
          document.getElementById('toggleCollision').removeEventListener('change', this.toggleCollision.bind(this));
          document.getElementById('toggleSnow').removeEventListener('change', this.toggleSnow.bind(this));
          document.getElementById('toggleRain').removeEventListener('change', this.toggleRain.bind(this));
          document.getElementById('toggleFog').removeEventListener('change', this.toggleFog.bind(this));
          document.getElementById('toggleStars').removeEventListener('change', this.toggleStars.bind(this));
          document.getElementById('toggleNight').removeEventListener('change', this.toggleNight.bind(this));
        },
        initializeCheckboxes: function() {
          document.getElementById('toggleSnow').checked = weather.snowActive;
          document.getElementById('toggleRain').checked = weather.rainActive;
          document.getElementById('toggleFog').checked = weather.fogActive;
          document.getElementById('toggleStars').checked = weather.starsActive;
          document.getElementById('toggleNight').checked = game.isNightActive;
        },
        toggleGrid: function(event) {
          game.showGrid = event.target.checked;
        },
        toggleCollision: function(event) {
          game.showCollision = event.target.checked;
        },
        toggleSnow: function(event) {
          weather.snowActive = event.target.checked;
          if (weather.snowActive) {
            weather.createSnow(0.7);
          } else {
            weather.stopSnow();
          }
        },
        toggleRain: function(event) {
          weather.rainActive = event.target.checked;
          if (weather.rainActive) {
            weather.createRain(0.7);
          } else {
            weather.rainDrops = [];
          }
        },
        toggleFog: function(event) {
          weather.fogActive = event.target.checked;
          if (weather.fogActive) {
            weather.createFog(0.05);
          } else {
            weather.fogs = [];
          }
        },
        toggleStars: function(event) {
          weather.starsActive = event.target.checked;
          if (weather.starsActive) {
            weather.createStars();
          } else {
            weather.stars = [];
          }
        },
        toggleNight: function(event) {
          game.isNightActive = event.target.checked;
        },
        changeEnergy: function(amount) {
        const player = game.sprites['main']; // Ensure the player sprite is accessible
        if (player) {
            player.updateEnergy(amount);
        }
    },
    
    changeHealth: function(amount) {
        const player = game.sprites['main']; // Ensure the player sprite is accessible
        if (player) {
            player.updateHealth(amount);  // Ensure this method exists in your player object
        }
    },
    vibrateTest: function() {
      const gamepad = navigator.getGamepads()[0];

      gamepad.vibrationActuator.playEffect("dual-rumble", {
  startDelay: 0,
  duration: 2000,
  weakMagnitude: 1.0,
  strongMagnitude: 1.0,
});
    },
        sprite: function(sprite) {
          const spriteX = sprite.x;
          const spriteY = sprite.y;
          const spriteWidth = sprite.width * sprite.scale;
          const spriteHeight = sprite.height * sprite.scale;
          game.ctx.save();
          this.drawBounds(spriteX, spriteY, spriteWidth, spriteHeight);
          game.ctx.restore();
        },
        drawBounds: function(x, y, width, height) {
          game.ctx.strokeStyle = 'blue';
          game.ctx.lineWidth = 2;
          game.ctx.strokeRect(x, y, width, height);
        },
        highlightWalkableArea: function(area) {
          game.ctx.save();
          game.ctx.fillStyle = 'rgba(0, 255, 0, 0.2)';
          game.ctx.fillRect(area.x, area.y, area.width, area.height);
          game.ctx.restore();
        },
        camera: function() {
          // Implementation for debugging camera
        },
        grid: function() {
          game.ctx.strokeStyle = 'rgba(0, 0, 0, 0.1)';
          game.ctx.lineWidth = 1;
          for (var x = 0; x <= game.worldWidth; x += 16) {
            game.ctx.beginPath();
            game.ctx.moveTo(x, 0);
            game.ctx.lineTo(x, game.worldHeight);
            game.ctx.stroke();
          }
          for (var y = 0; y <= game.worldHeight; y += 16) {
            game.ctx.beginPath();
            game.ctx.moveTo(0, y);
            game.ctx.lineTo(game.worldWidth, y);
            game.ctx.stroke();
          }
        },
        tiles: function() {
          if (!game.roomData || !game.roomData.items) return;
          game.roomData.items.forEach(roomItem => {
            const itemData = assets.load('objectData')[roomItem.id];
            if (itemData && itemData.length > 0) {
              const tileData = itemData[0];
              const xCoordinates = roomItem.x || [];
              const yCoordinates = roomItem.y || [];
              let index = 0;
              for (let j = 0; j < yCoordinates.length; j++) {
                for (let i = 0; i < xCoordinates.length; i++) {
                  const posX = parseInt(xCoordinates[i], 10) * 16;
                  const posY = parseInt(yCoordinates[j], 10) * 16;
                  let collisionOffsets;
                  if (Array.isArray(tileData.w)) {
                    collisionOffsets = tileData.w[index % tileData.w.length];
                  } else {
                    collisionOffsets = tileData.w === 0 ? [0, 0, 0, 0] : null;
                  }
                  if (collisionOffsets) {
                    const [nOffset, eOffset, sOffset, wOffset] = collisionOffsets;
                    const collisionX = posX + wOffset;
                    const collisionY = posY + nOffset;
                    const collisionWidth = 16 - wOffset - eOffset;
                    const collisionHeight = 16 - nOffset - sOffset;
                    game.ctx.save();
                    game.ctx.strokeStyle = 'red';
                    game.ctx.lineWidth = 1;
                    game.ctx.beginPath();
                    if (nOffset !== 16) {
                      game.ctx.moveTo(collisionX, collisionY);
                      game.ctx.lineTo(collisionX + collisionWidth, collisionY);
                    }
                    if (eOffset !== 16) {
                      game.ctx.moveTo(collisionX + collisionWidth, collisionY);
                      game.ctx.lineTo(collisionX + collisionWidth, collisionY + collisionHeight);
                    }
                    if (sOffset !== 16) {
                      game.ctx.moveTo(collisionX, collisionY + collisionHeight);
                      game.ctx.lineTo(collisionX + collisionWidth, collisionY + collisionHeight);
                    }
                    if (wOffset !== 16) {
                      game.ctx.moveTo(collisionX, collisionY);
                      game.ctx.lineTo(collisionX, collisionY + collisionHeight);
                    }
                    game.ctx.stroke();
                    game.ctx.fillStyle = 'black';
                    game.ctx.font = '2px Arial';
                    const text = collisionOffsets.join(',');
                    game.ctx.fillText(text, posX + 2, posY + 12);
                    game.ctx.restore();
                  }
                  index++;
                }
              }
            }
          });
          for (let id in game.sprites) {
            const sprite = game.sprites[id];
            if (sprite) {
              const collisionBox = {
                x: sprite.x,
                y: sprite.y + sprite.height * sprite.scale / 2,
                width: sprite.width * sprite.scale,
                height: sprite.height * sprite.scale / 2
              };
              game.ctx.save();
              game.ctx.strokeStyle = 'green';
              game.ctx.lineWidth = 1;
              game.ctx.strokeRect(collisionBox.x, collisionBox.y, collisionBox.width, collisionBox.height);
              game.ctx.restore();
            }
          }
        }
      };
      debug_window.start();
    </script>
  </div>
<?php
}
?>
