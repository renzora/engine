import { FastifyInstance, FastifyReply, FastifyRequest } from 'fastify';
import jwt from 'jsonwebtoken';
import crypto from 'crypto';
import { User } from '../models/User.js';
import { Note } from '../models/Note.js';
import { redis } from '../redis.js';

function clean(str: string | undefined): string {
  return str ? str.trim() : '';
}

function hashPassword(password: string): string {
  const salt = crypto.randomBytes(16);
  const iterationCount = 100_000;
  const keyLen = 32;
  const digest = 'sha256';

  const derivedKey = crypto.pbkdf2Sync(password, salt, iterationCount, keyLen, digest);
  const out = [
    iterationCount.toString(),
    salt.toString('hex'),
    derivedKey.toString('hex'),
  ];
  return out.join('$');
}

function verifyPassword(password: string, stored: string): boolean {
  const [iterationCountStr, saltHex, derivedHex] = stored.split('$');
  const iterationCount = parseInt(iterationCountStr, 10);
  const salt = Buffer.from(saltHex, 'hex');
  const derivedKey = Buffer.from(derivedHex, 'hex');
  const keyLen = derivedKey.length;
  const digest = 'sha256';

  const testKey = crypto.pbkdf2Sync(password, salt, iterationCount, keyLen, digest);
  return crypto.timingSafeEqual(testKey, derivedKey);
}

export async function authRoutes(fastify: FastifyInstance) {
  fastify.post('/login', async (request: FastifyRequest, reply: FastifyReply) => {
    const { auth } = request;
    if (auth) {
      return reply.send({ message: 'already_logged_in' });
    }

    const { login_username, login_password } = request.body as {
      login_username: string;
      login_password: string;
    };

    const username = clean(login_username);
    const password = clean(login_password);

    if (!username || !password) {
      return reply.send({ message: 'error_1' });
    }

    try {
      const user = await User.findOne({
        $or: [{ username }, { email: username }],
      });

      if (!user) {
        return reply.send({ message: 'user_not_found' });
      }

      if (!verifyPassword(password, user.password)) {
        return reply.send({ message: 'incorrect_info' });
      }

      const payload = {
        id: user._id.toString(),
        username: user.username,
        iat: Math.floor(Date.now() / 1000),
        exp: Math.floor(Date.now() / 1000) + 60 * 60 * 24 * 7, // 7 days
      };

      const token = jwt.sign(payload, process.env.JWT_SECRET as string);

      reply.setCookie('renaccount', token, {
        path: '/',
        secure: process.env.NODE_ENV === 'production',
        httpOnly: true,
        sameSite: 'Strict',
        expires: new Date(Date.now() + 7 * 24 * 60 * 60 * 1000),
      });

      await redis.set(`user:${user._id}`, JSON.stringify(user), 'EX', 86400);

      return reply.send({ message: 'login_complete', token });
    } catch (error) {
      fastify.log.error('Login error:', error);
      return reply.send({ message: 'server_error' });
    }
  });

  fastify.post('/register', async (request: FastifyRequest, reply: FastifyReply) => {
    const { auth } = request;
    if (auth) {
      return reply.send({ message: 'already_logged_in' });
    }

    const { register_username, register_password, register_email } = request.body as {
      register_username: string;
      register_password: string;
      register_email: string;
    };

    const username = clean(register_username);
    const password = clean(register_password);
    const email = clean(register_email);

    if (!username || !password || !email) {
      return reply.send({ message: 'error_1' });
    }

    try {
      const existingUser = await User.findOne({ username });
      if (existingUser) {
        return reply.send({ message: 'username_exists' });
      }

      const hashedPwd = hashPassword(password);

      const newUser = await User.create({
        username,
        password: hashedPwd,
        email,
      });

      const payload = {
        id: newUser._id.toString(),
        username,
      };

      const token = jwt.sign(payload, process.env.JWT_SECRET as string);

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

      await redis.set(`user:${newUser._id}`, JSON.stringify(newUser), 'EX', 86400);

      return reply.send({ message: 'registration_complete' });
    } catch (error: any) {
      fastify.log.error('Registration error:', error);
      if (error.name === 'ValidationError' && error.errors?.username) {
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
      return reply.send({ message: 'server_error' });
    }
  });

  fastify.post('/signout', (request: FastifyRequest, reply: FastifyReply) => {
    const { auth } = request;
    if (auth) {
      reply.clearCookie('renaccount', { path: '/' });
      redis.del(`user:${auth.id}`);
      return reply.send({ message: 'success' });
    }
    return reply.send({ message: 'not_logged_in' });
  });
}