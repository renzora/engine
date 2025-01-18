import bcrypt from 'bcrypt';
import jwt from 'jsonwebtoken';
import { User } from '../models/User.js';
import { Note } from '../models/Note.js';

function clean(str) {
    return str ? str.trim() : '';
}

export const authRoutes = async (fastify, opts) => {
    fastify.post('/login', async (request, reply) => {
        const { auth } = request;
        if (auth) {
            return reply.send({ message: 'already_logged_in' });
        }

        const login_username = clean(request.body.login_username);
        const login_password = clean(request.body.login_password);

        if (!login_username || !login_password) {
            return reply.send({ message: 'error_1' });
        }

        try {
            const user = await User.findOne({
                $or: [
                    { username: login_username },
                    { email: login_username },
                ],
            });

            if (!user) {
                return reply.send({ message: 'user_not_found' });
            }

            const passwordMatch = await bcrypt.compare(login_password, user.password);

            if (passwordMatch) {
                const payload = {
                    id: user._id.toString(),
                    username: user.username,
                    iat: Math.floor(Date.now() / 1000),
                    exp: Math.floor(Date.now() / 1000) + 60 * 60 * 24 * 7, // 7 days
                };

                const token = jwt.sign(payload, process.env.JWT_SECRET);

                reply.setCookie('renaccount', token, {
                    path: '/',
                    secure: process.env.NODE_ENV === 'production',
                    httpOnly: true,
                    sameSite: 'Strict',
                    expires: new Date(Date.now() + 7 * 24 * 60 * 60 * 1000),
                });

                return reply.send({ message: 'login_complete', token });
            } else {
                return reply.send({ message: 'incorrect_info' });
            }
        } catch (error) {
            fastify.log.error('Login error:', error);
            return reply.send({ message: 'server_error' });
        }
    });

    fastify.post('/register', async (request, reply) => {
        const { auth } = request;
        if (auth) {
            return reply.send({ message: 'already_logged_in' });
        }

        const register_username = clean(request.body.register_username);
        const register_password = clean(request.body.register_password);
        const register_email = clean(request.body.register_email);

        if (!register_username || !register_password || !register_email) {
            return reply.send({ message: 'error_1' });
        }

        try {
            const existingUser = await User.findOne({ username: register_username });
            if (existingUser) {
                return reply.send({ message: 'username_exists' });
            }

            const password_hash = await bcrypt.hash(register_password, 8);

            const newUser = await User.create({
                username: register_username,
                password: password_hash,
                email: register_email,
            });

            const payload = {
                id: newUser._id.toString(),
                username: register_username,
            };

            const token = jwt.sign(payload, process.env.JWT_SECRET);

            reply.setCookie('renaccount', token, {
                path: '/',
                secure: process.env.NODE_ENV === 'production',
                httpOnly: true,
                sameSite: 'Strict',
                expires: new Date(Date.now() + 7 * 24 * 60 * 60 * 1000),
            });

            await Note.create({
                profile_uid: newUser._id,
                note: 'Registered Account',
                author: 2,
            });

            return reply.send({ message: 'registration_complete' });
        } catch (error) {
            fastify.log.error('Registration error:', error);
            if (error.name === 'ValidationError') {
                if (error.errors.username) {
                    if (error.errors.username.kind === 'minlength') {
                        return reply.send({ message: 'username_too_short' });
                    }
                    if (error.errors.username.kind === 'maxlength') {
                        return reply.send({ message: 'username_too_long' });
                    }
                    if (error.errors.username.kind === 'regexp') {
                        return reply.send({ message: 'username_invalid' });
                    }
                }
            }
            return reply.send({ message: 'server_error' });
        }
    });

    fastify.post('/signout', (request, reply) => {
        const { auth } = request;
        if (auth) {
            reply.clearCookie('renaccount', {
                path: '/',
            });
            return reply.send({ message: 'success' });
        }
        return reply.send({ message: 'not_logged_in' });
    });
};
