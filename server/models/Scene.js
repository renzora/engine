const mongoose = require('mongoose');

const sceneSchema = new mongoose.Schema({
    _id: mongoose.Schema.Types.ObjectId,
    server_id: mongoose.Schema.Types.ObjectId,
    name: String,
    created_by: Number,
    created_at: Number,
    roomData: Object,
    public: Number,
    width: Number,
    height: Number,
    startingX: Number,
    startingY: Number,
    bg: String,
    facing: String,
    fireflys: Number,
    clouds: Number,
    rain: Number,
    snow: Number
});

module.exports = mongoose.model('Scene', sceneSchema, 'scenes');