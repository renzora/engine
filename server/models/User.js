const mongoose = require('mongoose');

const userSchema = new mongoose.Schema({
    username: { 
        type: String, 
        required: true, 
        unique: true,
        match: /^[a-zA-Z0-9._]+$/,
        minlength: 3,
        maxlength: 20
    },
    password: { type: String, required: true },
    email: { type: String, required: true },
    ugroup: { type: Number, default: 1 },
    created: { type: Number, default: () => Math.floor(Date.now() / 1000) }
});

module.exports = mongoose.model('User', userSchema);