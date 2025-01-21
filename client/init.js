// This is your startup file. Add all your plugins, preload assets and do any engine configurations here.

    assets.preload([
        { name: 'female-01', path: 'assets/img/sprites/characters/female-01.png' },
        { name: 'objectData', path: 'assets/json/objectData.json' },
        { name: 'spriteData', path: 'assets/json/spritesData.json' }
    ],() => {

        const playerSprite = sprite.create({
            id: game.playerid,
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

                utils.gameTime.hours = 10;

                game.scene(localStorage.getItem('sceneid') || '678ec2d7433aae2deee168ee');
                audio.createChannel('music', localStorage.getItem('music-volume') || audio.defaultVolume);
                audio.setVolume('music', localStorage.getItem('music-volume') || 0.05);
                audio.createChannel('sfx', localStorage.getItem('sfx-volume') || audio.defaultVolume);
                audio.createChannel('ambience', localStorage.getItem('ambience-volume') || 0.5);

                plugin.load({
                    id: 'gamepad_plugin',
                    url: 'plugins/gamepad/index.js'
                });


                plugin.load({ id: 'context_menu_window', url: 'plugins/context_menu/index.html' });
                plugin.load({ id: 'debug_window', url: 'plugins/dev/index.html' });
                plugin.load({ id: 'auth_window', url: 'plugins/auth/index.njk' });

                plugin.load({
                    id: 'notif',
                    url: 'plugins/notifs/index.html',
                    after: function() {
                        notif.show("load_success_1", "Press Tab to open the editor", "info");
                        notif.show("load_success_2", "Edit client/init.js to change startup settings", "danger");
                    } 
                });

                plugin.load({
                    id: 'weather_plugin',
                    url: 'plugins/weather/index.js',
                    reload: true,
                    after: function() {
                        weather_plugin.snow.active = true;
                    }
                });

                //plugin.load({ id: 'lifecyclea', url: 'plugins/lifecycle/index.js' });
                //plugin.load({ id: 'lifecycleb', url: 'plugins/lifecycleb/index.js' });
            }
        });

    });