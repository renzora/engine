// This is your startup file. Add all your plugins, preload assets and do any engine configurations here.

    assets.preload([
        { name: 'female-01', path: 'assets/img/sprites/characters/female-01.png' },
        { name: 'objectData', path: 'assets/json/objectData.json' },
        { name: 'spriteData', path: 'assets/json/spritesData.json' }
    ],() => {

        plugin.load({ id: 'time', url: 'plugins/time/index.js'});

        plugin.load({
            id: 'audio',
            url: 'plugins/audio/index.js',
            after: function() {
                audio.createChannel('music', localStorage.getItem('music-volume') || audio.defaultVolume);
                audio.setVolume('music', localStorage.getItem('music-volume') || 0.05);
                audio.createChannel('sfx', localStorage.getItem('sfx-volume') || audio.defaultVolume);
                audio.createChannel('ambience', localStorage.getItem('ambience-volume') || 0.5);
            }
        });

        plugin.load({ id: 'particles', url: 'plugins/particles/index.js'});
        plugin.load({ id: 'collision', url: 'plugins/collision/index.js' });
        //plugin.load({ id: 'network', url: 'plugins/network/index.js'});

        const playerSprite = sprite.create({
            id: 'player1',
            isPlayer: true,
            speed: 100,
            animalType: 'female-01',
            canShoot: true,
            targetAim: true
        });

        game.create({
            objectData: assets.use('objectData'),
            spriteData: assets.use('spriteData'),
            player: playerSprite,
            after: function() {

                game.scene(localStorage.getItem('sceneid') || '678ec2d7433aae2deee168ee');

                plugin.load({ id: 'gamepad_plugin', url: 'plugins/gamepad/index.js'});
                plugin.load({ id: 'context_menu_window', url: 'plugins/context_menu/index.html' });
                plugin.load({ id: 'auth_window', url: 'plugins/auth/index.njk' });

                plugin.load({
                    id: 'notif',
                    url: 'plugins/notifs/index.html',
                    after: function() {
                        notif.show("load_success_1", "Press Tab to open the editor", "info");
                        notif.show("load_success_2", "Edit client/init.js to change startup settings", "danger");
                    } 
                });

                plugin.load({ id: 'snow', url: 'plugins/snow/index.js', after: function() { snow.active = true; } });
                plugin.load({ id: 'fireflies', url: 'plugins/fireflies/index.js', after: function() { fireflies.active = true }});

            }
        });

        plugin.load({ id: 'lighting', url: 'plugins/lighting/index.js'});
        plugin.load({ id: 'debug', url: 'plugins/debug/index.html' });
        plugin.load({ id: 'terminal_plugin', url: 'plugins/terminal/index.html'});
        plugin.load({ id: 'effects', url: 'plugins/effects/index.js'});
        plugin.load({ id: 'activity_plugin', url: 'plugins/activity_monitor/index.njk'});

    });