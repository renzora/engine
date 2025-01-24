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
                    plugin.load('editor_window', { path: 'editor', ext: 'njk' });
                }
            });
        });

        input.assign('keydown+ctrl+shift+f', () => { plugin.ui.fullScreen(); });

        plugin.preload([
            { id: 'time', path: 'core' },
            { id: 'lighting' },
            { id: 'notif', path: 'core', ext: 'html' },
            { id: 'collision' },
            { id: 'pathfinding' },
            { id: 'scripting' },
            { id: 'snow', after: function() {
                plugin.notif.show("load_success_1", "Press Tab to open the editor", "info");
                plugin.notif.show("load_success_2", "Edit client/init.js to change startup settings", "danger");
            }},
            { id: 'terminal', ext: 'html' },
            { id: 'debug', ext: 'html' },
            { id: 'activity_monitor', ext: 'njk' },
            { id: 'pie_menu', ext: 'html' },
        ]);

        const playerSprite = sprite.create({
            id: 'player1',
            isPlayer: true,
            speed: 65,
            animalType: 'female-01',
            canShoot: true,
            targetAim: false
        });

        game.create({
            objectData: assets.use('objectData'),
            spriteData: assets.use('spriteData'),
            player: playerSprite,
            after: function() {
                game.scene(localStorage.getItem('sceneid') || '678ec2d7433aae2deee168ee');
                plugin.load('auth', { ext: 'njk' });
                sprite.init();
            }
        });

    });