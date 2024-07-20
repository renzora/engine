<div data-window='debug_utils_window' class='window window_bg' style='width: 300px;background: #2e2e2e;'>
    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#1a1a1a 1px, transparent 0) !important;'>
        <div class='float-right'>
            <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
        </div>
        <div data-part='title' class='title_bg window_border' style='background: #2e2e2e; color: #ffffff;'>Debug Utilities</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
        <div class='container text-light window_body p-2 text-white' style="height: 300px;">
            <div id="debug_utils_window_tabs">
                <div id="tabs" class="flex border-b border-gray-300">
                    <button class="tab text-white bg-gray-800 hover:bg-gray-700 px-4 py-2 focus:outline-none" data-tab="sprite-tab">Sprite</button>
                    <button class="tab text-white bg-gray-800 hover:bg-gray-700 px-4 py-2 focus:outline-none" data-tab="collisions-tab">Collisions</button>
                    <button class="tab text-white bg-gray-800 hover:bg-gray-700 px-4 py-2 focus:outline-none" data-tab="debug-tools-tab">Tools</button>
                </div>
                <div class="tab-content p-4 hidden text-white" data-tab-content="sprite-tab">
                    <p>Nearest Walkable Tile: <span id="nearest_tile"></span></p>
                    <p>Current Sprite Position: <span id="sprite_position"></span></p>
                    <p>Sprite Tile X: <span id="sprite_tile_x"></span></p>
                    <p>Sprite Tile Y: <span id="sprite_tile_y"></span></p>
                </div>
                <div class="tab-content p-4 hidden text-white" data-tab-content="collisions-tab">
                    <p>Collisions: <span id="collisions_info"></span></p>
                    <p>Collision Boundaries: <span id="collision_boundaries"></span></p>
                </div>
                <div class="tab-content p-4 hidden text-white" data-tab-content="debug-tools-tab">
                    <p>Camera Position: <span id="camera_position"></span></p>
                    <div>
                        <input type="checkbox" id="show_collision_boundaries" onchange="debug_utils_window.toggleCollisionBoundaries()" checked> Show Collision Boundaries
                    </div>
                    <div>
                        <input type="checkbox" id="show_walkable_tiles" onchange="debug_utils_window.toggleWalkableTiles()" checked> Show Nearest Walkable Tile
                    </div>
                </div>
            </div>
        </div>
    </div>
    <script>
        var debug_utils_window = {
            interval: null,
            showCollisionBoundaries: true,
            showWalkableTiles: true,
            start: function() {
                ui.initTabs('debug_utils_window_tabs', 'sprite-tab'); // Initialize the first tab as active
                this.updateDebugInfo();
                this.interval = setInterval(this.updateDebugInfo.bind(this), 1000); // Update every second
            },
            unmount: function() {
                clearInterval(this.interval);
            },
            toggleCollisionBoundaries: function() {
                this.showCollisionBoundaries = !this.showCollisionBoundaries;
            },
            toggleWalkableTiles: function() {
                this.showWalkableTiles = !this.showWalkableTiles;
            },
            updateDebugInfo: function() {
                const nearestTiles = this.getNearestWalkableTiles();
                const spritePosition = this.getCurrentSpritePosition();
                const cameraPosition = this.getCameraPosition();
                const collisions = this.getCollisions();
                const collisionBoundaries = this.getCollisionBoundaries();
                const sprite = game.mainSprite;

                document.getElementById('nearest_tile').innerText = nearestTiles.join(', ');
                document.getElementById('sprite_position').innerText = spritePosition;
                if (sprite) {
                    const tileX = Math.round(sprite.x / 16);
                    const tileY = Math.round(sprite.y / 16);
                    document.getElementById('sprite_tile_x').innerText = tileX;
                    document.getElementById('sprite_tile_y').innerText = tileY;
                } else {
                    document.getElementById('sprite_tile_x').innerText = "No sprite found";
                    document.getElementById('sprite_tile_y').innerText = "No sprite found";
                }
                document.getElementById('camera_position').innerText = cameraPosition;
                document.getElementById('collisions_info').innerText = collisions;
                document.getElementById('collision_boundaries').innerText = collisionBoundaries;
            },
            getNearestWalkableTiles: function() {
                const sprite = game.mainSprite;
                if (!sprite) return ["No sprite found"];
                const gridX = Math.round(sprite.x / 16);
                const gridY = Math.round(sprite.y / 16);
                const directions = [
                    { x: 0, y: -1 }, // N
                    { x: 1, y: 0 },  // E
                    { x: 0, y: 1 },  // S
                    { x: -1, y: 0 }  // W
                ];
                const walkableTiles = [];
                for (const direction of directions) {
                    const newX = gridX + direction.x;
                    const newY = gridY + direction.y;
                    if (game.isTileWalkable(newX, newY) && !collision.check(newX * 16, newY * 16, sprite)) {
                        walkableTiles.push(`(${newX}, ${newY})`);
                    }
                }
                return walkableTiles.length > 0 ? walkableTiles : ["No walkable tile found"];
            },
            getCurrentSpritePosition: function() {
                const sprite = game.mainSprite;
                if (!sprite) return "No sprite found";
                return `Top: ${sprite.y}, Left: ${sprite.x}`;
            },
            getCameraPosition: function() {
                return `X: ${camera.cameraX}, Y: ${camera.cameraY}`;
            },
            getCollisions: function() {
                const collisions = [];
                for (const id in game.sprites) {
                    const sprite = game.sprites[id];
                    if (collision.check(sprite.x, sprite.y, sprite)) {
                        collisions.push(`Sprite ${id} at (${sprite.x}, ${sprite.y})`);
                    }
                }
                return collisions.length > 0 ? collisions.join(", ") : "No collisions";
            },
            getCollisionBoundaries: function() {
                const boundaries = [];
                for (const id in game.sprites) {
                    const sprite = game.sprites[id];
                    const boundary = {
                        left: sprite.x,
                        right: sprite.x + sprite.width,
                        top: sprite.y,
                        bottom: sprite.y + sprite.height
                    };
                    boundaries.push(`Sprite ${id}: ${JSON.stringify(boundary)}`);
                }
                return boundaries.join("; ");
            },
            renderCollisionBoundaries: function() {
                for (const id in game.sprites) {
                    const sprite = game.sprites[id];
                    game.ctx.strokeStyle = 'red';
                    game.ctx.lineWidth = 1;
                    game.ctx.strokeRect(sprite.x, sprite.y, sprite.width, sprite.height);
                }
            },
            renderNearestWalkableTile: function() {
                for (const id in game.sprites) {
                    const sprite = game.sprites[id];
                    const gridX = Math.round(sprite.x / 16);
                    const gridY = Math.round(sprite.y / 16);

                    // Check each tile around the sprite for collisions
                    const directions = [
                        { x: 0, y: -1 }, // N
                        { x: 1, y: 0 },  // E
                        { x: 0, y: 1 },  // S
                        { x: -1, y: 0 }  // W
                    ];

                    directions.forEach(direction => {
                        const newX = gridX + direction.x;
                        const newY = gridY + direction.y;

                        // Check if there's a collision at this tile
                        const collisionDetected = collision.check(newX * 16, newY * 16, sprite);
                        const isWalkable = game.isTileWalkable(newX, newY);

                        const posX = newX * 16;
                        const posY = newY * 16;

                        if (collisionDetected || !isWalkable) {
                            // Draw the non-walkable tile in red
                            game.ctx.fillStyle = 'rgba(255, 0, 0, 0.5)';
                        } else {
                            // Draw the walkable tile in green
                            game.ctx.fillStyle = 'rgba(0, 255, 0, 0.5)';
                        }

                        game.ctx.fillRect(posX, posY, 16, 16);

                        // Draw the tile data (x, y, and top-left position) in very small text
                        game.ctx.fillStyle = 'white';
                        game.ctx.font = '2px Tahoma'; // Very small font size
                        game.ctx.textAlign = 'left';
                        game.ctx.textBaseline = 'top';
                        game.ctx.fillText(`x:${newX}`, posX + 1, posY + 1);
                        game.ctx.fillText(`y:${newY}`, posX + 1, posY + 3);
                        game.ctx.fillText(`${posX},${posY}`, posX + 1, posY + 6);
                    });
                }
            }
        };

        debug_utils_window.start();
    </script>
</div>
