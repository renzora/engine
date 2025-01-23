// This is your startup file. Add all your plugins, preload assets and do any engine configurations here.

    assets.preload([
        { name: 'female-01', path: 'assets/img/sprites/characters/female-01.png' },
        { name: 'objectData', path: 'assets/json/objectData.json' },
        { name: 'spriteData', path: 'assets/json/spritesData.json' }
    ],() => {

        input.assign('keydown.tab', () => {

            plugin.load('console_window', {
                path: 'editor',
                ext: 'njk',
                drag: false,
                reload: true,
                after: function () {
                    plugin.load('editor_window', { path: 'editor', ext: 'njk', drag: true, reload: true });
                }
            });
          });

        plugin.load('time');
        plugin.load('scripting');
        plugin.load('audio', {
            after: function() {
                audio.createChannel('music', localStorage.getItem('music-volume') || audio.defaultVolume);
                audio.setVolume('music', localStorage.getItem('music-volume') || 0.05);
            }
        });
        plugin.load('notif', { ext: 'html' });
        plugin.load('pathfinding');

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
                plugin.load('auth', { ext: 'njk' });
                plugin.notif.show("load_success_1", "Press Tab to open the editor", "info");
                plugin.notif.show("load_success_2", "Edit client/init.js to change startup settings", "danger");
            }
        });

    });