conn = new Mongo();
db = conn.getDB("admin");

// Create admin user
db.createUser({
    user: "admin",
    pwd: "password",
    roles: [
        { role: "userAdminAnyDatabase", db: "admin" },
        "readWriteAnyDatabase"
    ]
});

// Switch to the renzora database
db = conn.getDB("renzora");

// Insert data into the users collection
db.users.insertOne({
    _id: 1, // MongoDB uses _id instead of id for the primary key
    username: 'admin',
    password: '$2y$08$92FucCqO3x/x7oImSCLAWOMdmMCaQD/MB.6LZV0E3.TvTBH4GTz5W',
    email: 'admin@test.com',
    ugroup: 1,
    created: 1709851580,
    coins: 0,
    perms: '',
    avatar: '',
    active: 1,
    shadow_ban: 0,
    ban_expire: 0,
    two_fa: '',
    premium: 0,
    partner: 0,
    staff: 1,
    site_mod: 1
});

// Insert data into the servers collection
db.servers.insertMany([
    {
        _id: ObjectId("677e225423f363310b0e9a82"),
        name: "Game Server",
        created_by: 1,
        created_at: 1736319572,
        public: 1
    }
]);

// Insert data into the scenes collection
db.scenes.insertMany([
    {
        _id: ObjectId("677e269fb2e1d04dd00e9cf2"),
        server_id: ObjectId("677e225423f363310b0e9a82"),
        name: "Default Scene",
        created_by: 1,
        created_at: 1736320671,
        roomData: {
            items: [
                {
                    id: "6772e14939830",
                    x: [
                        10.125, 11.125, 12.125, 13.125, 14.125, 15.125, 16.125,
                        17.125, 18.125, 19.125, 20.125, 21.125, 22.125, 23.125,
                        24.125, 25.125, 26.125, 27.125, 28.125
                    ],
                    y: [
                        6.875, 7.875, 8.875, 9.875, 10.875, 11.875, 12.875,
                        13.875, 14.875, 15.875, 16.875, 17.875, 18.875
                    ],
                    animationState: [
                        {
                            currentFrame: 0,
                            elapsedTime: 722649.9999997823
                        }
                    ],
                    w: []
                },
                {
                    id: "6772e419ccf5e",
                    x: [13.25, 14.25, 15.25, 16.25],
                    y: [11.3125, 12.3125, 13.3125, 14.3125, 15.3125],
                    animationState: [
                        {
                            currentFrame: 0,
                            elapsedTime: 0
                        }
                    ],
                    w: [
                        { x: 1, y: 56 },
                        { x: 28, y: 42 },
                        { x: 57, y: 56 },
                        { x: 29, y: 71 }
                    ]
                },
                {
                    id: "6776f796d8867",
                    x: [16.75, 17.75],
                    y: [7.875, 8.875, 9.875, 10.875],
                    animationState: [
                        {
                            currentFrame: 0,
                            elapsedTime: 0
                        }
                    ],
                    w: 1
                },
                {
                    id: "6777afafbf17d",
                    x: [23.4375, 24.4375],
                    y: [11.5, 12.5, 13.5, 14.5],
                    animationState: [
                        {
                            currentFrame: 10,
                            elapsedTime: 203.33333333330728
                        }
                    ],
                    w: [
                        { x: 12, y: 56 },
                        { x: 26, y: 56 },
                        { x: 26, y: 45 },
                        { x: 12, y: 45 }
                    ]
                }
            ],
            startingX: 18,
            startingY: 13
        },
        public: 1,
        width: 640,
        height: 464,
        startingX: 288,
        startingY: 208,
        bg: "",
        facing: "S",
        fireflys: 0,
        clouds: 0,
        rain: 0,
        snow: 0
    }
]);
