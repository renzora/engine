// This is your startup file. Add all your plugins, preload assets and do any engine configurations here.

    assets.preload([
        { name: 'female-01', path: 'assets/img/sprites/characters/female-01.png' },
        { name: 'objectData', path: 'assets/json/objectData.json' },
        { name: 'spriteData', path: 'assets/json/spritesData.json' }
    ],() => {

        plugin.load({ id: 'time', url: 'plugins/time/index.js'});
        plugin.load({ id: 'scripting', url: 'plugins/scripting/index.js' });

        plugin.load({
            id: 'audio',
            url: 'plugins/audio/index.js',
            after: function() {
                audio.createChannel('music', localStorage.getItem('music-volume') || audio.defaultVolume);
                audio.setVolume('music', localStorage.getItem('music-volume') || 0.05);
            }
        });

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
                plugin.load({ id: 'auth_window', url: 'plugins/auth/index.njk' });

                plugin.load({
                    id: 'notif',
                    url: 'plugins/notifs/index.html',
                    after: function() {
                        notif.show("load_success_1", "Press Tab to open the editor", "info");
                        notif.show("load_success_2", "Edit client/init.js to change startup settings", "danger");
                    } 
                });

            }
        });

    });