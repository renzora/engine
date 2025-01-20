const HASHED_ADMIN_PASSWORD = '100000$fef8950db4d167a031d0d9ceb615745c$279e06111f639fa82bbcd984938603070f6e822c9793001ac6b0ea8de962de6a';
const adminDb = db.getSiblingDB('admin');
const MONGO_PASSWORD = 'this_is_a_test_password';

if (!adminDb.system.users.find({ user: "admin" }).hasNext()) {
    adminDb.createUser({
        user: "admin",
        pwd: MONGO_PASSWORD,
        roles: [{ role: "userAdminAnyDatabase", db: "admin" }, "readWriteAnyDatabase"]
    });
}

const renzora = db.getSiblingDB('renzora');

try {
    if (!renzora.servers.findOne({ _id: ObjectId("677e225423f363310b0e9a82") })) {
        renzora.servers.insertMany([{
            _id: ObjectId("677e225423f363310b0e9a82"),
            name: "Game Server",
            created_by: 1,
            created_at: 1736319572,
            public: 1
        }]);
    }

    if (!renzora.scenes.findOne({ _id: ObjectId("678ec2d7433aae2deee168ee") })) {
        renzora.scenes.insertMany([{
            _id: ObjectId("678ec2d7433aae2deee168ee"),
            server_id: ObjectId("677e225423f363310b0e9a82"),
            name: "Default Scene",
            created_by: 1,
            created_at: 1736320671,
            roomData: {
                items: [{
                    id: "405gzo1m64neik4",
                    n: "Room Layout",
                    x: [12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26],
                    y: [9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19],
                    layer_id: "pga0nwe",
                    animationState: [],
                    w: [],
                    public: 1
                }]
            },
            public: 1,
            width: 640,
            height: 640,
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