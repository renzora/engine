const BCRYPT_HASHED_ADMIN_PASSWORD = '$2b$08$2i8woGGgvPL1FweRsCl.MOJKYsbpGCREJQjUnlEQOkhvTkjEdqkLa';

const adminDb = db.getSiblingDB('admin');

if (!adminDb.system.users.find({ user: "admin" }).hasNext()) {
    adminDb.createUser({
        user: "admin",
        pwd: "password",
        roles: [{ role: "userAdminAnyDatabase", db: "admin" }, "readWriteAnyDatabase"]
    });
}

const renzora = db.getSiblingDB('renzora');

try {
    if (!renzora.users.findOne({ username: 'admin' })) {
        renzora.users.insertOne({
            _id: 1,
            username: 'admin',
            password: BCRYPT_HASHED_ADMIN_PASSWORD,
            email: 'admin@admin.com',
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
    }

    if (!renzora.servers.findOne({ _id: ObjectId("677e225423f363310b0e9a82") })) {
        renzora.servers.insertMany([{
            _id: ObjectId("677e225423f363310b0e9a82"),
            name: "Game Server",
            created_by: 1,
            created_at: 1736319572,
            public: 1
        }]);
    }

    if (!renzora.scenes.findOne({ _id: ObjectId("677e269fb2e1d04dd00e9cf2") })) {
        renzora.scenes.insertMany([{
            _id: ObjectId("677e269fb2e1d04dd00e9cf2"),
            server_id: ObjectId("677e225423f363310b0e9a82"),
            name: "Default Scene",
            created_by: 1,
            created_at: 1736320671,
            roomData: {
                items: [],
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
        }]);
    }
} catch (e) {
    print("Error during initialization:", e);
}
