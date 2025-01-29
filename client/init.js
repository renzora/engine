// This is your startup file. Add all your plugins, preload assets and do any engine configurations here.
    assets.preload([
        { name: 'female-01', path: 'assets/img/sprites/characters/female-01.png' },
        { name: 'objectData', path: 'assets/json/objectData.json' },
        { name: 'spriteData', path: 'assets/json/spritesData.json' }
    ],() => {

        input.assign('keydown+shift+e', () => {
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

        input.assign('keydown+shift+f', () => { plugin.ui.fullScreen(); });

        plugin.preload([
            { id: 'time', path: 'core' },
            { id: 'lighting' },
            { id: 'notif', path: 'core', ext: 'html' },
            { id: 'collision' },
            { id: 'pathfinding' },
            //{ id: 'snow' },
            { id: 'debug', ext: 'html' },
            { id: 'pie_menu', ext: 'html' },
            { id: 'code', ext: 'html' }
            
        ]);

        const playerSprite = sprite.create({
            id: 'player1',
            isPlayer: true,
            speed: 85,
            animalType: 'female-01',
            canShoot: true,
            targetAim: false
        });

        game.create({
            objectData: assets.use('objectData'),
            spriteData: assets.use('spriteData'),
            player: playerSprite,
            after: function() {
                game.scene(localStorage.getItem('sceneid') || '6799a5c66a1f68cfcfd3d4d1');
                plugin.load('auth', { ext: 'njk' });
                sprite.init();
                plugin.time.hours = 10;
            }
        });

    });