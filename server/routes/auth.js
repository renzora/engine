// server/routes/auth.js
const express = require('express');
const router = express.Router();
const bcrypt = require('bcrypt');
const jwt = require('jsonwebtoken');
const User = require('../models/User');
const Note = require('../models/Note');

// Helper function to clean input
function clean(str) {
    return str ? str.trim() : '';
}

// Login route
router.post('/login', async (req, res) => {
    if (req.auth) {
        return res.json({ message: "already_logged_in" });
    }

    const login_username = clean(req.body.login_username);
    const login_password = clean(req.body.login_password);

    if (!login_username || !login_password) {
        return res.json({ message: "error_1" });
    }

    try {
        const user = await User.findOne({
            $or: [
                { username: login_username },
                { email: login_username }
            ]
        });

        if (!user) {
            return res.json({ message: "user_not_found" });
        }

        const passwordMatch = await bcrypt.compare(login_password, user.password);

        if (passwordMatch) {
            const payload = {
                id: user._id.toString(),
                username: user.username,
                iat: Math.floor(Date.now() / 1000),
                exp: Math.floor(Date.now() / 1000) + (60 * 60 * 24 * 7)
            };

            const token = jwt.sign(payload, process.env.JWT_SECRET);

            res.cookie('renaccount', token, {
                expires: new Date(Date.now() + 7 * 24 * 60 * 60 * 1000),
                path: '/',
                secure: process.env.NODE_ENV === 'production',
                httpOnly: true,
                sameSite: 'Strict'
            });

            return res.json({ message: "login_complete", token });
        } else {
            return res.json({ message: "incorrect_info" });
        }
    } catch (error) {
        console.error('Login error:', error);
        return res.json({ message: "server_error" });
    }
});

// Register route
router.post('/register', async (req, res) => {
    if (req.auth) {
        return res.json({ message: "already_logged_in" });
    }

    const register_username = clean(req.body.register_username);
    const register_password = clean(req.body.register_password);
    const register_email = clean(req.body.register_email);

    if (!register_username || !register_password || !register_email) {
        return res.json({ message: "error_1" });
    }

    try {
        // Check if username exists
        const existingUser = await User.findOne({ username: register_username });
        if (existingUser) {
            return res.json({ message: "username_exists" });
        }

        // Hash password
        const password_hash = await bcrypt.hash(register_password, 8);

        // Create new user
        const newUser = await User.create({
            username: register_username,
            password: password_hash,
            email: register_email
        });

        // Create JWT
        const payload = {
            id: newUser._id.toString(),
            username: register_username
        };

        const token = jwt.sign(payload, process.env.JWT_SECRET);

        // Set cookie
        res.cookie('renaccount', token, {
            expires: new Date(Date.now() + 7 * 24 * 60 * 60 * 1000),
            path: '/',
            secure: process.env.NODE_ENV === 'production',
            httpOnly: true,
            sameSite: 'Strict'
        });

        // Create note
        await Note.create({
            profile_uid: newUser._id,
            note: 'Registered Account',
            author: 2
        });

        return res.json({ message: "registration_complete" });
    } catch (error) {
        console.error('Registration error:', error);
        if (error.name === 'ValidationError') {
            if (error.errors.username) {
                if (error.errors.username.kind === 'minlength') {
                    return res.json({ message: "username_too_short" });
                }
                if (error.errors.username.kind === 'maxlength') {
                    return res.json({ message: "username_too_long" });
                }
                if (error.errors.username.kind === 'regexp') {
                    return res.json({ message: "username_invalid" });
                }
            }
        }
        return res.json({ message: "server_error" });
    }
});

// Signout route
router.post('/signout', (req, res) => {
    if (req.auth) {
        res.clearCookie('renaccount', {
            path: '/'
        });
        return res.json({ message: "success" });
    }
    return res.json({ message: "not_logged_in" });
});

module.exports = router;