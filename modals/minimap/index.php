<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='minimap_window' class='fixed bottom-0 right-0 window window_bg' style='width: 200px; height: 230px; background: #57476d; margin: 0 12px 30px 0;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#272031 1px, transparent 0) !important;'>
      <div class='float-right mt-1'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #57476d; color: #ede8d6;'>Mini Map</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2'>
        <canvas id="minimap" width="200" height="200"></canvas>
      </div>
    </div>

    <script>
var minimap_window = {
    start: function() {
        this.initMiniMap();
        this.updateMiniMapPosition(); // Start updating the mini-map position
    },
    unmount: function() {
        window.removeEventListener('resize', this.resizeMiniMap.bind(this));
    },

    miniMapScale: 0.3,
    miniMapCanvas: null,
    miniMapCtx: null,
    miniMapBackground: null,
    dragging: false,
    dragStartX: 0,
    dragStartY: 0,

    initMiniMap: function() {
        this.miniMapCanvas = document.getElementById('minimap');
        this.miniMapCtx = this.miniMapCanvas.getContext('2d');
        this.miniMapCanvas.addEventListener('mousedown', this.handleMiniMapMouseDown.bind(this));
        this.miniMapCanvas.addEventListener('mousemove', this.handleMiniMapMouseMove.bind(this));
        this.miniMapCanvas.addEventListener('mouseup', this.handleMiniMapMouseUp.bind(this));
        this.miniMapCanvas.addEventListener('mouseleave', this.handleMiniMapMouseUp.bind(this)); 
        this.miniMapCanvas.style.imageRendering = 'pixelated'; 
        this.resizeMiniMap(); 
        this.renderMiniMapBackground(); 
        this.updateMiniMap(); 

        // Add general mouseup listener to handle the case when mouse is released outside the mini-map
        document.addEventListener('mouseup', this.handleMiniMapMouseUp.bind(this));
    },

    resizeMiniMap: function() {
        const container = this.miniMapCanvas.parentElement;
        this.miniMapCanvas.width = container.clientWidth;
        this.miniMapCanvas.height = container.clientHeight;
        this.renderMiniMapBackground(); 
        this.updateMiniMap(); 
    },

    renderMiniMapBackground: function() {
        const scale = this.miniMapScale;
        const ctx = this.miniMapCtx;

        ctx.clearRect(0, 0, this.miniMapCanvas.width, this.miniMapCanvas.height);

        ctx.save();
        ctx.scale(scale, scale);
        ctx.imageSmoothingEnabled = false;
        this.renderScene(ctx);
        ctx.restore();

        this.miniMapBackground = ctx.getImageData(0, 0, this.miniMapCanvas.width, this.miniMapCanvas.height);
    },

    updateMiniMap: function() {
        const ctx = this.miniMapCtx;
        const scale = this.miniMapScale;

        ctx.putImageData(this.miniMapBackground, 0, 0);

        const player = game.sprites[game.playerid];
        if (player) {
            ctx.fillStyle = 'red';
            ctx.fillRect(player.x * scale, player.y * scale, 3, 3);
        }

        const cameraX = game.cameraX * scale;
        const cameraY = game.cameraY * scale;
        const cameraWidth = game.canvas.width * scale / game.zoomLevel;
        const cameraHeight = game.canvas.height * scale / game.zoomLevel;
        ctx.strokeStyle = 'white';
        ctx.lineWidth = 2;
        ctx.strokeRect(cameraX, cameraY, cameraWidth, cameraHeight);
    },

    renderScene: function(ctx) {
        game.grid();

        if (game.roomData && game.roomData.items) {
            game.roomData.items.forEach(roomItem => {
                const itemData = game.objectData[roomItem.id];
                if (itemData && itemData.length > 0) {
                    const tileData = itemData[0];
                    const xCoordinates = roomItem.x || [];
                    const yCoordinates = roomItem.y || [];

                    let index = 0;

                    for (let y = Math.min(...yCoordinates); y <= Math.max(...yCoordinates); y++) {
                        for (let x = Math.min(...xCoordinates); x <= Math.max(...xCoordinates); x++) {
                            const posX = Math.floor(x * 16);
                            const posY = Math.floor(y * 16);

                            let tileFrameIndex;
                            if (tileData.d) {
                                const currentFrame = tileData.currentFrame || 0;
                                tileFrameIndex = Array.isArray(tileData.i) ? tileData.i[(currentFrame + index) % tileData.i.length] : tileData.i;
                            } else {
                                tileFrameIndex = tileData.i[index];
                            }

                            const srcX = (tileFrameIndex % 150) * 16;
                            const srcY = Math.floor(tileFrameIndex / 150) * 16;

                            ctx.drawImage(assets.load(tileData.t), srcX, srcY, 16, 16, posX, posY, 16, 16);

                            index++;
                        }
                    }
                }
            });
        }
    },

    handleMiniMapMouseDown: function(event) {
        event.preventDefault();
        document.body.style.userSelect = 'none'; // Disable text selection
        const rect = this.miniMapCanvas.getBoundingClientRect();
        const clickX = event.clientX - rect.left;
        const clickY = event.clientY - rect.top;

        const scale = this.miniMapScale;
        const cameraX = game.cameraX * scale;
        const cameraY = game.cameraY * scale;
        const cameraWidth = game.canvas.width * scale / game.zoomLevel;
        const cameraHeight = game.canvas.height * scale / game.zoomLevel;

        if (clickX >= cameraX && clickX <= cameraX + cameraWidth && clickY >= cameraY && clickY <= cameraY + cameraHeight) {
            this.dragging = true;
            this.dragStartX = clickX;
            this.dragStartY = clickY;
            game.activeCamera = false; 
            this.miniMapCanvas.style.cursor = 'grabbing'; 
        }
    },

    handleMiniMapMouseMove: function(event) {
        if (this.dragging) {
            event.preventDefault();
        }
        const rect = this.miniMapCanvas.getBoundingClientRect();
        const currentX = event.clientX - rect.left;
        const currentY = event.clientY - rect.top;
        const scale = this.miniMapScale;
        const cameraX = game.cameraX * scale;
        const cameraY = game.cameraY * scale;
        const cameraWidth = game.canvas.width * scale / game.zoomLevel;
        const cameraHeight = game.canvas.height * scale / game.zoomLevel;

        if (!this.dragging) {
            if (currentX >= cameraX && currentX <= cameraX + cameraWidth && currentY >= cameraY && currentY <= cameraY + cameraHeight) {
                this.miniMapCanvas.style.cursor = 'grab';
            } else {
                this.miniMapCanvas.style.cursor = 'default';
            }
            return;
        }

        const deltaX = currentX - this.dragStartX;
        const deltaY = currentY - this.dragStartY;

        game.cameraX += deltaX / scale;
        game.cameraY += deltaY / scale;

        game.cameraX = Math.max(0, Math.min(game.cameraX, game.worldWidth - (game.canvas.width / game.zoomLevel)));
        game.cameraY = Math.max(0, Math.min(game.cameraY, game.worldHeight - (game.canvas.height / game.zoomLevel)));

        this.updateMiniMap();

        this.dragStartX = currentX;
        this.dragStartY = currentY;
    },

    handleMiniMapMouseUp: function() {
        if (this.dragging) {
            this.dragging = false;
            game.activeCamera = true; 
            this.miniMapCanvas.style.cursor = 'grab'; 
            document.body.style.userSelect = ''; // Re-enable text selection
        }
    },

    updateMiniMapPosition: function() {
        if (this.miniMapCanvas) {
            this.updateMiniMap();
        }
        requestAnimationFrame(this.updateMiniMapPosition.bind(this));
    }
}

minimap_window.start();
</script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
