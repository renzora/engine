// server/models/auth/Note.js
const mongoose = require('mongoose');

const noteSchema = new mongoose.Schema({
    profile_uid: { type: mongoose.Schema.Types.ObjectId, ref: 'User', required: true },
    note: { type: String, required: true },
    author: { type: Number, required: true },
    time: { type: Number, default: () => Math.floor(Date.now() / 1000) }
});

module.exports = mongoose.model('Note', noteSchema);