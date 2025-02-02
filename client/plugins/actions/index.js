actions = {
    audioCooldown: 0.5,
    lastPlayedTimesByType: {},
    throttleInterval: 2000,
    lastExecutionTime: 0,
    tooltipActive: true,
    proximityThreshold: 42,

    start() {

    },

    onRender() {
        this.checkForNearbyItems();
    },

        isThrottled: function () {
        const now = Date.now();
        if (now - this.lastExecutionTime < this.throttleInterval) {
            return true;
        }

        this.lastExecutionTime = now;
        return false;
    },

    executeActionWithButton: function (action, config, context, item) {

        const buttonToCheck = config.button || context.button;
        
        if (buttonToCheck) {
            if (this.isButtonPressed(buttonToCheck)) {
                this[action](config, context, item);
            }
        } else {
            this[action](config, context, item);
        }
    }, 

    checkForNearbyItems: function () {
        if (!game.roomData || !game.roomData.items) return
        const sprite = game.mainSprite
        if (!sprite) return
    
        const spriteBoundary = {
            left: sprite.x,
            right: sprite.x + sprite.width,
            top: sprite.y,
            bottom: sprite.y + sprite.height,
        }
    
        let closestItem = null
    
        game.roomData.items
            .filter((item) => {
                const itemX = Math.min(...item.x) * 16
                const itemY = Math.min(...item.y) * 16
                const distanceX = Math.abs(sprite.x - itemX)
                const distanceY = Math.abs(sprite.y - itemY)
    
                return distanceX < this.proximityThreshold && distanceY < this.proximityThreshold
            })
            .forEach((item) => {
                const objectData = game.objectData[item.id]
                if (!objectData || !objectData[0] || !objectData[0].script) return
    
                const scriptData = objectData[0].script
                let script
                if (typeof scriptData === 'string') {
                    script = this.parseYaml(scriptData)
                } else if (typeof scriptData === 'object' && scriptData !== null) {
                    script = scriptData
                } else {
                    return
                }
    
                const itemX = Math.min(...item.x) * 16
                const itemY = Math.min(...item.y) * 16
                const itemWidth = (Math.max(...item.x) - Math.min(...item.x) + 1) * 16
                const itemHeight = (Math.max(...item.y) - Math.min(...item.y) + 1) * 16
    
                const objectBoundary = {
                    left: itemX,
                    right: itemX + itemWidth,
                    top: itemY,
                    bottom: itemY + itemHeight,
                }
    
                const isSpriteInsideObject = (
                    spriteBoundary.right >= objectBoundary.left &&
                    spriteBoundary.left <= objectBoundary.right &&
                    spriteBoundary.bottom >= objectBoundary.top &&
                    spriteBoundary.top <= objectBoundary.bottom
                )
    
                if (isSpriteInsideObject) {
                    closestItem = item
                    const objectType = objectData[0].type || item.id
    
                    if (script.walk && script.walk.tooltip && this.tooltipActive) {
                        const objectName = objectData[0].n || 'Unnamed Object'
                        const buttonName = script.walk.button || 'button'
                        const tooltipText = script.walk.tooltip
                            .replace('{name}', objectName)
                            .replace(
                                '{button}',
                                `<div class="gamepad_button_${buttonName.toLowerCase()}"></div>`
                            )
                        this.tooltip(tooltipText, item, sprite)
                        gamepad.updateButtonImages()
                    }
    
                    if (script.walk && script.walk.scene) {
                        const sceneButton = script.walk.scene.button || script.walk.button
                        if (sceneButton && this.isButtonPressed(sceneButton)) {
                            if (this.isThrottled(sceneButton)) {
                                console.log('Scene change action throttled')
                                return
                            }
                            const activeTime = script.walk.scene.active || "0-24"
                            if (this.isWithinActiveTime(activeTime)) {
                                game.loadScene(script.walk.scene.id)
                                console.log(`Loading scene: ${script.walk.scene.id}`)
                            } else {
                                console.log(`Scene ${script.walk.scene.id} is closed. Active hours are ${activeTime}`)
                            }
                        }
                    }
    
                    if (script.walk && script.walk.speech) {
                        const speechButton = script.walk.speech.button || script.walk.button
                        if (speechButton && this.isButtonPressed(speechButton)) {
                            if (this.isThrottled(speechButton)) {
                                console.log('Speech action throttled')
                                return
                            }
                            if (!script.walk.speech.active || this.isWithinActiveTime(script.walk.speech.active)) {
                                const icon = script.walk.speech.icon || 'self'
                                this.speech(script.walk.speech, item, sprite, icon)
                            } else {
                                console.log(`Speech is unavailable. Active hours are ${script.walk.speech.active}`)
                            }
                        }
                    }
    
                    this.audio(script, item, objectType)
    
                    if (script.walk && script.walk.plugin) {
                        this.plugin(script.walk.plugin, item, objectType)
                    }
    
                    if (!script.walk.button || this.isButtonPressed(script.walk.button)) {
                        for (let action in script.walk) {
                            if (action !== 'button' && action !== 'tooltip' && action !== 'audio' && action !== 'plugin' && action !== 'scene' && action !== 'speech' && this[action] && typeof this[action] === 'function') {
                                this.executeActionWithButton(action, script.walk[action], item, sprite)
                            }
                        }
                    }
                } else {
                    if (item.audioPlaying) {
                        plugin.audio.stopLoopingAudio(script.walk.audio.soundId, 'sfx')
                        item.audioPlaying = false
                    }
                    item.swayTriggered = false
                }
            })
    
        if (!closestItem) {
            this.hideTooltip()
        }
    },    

    plugin: function (config, context, item) {

        const buttonToCheck = config.button || null;
        
        if (!buttonToCheck || this.isButtonPressed(buttonToCheck)) {
            plugin.load({
                id: config.id || 'plugin_window',
                url: config.url || '',
                name: config.name || 'plugin',
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
        const currentHour = plugin.time.hours + (plugint.time.minutes / 60);
        console.log(`Current game time (hours): ${plugin.time.hours}:${plugint.time.minutes}, as decimal: ${currentHour}`);
    
        const [startTime, endTime] = activeTime.split('-').map(time => {
            if (time.includes(':')) {
                const [hours, minutes] = time.split(':').map(Number);
                return hours + (minutes / 60);
            }
            return parseFloat(time);
        });
    
        console.log(`Checking active time range: ${startTime} to ${endTime}`);
    
        if (startTime <= endTime) {
            return currentHour >= startTime && currentHour <= endTime;
        } else {
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
            this.handleRotation(context);
        }
    },

    tooltip: function (htmlContent, context, item) {
        if (!this.tooltipActive) return;
        const sprite = game.mainSprite;
        if (sprite) {
            const spriteScreenX = (sprite.x - camera.cameraX) * game.zoomLevel;
            const spriteScreenY = (sprite.y - camera.cameraY) * game.zoomLevel;
            const spriteCenterX = spriteScreenX + (sprite.width * game.zoomLevel) / 2;
    
            let tooltip = document.getElementById('game_tooltip');
            if (!tooltip) {
                tooltip = document.createElement('div');
                tooltip.id = 'game_tooltip';
                tooltip.className = `
                    absolute px-4 py-2 bg-black bg-opacity-70 text-white 
                    rounded-md pointer-events-none flex items-center gap-2 
                    whitespace-nowrap z-10
                `;
                document.body.appendChild(tooltip);
            }
    
            tooltip.innerHTML = htmlContent; // Render HTML content
            tooltip.style.display = 'flex';
    
            const tooltipWidth = tooltip.offsetWidth;
            tooltip.style.left = `${spriteCenterX - tooltipWidth / 2}px`;
            tooltip.style.top = `${spriteScreenY - 20}px`;
        }
    },
          

speech: function (config, context, item) {
    const speechButton = config.button || null;

    if (speechButton && this.isButtonPressed(speechButton)) {
        if (this.isThrottled(speechButton)) {
            console.log('Speech action throttled');
            return;
        }

        if (config.message && Array.isArray(config.message.message)) {
            if (!item.speechTriggered) {
                game.allowControls = false;

                speech_window.startSpeech(
                    config.message.message,
                    () => { 
                        item.speechTriggered = false;
                        game.allowControls = true;
                    },
                    context.id
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
        const audioBuffer = assets.use(soundId);
        const customCooldown = script.walk.audio.cooldown || this.audioCooldown;
        const currentTime = Date.now();
        const lastPlayedTime = this.lastPlayedTimesByType[objectType] || 0;
        const timeSinceLastPlay = (currentTime - lastPlayedTime) / 1000;
        const buttonToCheck = script.walk.audio.button || null;
        const shouldPlayAudio = (!buttonToCheck && sprite.moving) || (buttonToCheck && this.isButtonPressed(buttonToCheck));
    
        if (audioBuffer && timeSinceLastPlay > customCooldown && shouldPlayAudio) {
            if (!item.audioPlaying) {
                audio.playAudio(soundId, audioBuffer, 'sfx', script.walk.audio.loop || false);
                item.audioPlaying = true;
                this.lastPlayedTimesByType[objectType] = currentTime;
            }
        }
    
        const shouldStopAudio = (!buttonToCheck && !sprite.moving) || (buttonToCheck && !this.isButtonPressed(buttonToCheck));
        if (item.audioPlaying && shouldStopAudio) {
            audio.stopLoopingAudio(soundId, 'sfx');
            item.audioPlaying = false;
        }
    },    

    reward: function (config, context, item) {
        const rewardId = config.id === 'self' ? context.id : config.id;
        
        if (rewardId && config.amount && !item.rewardGiven) {
            ui_inventory_window.addToInventory(rewardId, config.amount);
            item.rewardGiven = true;
        
            setTimeout(() => {
                item.rewardGiven = false;
            }, 100);
            
            if (config.remove) {
                const itemIndex = game.roomData.items.indexOf(item);
                if (itemIndex !== -1) {
                    game.roomData.items.splice(itemIndex, 1);
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

    isButtonPressed: function (button) {
        const buttonMap = {
            'y': 'YButton',
            'x': 'XButton',
            'a': 'AButton',
            'b': 'BButton'
        };
        return input[`is${buttonMap[button]}Held`];
    },

    hideTooltip: function () {
        const tooltip = document.getElementById('game_tooltip');
        if (tooltip) {
            tooltip.style.display = 'none';
        }
    },

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
    
        horseSprite.riderId = playerId;
        game.mainSprite = horseSprite;
        game.setActiveSprite(horseId);
        playerSprite.onHorse = true;
    
        console.log(`${playerId} is now riding ${horseId}`);
    },
    

    dismountHorse: function (horseId) {
        const horseSprite = game.sprites[horseId];
    
        if (!horseSprite) {
            console.log('Horse not found');
            return;
        }
    
        const riderId = horseSprite.riderId;
        if (!riderId) {
            console.log('No rider on this horse');
            return;
        }
    
        horseSprite.riderId = null;
        const playerId = riderId.replace('_riding', '');
        game.sprites[playerId] = game.sprites[riderId];
        delete game.sprites[riderId];
        game.mainSprite = game.sprites[playerId];
        game.setActiveSprite(playerId);
        game.sprites[playerId].visible = true;
    
        console.log(`${playerId} has dismounted from ${horseId}`);
    },


    parseYaml: function(yaml) {
        const lines = yaml.split('\n');
        const result = {};
        let currentObject = result;
        let objectStack = [result];
        let indentStack = [0];
        let previousIndent = 0;
        let lastKey = '';
    
        lines.forEach(line => {
            const cleanLine = line.split('#')[0].trim();
    
            if (cleanLine === '') return;
    
            const indent = line.search(/\S/);
    
            if (indent < previousIndent && objectStack.length > 1) {
                while (indent <= indentStack[indentStack.length - 1]) {
                    objectStack.pop();
                    indentStack.pop();
                }
                currentObject = objectStack[objectStack.length - 1];
            }
    
            if (cleanLine.startsWith('- ')) {
                const listItem = cleanLine.slice(2).trim().replace(/^["']|["']$/g, '');
                if (Array.isArray(currentObject[lastKey])) {
                    currentObject[lastKey].push(listItem);
                } else {
                    currentObject[lastKey] = [listItem];
                }
            } else if (cleanLine.includes(':')) {
                const [rawKey, ...rawValue] = cleanLine.split(':');
                const key = rawKey.trim();
                let value = rawValue.join(':').trim().replace(/^["']|["']$/g, '');
    
                if (value === '') {
                    currentObject[key] = {};
                    objectStack.push(currentObject[key]);
                    currentObject = currentObject[key];
                    indentStack.push(indent);
                } else {
                    currentObject[key] = value;
                }
                lastKey = key;
            }
    
            previousIndent = indent;
        });
    
        return result;
    }
    
};