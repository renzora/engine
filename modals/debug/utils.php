<div data-window='debug_utils_window' class='window window_bg' style='width: 330px;background: #2e2e2e;'>
    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#1a1a1a 1px, transparent 0) !important;'>
        <div class='float-right'>
            <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
        </div>
        <div data-part='title' class='title_bg window_border' style='background: #2e2e2e; color: #ffffff;'>Debug Utilities</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
        <div class='container text-light window_body p-2 text-white' style="height: 400px;">
            <div id="debug_utils_window_tabs">
                <div id="tabs" class="flex border-b border-gray-300">
                    <button class="tab text-white bg-gray-800 hover:bg-gray-700 px-4 py-2 focus:outline-none" data-tab="sprite-tab">Sprite</button>
                    <button class="tab text-white bg-gray-800 hover:bg-gray-700 px-4 py-2 focus:outline-none" data-tab="collisions-tab">Collisions</button>
                    <button class="tab text-white bg-gray-800 hover:bg-gray-700 px-4 py-2 focus:outline-none" data-tab="debug-tools-tab">Tools</button>
                    <button class="tab text-white bg-gray-800 hover:bg-gray-700 px-4 py-2 focus:outline-none" data-tab="enemy-tab">Enemy</button>
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
                    <div>
                        <label for="attack_slider">Attack:</label>
                        <input type="range" id="attack_slider" min="0" max="200" step="1" value="100" onchange="debug_utils_window.updateAttribute('attack', this.value)">
                        <span id="attack_value">100</span>
                    </div>
                    <div>
                        <label for="defense_slider">Defense:</label>
                        <input type="range" id="defense_slider" min="0" max="100" step="1" value="50" onchange="debug_utils_window.updateAttribute('defense', this.value)">
                        <span id="defense_value">50</span>
                    </div>
                    <div>
                        <label for="intensity_slider">Intensity:</label>
                        <input type="range" id="intensity_slider" min="0" max="100" step="1" value="50" onchange="debug_utils_window.updateAttribute('intensity', this.value)">
                        <span id="intensity_value">50</span>
                    </div>
                </div>
                <div class="tab-content p-4 hidden text-white" data-tab-content="enemy-tab">
                    <p>Select Sprite:</p>
                    <select id="enemy_select" class="mb-3" style="color: black;" onchange="debug_utils_window.updateSelectedEnemy(this.value)">
                        <option value="">Select a sprite</option>
                    </select>

                    <div class="mt-1 mb-3">
                        <button class="green_button text-white font-bold py-3 px-4 rounded w-full shadow-md" onclick="debug_utils_window.changeCameraToSelectedSprite()">Change Camera</button>
                    </div>

                    <div>
                        <input type="checkbox" id="is_enemy" onchange="debug_utils_window.updateEnemyAttribute('isEnemy', this.checked)"> Is Enemy
                    </div>

                    <div>
                        <input type="checkbox" id="show_attack_radius" onchange="debug_utils_window.toggleAttackRadius()"> Show Attack Radius
                    </div>

                    <div>
                        <label for="enemy_attack_slider">Attack:</label>
                        <input type="range" id="enemy_attack_slider" min="0" max="200" step="1" value="100" onchange="debug_utils_window.updateEnemyAttribute('attack', this.value)">
                        <span id="enemy_attack_value">100</span>
                    </div>
                    <div>
                        <label for="enemy_maxRadius_slider">Max Range:</label>
                        <input type="range" id="enemy_maxRadius_slider" min="0" max="500" step="1" value="30" onchange="debug_utils_window.updateEnemyAttribute('maxRange', this.value)">
                        <span id="enemy_maxRange_value">30</span>
                    </div>
                    <div>
                        <label for="enemy_defense_slider">Defense:</label>
                        <input type="range" id="enemy_defense_slider" min="0" max="100" step="1" value="50" onchange="debug_utils_window.updateEnemyAttribute('defense', this.value)">
                        <span id="enemy_defense_value">50</span>
                    </div>
                    <div>
                        <label for="enemy_intensity_slider">Intensity:</label>
                        <input type="range" id="enemy_intensity_slider" min="0" max="100" step="1" value="50" onchange="debug_utils_window.updateEnemyAttribute('intensity', this.value)">
                        <span id="enemy_intensity_value">50</span>
                    </div>
                    <div>
                        <label for="enemy_health_slider">Health:</label>
                        <input type="range" id="enemy_health_slider" min="0" max="100" step="1" value="100" onchange="debug_utils_window.updateEnemyAttribute('health', this.value)">
                        <span id="enemy_health_value">100</span>
                    </div>

                    <div>
                        <label for="enemy_speed_slider">Speed:</label>
                        <input type="range" id="enemy_speed_slider" min="0" max="200" step="1" value="100" onchange="debug_utils_window.updateEnemyAttribute('speed', this.value)">
                        <span id="enemy_Speed_value">100</span>
                    </div>

                    <div class="mt-2">
                        <button class="green_button text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md" onclick="debug_utils_window.applyToAllEnemies()">Apply to All</button>
                    </div>

                    <div class="mt-4">Appearance</div>

                    <div>
                        <label for="enemy_body_slider">Body:</label>
                        <input type="range" id="enemy_body_slider" min="0" max="5" step="1" value="1" onchange="debug_utils_window.updateEnemyAttribute('body', this.value)">
                        <span id="enemy_body_value">1</span>
                    </div>
                    <div>
                        <label for="enemy_head_slider">Head:</label>
                        <input type="range" id="enemy_head_slider" min="0" max="5" step="1" value="1" onchange="debug_utils_window.updateEnemyAttribute('head', this.value)">
                        <span id="enemy_head_value">1</span>
                    </div>
                    <div>
                        <label for="enemy_eyes_slider">Eyes:</label>
                        <input type="range" id="enemy_eyes_slider" min="0" max="5" step="1" value="1" onchange="debug_utils_window.updateEnemyAttribute('eyes', this.value)">
                        <span id="enemy_eyes_value">1</span>
                    </div>
                    <div>
                        <label for="enemy_hair_slider">Hair:</label>
                        <input type="range" id="enemy_hair_slider" min="0" max="29" step="1" value="0" onchange="debug_utils_window.updateEnemyAttribute('hair', this.value)">
                        <span id="enemy_hair_value">0</span>
                    </div>
                    <div>
                        <label for="enemy_outfit_slider">Outfit:</label>
                        <input type="range" id="enemy_outfit_slider" min="0" max="5" step="1" value="2" onchange="debug_utils_window.updateEnemyAttribute('outfit', this.value)">
                        <span id="enemy_outfit_value">2</span>
                    </div>
                    <div>
                        <label for="enemy_facial_slider">Facial:</label>
                        <input type="range" id="enemy_facial_slider" min="0" max="3" step="1" value="0" onchange="debug_utils_window.updateEnemyAttribute('facial', this.value)">
                        <span id="enemy_facial_value">0</span>
                    </div>
                    <div>
                        <label for="enemy_hat_slider">Hat:</label>
                        <input type="range" id="enemy_hat_slider" min="0" max="6" step="1" value="4" onchange="debug_utils_window.updateEnemyAttribute('hat', this.value)">
                        <span id="enemy_hat_value">4</span>
                    </div>
                    <div>
                        <label for="enemy_glasses_slider">Glasses:</label>
                        <input type="range" id="enemy_glasses_slider" min="0" max="2" step="1" value="0" onchange="debug_utils_window.updateEnemyAttribute('glasses', this.value)">
                        <span id="enemy_glasses_value">0</span>
                    </div>
                    
                    <div class="mt-4">Position</div>
                    <div>
                        <label for="enemy_x_slider">X:</label>
                        <input type="range" id="enemy_x_slider" min="0" max="500" step="1" value="0" onchange="debug_utils_window.updateEnemyAttribute('x', this.value)">
                        <span id="enemy_x_value">0</span>
                    </div>
                    <div>
                        <label for="enemy_y_slider">Y:</label>
                        <input type="range" id="enemy_y_slider" min="0" max="500" step="1" value="0" onchange="debug_utils_window.updateEnemyAttribute('y', this.value)">
                        <span id="enemy_y_value">0</span>
                    </div>
                    <div>
                        <label for="enemy_top_slider">Top:</label>
                        <input type="range" id="enemy_top_slider" min="0" max="500" step="1" value="0" onchange="debug_utils_window.updateEnemyAttribute('top', this.value)">
                        <span id="enemy_top_value">0</span>
                    </div>
                    <div>
                        <label for="enemy_left_slider">Left:</label>
                        <input type="range" id="enemy_left_slider" min="0" max="500" step="1" value="0" onchange="debug_utils_window.updateEnemyAttribute('left', this.value)">
                        <span id="enemy_left_value">0</span>
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
    showAttackRadius: false,
    selectedEnemy: null,
    start: function() {
        ui.initTabs('debug_utils_window_tabs', 'sprite-tab'); // Initialize the first tab as active
        this.updateDebugInfo();
        this.populateEnemySelect(); // Populate the enemy select box
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
    toggleAttackRadius: function() {
        this.showAttackRadius = !this.showAttackRadius;
        this.renderAttackRadius();
    },
    truncate: function(str, maxLength) {
        return str.length > maxLength ? str.substring(0, maxLength - 3) + '...' : str;
    },
    updateAttribute: function(attribute, value) {
        const sprite = game.mainSprite;
        if (sprite) {
            sprite[attribute] = parseInt(value);
            document.getElementById(attribute + '_value').innerText = value;
        }
    },
    updateEnemyAttribute: function(attribute, value) {
        if (this.selectedEnemy) {
            if (attribute === 'isEnemy') {
                this.selectedEnemy[attribute] = value;
            } else {
                this.selectedEnemy[attribute] = parseInt(value);
                document.getElementById('enemy_' + attribute + '_value').innerText = value;
            }
        }
    },
    updateSelectedEnemy: function(enemyId) {
        this.selectedEnemy = game.sprites[enemyId];
        if (this.selectedEnemy) {
            document.getElementById('is_enemy').checked = this.selectedEnemy.isEnemy;
            document.getElementById('enemy_attack_slider').value = this.selectedEnemy.attack;
            document.getElementById('enemy_attack_value').innerText = this.selectedEnemy.attack;
            document.getElementById('enemy_maxRadius_slider').value = this.selectedEnemy.maxRange;
            document.getElementById('enemy_maxRange_value').innerText = this.selectedEnemy.maxRange;
            document.getElementById('enemy_defense_slider').value = this.selectedEnemy.defense;
            document.getElementById('enemy_defense_value').innerText = this.selectedEnemy.defense;
            document.getElementById('enemy_intensity_slider').value = this.selectedEnemy.intensity;
            document.getElementById('enemy_intensity_value').innerText = this.selectedEnemy.intensity;
            document.getElementById('enemy_health_slider').value = this.selectedEnemy.health;
            document.getElementById('enemy_health_value').innerText = this.selectedEnemy.health;
            document.getElementById('enemy_speed_slider').value = this.selectedEnemy.speed;
            document.getElementById('enemy_Speed_value').innerText = this.selectedEnemy.speed;
            document.getElementById('enemy_x_slider').value = this.selectedEnemy.x;
            document.getElementById('enemy_x_value').innerText = this.selectedEnemy.x;
            document.getElementById('enemy_y_slider').value = this.selectedEnemy.y;
            document.getElementById('enemy_y_value').innerText = this.selectedEnemy.y;
            document.getElementById('enemy_top_slider').value = this.selectedEnemy.top;
            document.getElementById('enemy_top_value').innerText = this.selectedEnemy.top;
            document.getElementById('enemy_left_slider').value = this.selectedEnemy.left;
            document.getElementById('enemy_left_value').innerText = this.selectedEnemy.left;
        }
    },
    populateEnemySelect: function() {
        const enemySelect = document.getElementById('enemy_select');
        const selectedEnemyId = enemySelect.value; // Store the currently selected enemy ID
        enemySelect.innerHTML = '<option value="">Select a sprite</option>';
        for (const id in game.sprites) {
            const sprite = game.sprites[id];
            const option = document.createElement('option');
            option.value = id;
            option.innerText = this.truncate(id, 30); // Truncate the text to 20 characters
            enemySelect.appendChild(option);
        }
        enemySelect.value = selectedEnemyId; // Set the selected value again
    },
    applyToAllEnemies: function() {
        const attackValue = parseInt(document.getElementById('enemy_attack_slider').value);
        const maxRangeValue = parseInt(document.getElementById('enemy_maxRadius_slider').value);
        const defenseValue = parseInt(document.getElementById('enemy_defense_slider').value);
        const intensityValue = parseInt(document.getElementById('enemy_intensity_slider').value);
        const healthValue = parseInt(document.getElementById('enemy_health_slider').value);
        const speedValue = parseInt(document.getElementById('enemy_speed_slider').value);

        for (const id in game.sprites) {
            const sprite = game.sprites[id];
            if (sprite.isEnemy) {
                sprite.attack = attackValue;
                sprite.maxRange = maxRangeValue;
                sprite.defense = defenseValue;
                sprite.intensity = intensityValue;
                sprite.health = healthValue;
                sprite.speed = speedValue;
            }
        }
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
            document.getElementById('attack_slider').value = sprite.attack;
            document.getElementById('defense_slider').value = sprite.defense;
            document.getElementById('intensity_slider').value = sprite.intensity;
            document.getElementById('attack_value').innerText = sprite.attack;
            document.getElementById('defense_value').innerText = sprite.defense;
            document.getElementById('intensity_value').innerText = sprite.intensity;
        } else {
            document.getElementById('sprite_tile_x').innerText = "No sprite found";
            document.getElementById('sprite_tile_y').innerText = "No sprite found";
        }
        document.getElementById('camera_position').innerText = cameraPosition;
        document.getElementById('collisions_info').innerText = collisions;
        document.getElementById('collision_boundaries').innerText = collisionBoundaries;

        // Update enemy select box
        this.populateEnemySelect();
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
    },
    renderAttackRadius: function() {
        if (!this.showAttackRadius) return; // Check if the showAttackRadius is enabled
        for (const id in game.sprites) {
            const sprite = game.sprites[id];
            if (sprite.isEnemy) {
                // Calculate dynamic stop radius based on defense and attack
                const dynamicStopRadius = Math.max(30, 100 - sprite.defense + sprite.attack);

                game.ctx.save();
                game.ctx.strokeStyle = 'rgba(255, 0, 0, 0.5)';
                game.ctx.lineWidth = 2;
                game.ctx.beginPath();
                game.ctx.arc(sprite.x + sprite.width / 2, sprite.y + sprite.height / 2, dynamicStopRadius, 0, 2 * Math.PI);
                game.ctx.stroke();

                // Draw the enemy's name
                game.ctx.fillStyle = 'white';
                game.ctx.font = '10px Arial';
                game.ctx.textAlign = 'center';
                game.ctx.fillText(sprite.id, sprite.x + sprite.width / 2, sprite.y - 20); // Adjust position as needed

                game.ctx.restore();
            }
        }
    },
    changeCameraToSelectedSprite: function() {
        if (this.selectedEnemy) {
            game.setActiveSprite(this.selectedEnemy.id);
        }
    }
};

debug_utils_window.start();

    </script>
</div>
