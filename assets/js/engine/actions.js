const actions = {
    audioCooldown: 0.5, // Cooldown period in seconds
    lastPlayedTimesByType: {}, // Track last played time by object type

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
            bottom: sprite.y + sprite.height
        };
    
        let closestItem = null;
        const proximityThreshold = 100; // Example value, adjust as necessary for your game
    
        game.roomData.items
            .filter(item => {
                const itemX = Math.min(...item.x) * 16;
                const itemY = Math.min(...item.y) * 16;
                const itemWidth = (Math.max(...item.x) - Math.min(...item.x) + 1) * 16;
                const itemHeight = (Math.max(...item.y) - Math.min(...item.y) + 1) * 16;
    
                const distanceX = Math.abs(sprite.x - itemX);
                const distanceY = Math.abs(sprite.y - itemY);
    
                return distanceX < proximityThreshold && distanceY < proximityThreshold;
            })
            .forEach(item => {
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
                    bottom: itemY + itemHeight
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
    
                    // Execute the script actions
                    if (script.walk) {
                        const globalButton = script.walk.button; // Extract the global button
    
                        if (script.walk.tooltip) {
                            const objectName = objectData[0].n || 'Unnamed Object';
                            const tooltipText = script.walk.tooltip.replace('{name}', objectName);
                            this.tooltip(tooltipText, item, sprite);  // Pass modified tooltip text
                        }
    
                        // Handle audio playback
                        this.audio(script, item, objectType);
    
                        // Handle modal popup if present
                        if (script.walk.modal) {
                            this.modal(script.walk.modal, item, objectType);
                        }
    
                        // Execute other actions
                        if (!globalButton || this.isButtonPressed(globalButton)) {
                            for (let action in script.walk) {
                                if (action !== 'button' && action !== 'tooltip' && action !== 'audio' && action !== 'modal' && this[action] && typeof this[action] === 'function') {
                                    this.executeActionWithButton(action, script.walk[action], item, sprite);
                                }
                            }
                        }
                    }
                } else {
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
        if (config.message && Array.isArray(config.message.message)) {
            // Ensure speech hasn't already been triggered
            if (!item.speechTriggered) {
                // Disable player controls while speech is active
                game.allowControls = false;
    
                // Start the speech with the full array of messages
                speech_window.startSpeech(
                    config.message.message,  // Send the full array of messages
                    () => { 
                        item.speechTriggered = false;
                        // Re-enable player controls when speech ends
                        game.allowControls = true;
                    }
                );
    
                // Mark speech as triggered to avoid re-triggering
                item.speechTriggered = true;
            }
        }
    },

    audio: function (script, item, objectType) {
        const sprite = game.mainSprite;
        if (!sprite || !script.walk.audio) return;
    
        const soundId = script.walk.audio.soundId;
        const audioBuffer = assets.load(soundId); // Assuming assets.load loads the audio buffer
    
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
