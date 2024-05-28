<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if($auth) {
?>
  <div data-window='debug_window' class='window position-fixed bottom-12 right-2' style='width: 250px;height: 370px; background: #bba229;'>
  
    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>Scene Debug</div>
    </div>
    <div class='clearfix'></div>
    <div class='position-relative'>
      <div class='container text-light window_body p-2'>
        <div id="gameFps"></div>
        <div class="clearfix mt-2"></div>
        <div class="debug-controls mt-2">
          <label><input type="checkbox" id="toggleFPS" checked> Show FPS</label><br>
          <label><input type="checkbox" id="toggleGrid"> Show Grid</label><br>
          <label><input type="checkbox" id="toggleCollision"> Show Collision</label>
        </div>
      </div>
    </div>

    <script>
  var debug_window = {
    start: function() {
      // Initialize debug functions here
      this.updateFps();
      this.bindControls();
    },
    unmount: function() {
      cancelAnimationFrame(this.fpsAnimationFrame);
      this.removeControls();
    },
    bindControls: function() {
      document.getElementById('toggleFPS').addEventListener('change', this.toggleFPS.bind(this));
      document.getElementById('toggleGrid').addEventListener('change', this.toggleGrid.bind(this));
      document.getElementById('toggleCollision').addEventListener('change', this.toggleCollision.bind(this));
      document.getElementById('toggleTiles').addEventListener('change', this.toggleTiles.bind(this));
    },
    removeControls: function() {
      document.getElementById('toggleFPS').removeEventListener('change', this.toggleFPS.bind(this));
      document.getElementById('toggleGrid').removeEventListener('change', this.toggleGrid.bind(this));
      document.getElementById('toggleCollision').removeEventListener('change', this.toggleCollision.bind(this));
      document.getElementById('toggleTiles').removeEventListener('change', this.toggleTiles.bind(this));
    },
    toggleFPS: function(event) {
      if (event.target.checked) {
        this.updateFps();
      } else {
        cancelAnimationFrame(this.fpsAnimationFrame);
        document.getElementById('gameFps').innerHTML = '';
      }
    },
    toggleGrid: function(event) {
      game.showGrid = event.target.checked;
    },
    toggleCollision: function(event) {
      game.showCollision = event.target.checked;
    },
    toggleTiles: function(event) {
      game.showTiles = event.target.checked;
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
    updateFps: function() {
      var debugFPS = document.getElementById('gameFps');
      if (debugFPS) {
        if (typeof game.fps !== 'undefined') {
          debugFPS.innerHTML = "FPS: " + game.fps.toFixed(3);
        } else {
          debugFPS.innerHTML = "FPS: N/A";
        }
      }
      this.fpsAnimationFrame = requestAnimationFrame(this.updateFps.bind(this));
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
