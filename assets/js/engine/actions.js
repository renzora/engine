const actions = {
    audioCooldown: 0.5, // Cooldown period in seconds
    lastPlayedTimesByType: {}, // Track last played time by object type
    throttleInterval: 2000, // Global throttle interval in milliseconds (2 seconds)
    lastExecutionTime: 0, // Tracks the last execution time for any action

        isThrottled: function () {
        const now = Date.now();
        if (now - this.lastExecutionTime < this.throttleInterval) {
            return true; // Throttled
        }

        this.lastExecutionTime = now; // Update the last executed time
        return false; // Not throttled
    },

    // Central handler to execute actions that may require a button press
    executeActionWithButton: function (action, config, context, item) {
        // Check for individual button first, then fall back to the global button
        const buttonToCheck = config.button || context.button;
        
        if (buttonToCheck) {
            if (this.isButtonPressed(buttonToCheck)) {
                this[action](config, context, item); // Run the action only if the button is pressed
            }
        } else {
            this[action](config, context, item); // If no button, run automatically
        }
    }, 

checkForNearbyItems: function () {
    if (!game.roomData || !game.roomData.items) return;
    const sprite = game.mainSprite;
    if (!sprite) return;

    const spriteBoundary = {
        left: sprite.x,
        right: sprite.x + sprite.width,
        top: sprite.y,
        bottom: sprite.y + sprite.height,
    };

    let closestItem = null;
    const proximityThreshold = 100; // Example value, adjust as necessary for your game

    game.roomData.items
        .filter((item) => {
            const itemX = Math.min(...item.x) * 16;
            const itemY = Math.min(...item.y) * 16;
            const itemWidth = (Math.max(...item.x) - Math.min(...item.x) + 1) * 16;
            const itemHeight = (Math.max(...item.y) - Math.min(...item.y) + 1) * 16;

            const distanceX = Math.abs(sprite.x - itemX);
            const distanceY = Math.abs(sprite.y - itemY);

            return distanceX < proximityThreshold && distanceY < proximityThreshold;
        })
        .forEach((item) => {
            const objectData = game.objectData[item.id];
            if (!objectData || !objectData[0] || !objectData[0].script) return;

            const scriptData = objectData[0].script;

            let script;
            if (typeof scriptData === 'string') {
                script = utils.parseYaml(scriptData);
            } else if (typeof scriptData === 'object' && scriptData !== null) {
                script = scriptData;
            } else {
                return; // Exit early if script is invalid
            }

            const itemX = Math.min(...item.x) * 16;
            const itemY = Math.min(...item.y) * 16;
            const itemWidth = (Math.max(...item.x) - Math.min(...item.x) + 1) * 16;
            const itemHeight = (Math.max(...item.y) - Math.min(...item.y) + 1) * 16;

            const objectBoundary = {
                left: itemX,
                right: itemX + itemWidth,
                top: itemY,
                bottom: itemY + itemHeight,
            };

            const isSpriteInsideObject = (
                spriteBoundary.right >= objectBoundary.left &&
                spriteBoundary.left <= objectBoundary.right &&
                spriteBoundary.bottom >= objectBoundary.top &&
                spriteBoundary.top <= objectBoundary.bottom
            );

            if (isSpriteInsideObject) {
                closestItem = item;

                const objectType = objectData[0].type || item.id;

                // Handle tooltip
                if (script.walk && script.walk.tooltip) {
                    const objectName = objectData[0].n || 'Unnamed Object';
                    const tooltipText = script.walk.tooltip.replace('{name}', objectName);
                    this.tooltip(tooltipText, item, sprite); // Pass modified tooltip text
                }

                // Scene change logic with button throttling
                if (script.walk && script.walk.scene) {
                    const sceneButton = script.walk.scene.button || script.walk.button; // Use scene-specific or global button
                    if (sceneButton && this.isButtonPressed(sceneButton)) {
                        if (this.isThrottled(sceneButton)) {
                            console.log('Scene change action throttled');
                            return;
                        }

                        const activeTime = script.walk.scene.active || "0-24"; // Default active time
                        if (this.isWithinActiveTime(activeTime)) {
                            game.loadScene(script.walk.scene.id);
                            console.log(`Loading scene: ${script.walk.scene.id}`);
                        } else {
                            console.log(`Scene ${script.walk.scene.id} is closed. Active hours are ${activeTime}`);
                        }
                    }
                }

                // Handle speech with button throttling
                if (script.walk && script.walk.speech) {
                    const speechButton = script.walk.speech.button || script.walk.button; // Use speech-specific or global button
                    if (speechButton && this.isButtonPressed(speechButton)) {
                        if (this.isThrottled(speechButton)) {
                            console.log('Speech action throttled');
                            return;
                        }

                        if (!script.walk.speech.active || this.isWithinActiveTime(script.walk.speech.active)) {
                            const icon = script.walk.speech.icon || 'self'; // Get the icon field, default to 'self'
                            this.speech(script.walk.speech, item, sprite, icon); // Pass the icon to the speech function
                        } else {
                            console.log(`Speech is unavailable. Active hours are ${script.walk.speech.active}`);
                        }
                    }
                }

                // Handle audio playback
                this.audio(script, item, objectType);

                // Handle modal popup if present
                if (script.walk && script.walk.modal) {
                    this.modal(script.walk.modal, item, objectType);
                }

                // Execute other actions
                if (!script.walk.button || this.isButtonPressed(script.walk.button)) {
                    for (let action in script.walk) {
                        if (action !== 'button' && action !== 'tooltip' && action !== 'audio' && action !== 'modal' && action !== 'scene' && action !== 'speech' && this[action] && typeof this[action] === 'function') {
                            this.executeActionWithButton(action, script.walk[action], item, sprite);
                        }
                    }
                }
            } else {
                // Stop any active audio if the sprite moves away from the item
                if (item.audioPlaying) {
                    audio.stopLoopingAudio(script.walk.audio.soundId, 'sfx');
                    item.audioPlaying = false;
                }
                item.swayTriggered = false;
            }
        });

    // Hide tooltip if no item is close
    if (!closestItem) {
        this.hideTooltip();
    }
},

    modal: function (config, context, item) {
        // Check if a button is required to trigger the modal
        const buttonToCheck = config.button || null;
        
        // If no button is required or the required button is pressed, open the modal
        if (!buttonToCheck || this.isButtonPressed(buttonToCheck)) {
            modal.load({
                id: config.id || 'modal_window',
                url: config.url || '',
                name: config.name || 'Modal',
                drag: config.drag || false,
                reload: config.reload || false,
            });
        }
    },

    handleClosedTime: function (item) {
        if (item.audioPlaying) {
            audio.stopLoopingAudio(script.walk.audio.soundId, 'sfx');
            item.audioPlaying = false;
        }
        item.swayTriggered = false;
    },

    isWithinActiveTime: function(activeTime) {
        const currentHour = game.gameTime.hours + (game.gameTime.minutes / 60);  // Convert current time to decimal hours
        console.log(`Current game time (hours): ${game.gameTime.hours}:${game.gameTime.minutes}, as decimal: ${currentHour}`);
    
        const [startTime, endTime] = activeTime.split('-').map(time => {
            if (time.includes(':')) {
                const [hours, minutes] = time.split(':').map(Number);
                return hours + (minutes / 60);  // Convert to decimal hours
            }
            return parseFloat(time);  // Assume it's in "H-H" format
        });
    
        console.log(`Checking active time range: ${startTime} to ${endTime}`);
    
        if (startTime <= endTime) {
            // Regular time range (e.g., 8:00-17:00)
            return currentHour >= startTime && currentHour <= endTime;
        } else {
            // Spanning midnight (e.g., 17:01-7:59)
            // We check for hours after startTime OR before endTime
            const inActiveRange = currentHour >= startTime || currentHour <= endTime;
            console.log(`Midnight-spanning time range check: ${inActiveRange}`);
            return inActiveRange;
        }
    },      

    sway: function (config, context, item) {
        if (context && !context.swayTriggered) {
            context.swayTriggered = true;
            context.isRotating = true;
            context.rotationElapsed = 0;
            this.handleRotation(context); // Ensure handleRotation is properly defined
        }
    },

    tooltip: function (config, context, item) {
        const sprite = game.mainSprite;
        if (sprite) {
            const spriteScreenX = (sprite.x - camera.cameraX) * game.zoomLevel;
            const spriteScreenY = (sprite.y - camera.cameraY) * game.zoomLevel;
            const spriteCenterX = spriteScreenX + (sprite.width * game.zoomLevel) / 2;
    
            // Replace placeholder {name} with the actual object name from context or item
            let tooltipText = config.replace('{name}', context.name || item.name || 'Object');
    
            this.showTooltip(tooltipText, spriteCenterX, spriteScreenY);
    
            const tooltip = document.getElementById('game_tooltip');
            if (tooltip) {
                const tooltipWidth = tooltip.offsetWidth;
                const centeredX = spriteCenterX - (tooltipWidth / 2);
                tooltip.style.left = `${centeredX}px`;
                tooltip.style.top = `${spriteScreenY}px`;
            }
        }
    },    

speech: function (config, context, item) {
    const speechButton = config.button || null; // Use speech-specific button or null if no button is required

    if (speechButton && this.isButtonPressed(speechButton)) {
        if (this.isThrottled(speechButton)) {
            console.log('Speech action throttled');
            return;
        }

        if (config.message && Array.isArray(config.message.message)) {
            if (!item.speechTriggered) {
                game.allowControls = false;

                // Start the speech, passing context.id as the icon
                speech_window.startSpeech(
                    config.message.message, // Send the full array of messages
                    () => { 
                        item.speechTriggered = false;
                        game.allowControls = true;
                    },
                    context.id // Pass the object's id (context.id) as the icon
                );

                item.speechTriggered = true;
            }
        }
    } else {
        console.log('Speech button not pressed or not defined');
    }
},

    
    
    audio: function (script, item, objectType) {
        const sprite = game.mainSprite;
        if (!sprite || !script.walk.audio) return;
    
        const soundId = script.walk.audio.soundId;
        const audioBuffer = assets.use(soundId); // Assuming assets.load loads the audio buffer
    
        // Use the custom cooldown if provided, otherwise default to audioCooldown
        const customCooldown = script.walk.audio.cooldown || this.audioCooldown;
    
        // Add cooldown check based on object type
        const currentTime = Date.now();
        const lastPlayedTime = this.lastPlayedTimesByType[objectType] || 0;
        const timeSinceLastPlay = (currentTime - lastPlayedTime) / 1000; // Convert to seconds
    
        // Check if a button is defined
        const buttonToCheck = script.walk.audio.button || null;
    
        // Play audio if button is pressed OR sprite is moving and no button is required
        const shouldPlayAudio = (!buttonToCheck && sprite.moving) || (buttonToCheck && this.isButtonPressed(buttonToCheck));
    
        if (audioBuffer && timeSinceLastPlay > customCooldown && shouldPlayAudio) {
            // Play the audio if it’s not already playing
            if (!item.audioPlaying) {
                audio.playAudio(soundId, audioBuffer, 'sfx', script.walk.audio.loop || false);
                item.audioPlaying = true;
                this.lastPlayedTimesByType[objectType] = currentTime; // Update last played time by object type
            }
        }
    
        // Stop audio if sprite stops moving or button is released (if applicable)
        const shouldStopAudio = (!buttonToCheck && !sprite.moving) || (buttonToCheck && !this.isButtonPressed(buttonToCheck));
        if (item.audioPlaying && shouldStopAudio) {
            audio.stopLoopingAudio(soundId, 'sfx');
            item.audioPlaying = false;
        }
    },    

    reward: function (config, context, item) {
        // If the id is 'self', use the context's id as the reward id
        const rewardId = config.id === 'self' ? context.id : config.id;
        
        // Proceed if rewardId and amount are valid, and if the reward hasn't been given yet
        if (rewardId && config.amount && !item.rewardGiven) {
            // Add the item (or self) to the inventory
            ui_inventory_window.addToInventory(rewardId, config.amount);
            item.rewardGiven = true;
        
            // Reset reward after a delay or condition
            setTimeout(() => {
                item.rewardGiven = false; // Reset after 5 seconds (example)
            }, 100);
            
            // Check if the remove property is set to true
            if (config.remove) {
                // Remove the item from the game, for example, by removing it from the room data
                const itemIndex = game.roomData.items.indexOf(item);
                if (itemIndex !== -1) {
                    game.roomData.items.splice(itemIndex, 1); // Remove the item from the game room
                }
            }
        }
    },
    
    random: function(config, context, item) {
        console.log("random function for object");
    },

    another: function(config, context, item) {
        console.log("another function for object");
    },

    silly: function(config, context, item) {
        console.log("silly function for object");
    },

    // Utility to check if a required button is pressed
    isButtonPressed: function (button) {
        const buttonMap = {
            'y': 'YButton',
            'x': 'XButton',
            'a': 'AButton',
            'b': 'BButton'
        };
        return input[`is${buttonMap[button]}Held`]; // Check if the specified button is held
    },

    showTooltip: function (text, x, y) {
        let tooltip = document.getElementById('game_tooltip');
        if (!tooltip) {
            tooltip = document.createElement('div');
            tooltip.id = 'game_tooltip';
            tooltip.style.position = 'absolute';
            tooltip.style.padding = '5px';
            tooltip.style.backgroundColor = 'rgba(0, 0, 0, 0.7)';
            tooltip.style.color = 'white';
            tooltip.style.borderRadius = '5px';
            tooltip.style.pointerEvents = 'none';
            tooltip.style.zIndex = '10';
            tooltip.style.whiteSpace = 'nowrap';
            document.body.appendChild(tooltip);
        }

        tooltip.innerText = text;
        tooltip.style.display = 'block';

        const tooltipWidth = tooltip.offsetWidth;
        tooltip.style.left = `${x - (tooltipWidth / 2)}px`;
        tooltip.style.top = `${y - 20}px`;
    },

    hideTooltip: function () {
        const tooltip = document.getElementById('game_tooltip');
        if (tooltip) {
            tooltip.style.display = 'none';
        }
    },

    // Ensure the handleRotation function is defined and accessible
    handleRotation: function (context) {
        let baseSwayAngle = Math.PI / 12;
        let directionMultiplier = 1;
        const sprite = game.sprites[game.playerid];

        if (sprite) {
            if (sprite.direction === 'left' || sprite.direction === 'W') {
                directionMultiplier = -1;
            } else if (sprite.direction === 'right' || sprite.direction === 'E') {
                directionMultiplier = 1;
            }

            const maxSwayAngle = baseSwayAngle + (Math.random() * Math.PI / 24) * directionMultiplier;
            const totalRotationDuration = 150;
            const recoveryTime = 300;
            const elapsedTime = context.rotationElapsed || 0;
            context.rotationElapsed = elapsedTime + game.deltaTime;

            let sway = 0;
            if (elapsedTime < totalRotationDuration) {
                sway = directionMultiplier * Math.sin((elapsedTime / totalRotationDuration) * (Math.PI / 2)) * maxSwayAngle;
            } else if (elapsedTime < totalRotationDuration + recoveryTime) {
                const recoveryElapsed = elapsedTime - totalRotationDuration;
                sway = directionMultiplier * Math.cos((recoveryElapsed / recoveryTime) * (Math.PI / 2)) * maxSwayAngle;
            }

            context.rotation = sway;

            if (elapsedTime >= totalRotationDuration + recoveryTime) {
                context.isRotating = false;
                context.rotationElapsed = 0;
                context.rotation = 0;
            }
        }
    },
    
    mountHorse: function (playerId, horseId) {
        const playerSprite = game.sprites[playerId];
        const horseSprite = game.sprites[horseId];
    
        if (!playerSprite || !horseSprite) {
            console.log('Player or horse not found');
            return;
        }
    
        // Set the horse's riderId to the player ID
        horseSprite.riderId = playerId;
    
        // Set the active sprite to the horse so the player controls the horse
        game.mainSprite = horseSprite;
        game.setActiveSprite(horseId); // Set the active sprite to the horse
    
        // Update the player's sprite position to match the horse's position continuously
        playerSprite.onHorse = true; // Add a flag to indicate that the player is mounted
    
        console.log(`${playerId} is now riding ${horseId}`);
    },
    

    dismountHorse: function (horseId) {
        const horseSprite = game.sprites[horseId];
    
        if (!horseSprite) {
            console.log('Horse not found');
            return;
        }
    
        // Get the rider's ID, which should be suffixed with "_riding"
        const riderId = horseSprite.riderId;
        if (!riderId) {
            console.log('No rider on this horse');
            return;
        }
    
        // Remove the riderId from the horse sprite
        horseSprite.riderId = null;
    
        // Restore the original playerId by removing the "_riding" suffix
        const playerId = riderId.replace('_riding', '');
        game.sprites[playerId] = game.sprites[riderId]; // Restore the original player sprite
        delete game.sprites[riderId]; // Remove the "_riding" entry
    
        // Set the active sprite back to the player
        game.mainSprite = game.sprites[playerId];
        game.setActiveSprite(playerId); // Switch back to the player sprite
    
        // Show the player sprite again when dismounting
        game.sprites[playerId].visible = true; // Ensure the player is visible after dismounting
    
        console.log(`${playerId} has dismounted from ${horseId}`);
    }
    
};
