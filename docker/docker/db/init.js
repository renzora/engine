conn = new Mongo();
db = conn.getDB("admin");

db.createUser({
    user: "admin",
    pwd: "password",
    roles: [
        { role: "userAdminAnyDatabase", db: "admin" },
        "readWriteAnyDatabase"
    ]
});

db.getSiblingDB('renzora').users.insertOne({
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