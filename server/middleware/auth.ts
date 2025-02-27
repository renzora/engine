import { FastifyReply, FastifyRequest } from 'fastify';
import jwt from 'jsonwebtoken';
import { User } from '../models/User.js';
import { redis } from '../redis.js';

export async function authMiddleware(
  request: FastifyRequest,
  reply: FastifyReply,
): Promise<void> {
  const token = request.cookies?.renaccount;
  if (token) {
    try {
      const decoded = jwt.verify(token, process.env.JWT_SECRET as string) as {
        id: string;
        username: string;
        iat: number;
        exp: number;
      };

      const cachedUser = await redis.get(`user:${decoded.id}`);
      if (cachedUser) {
        console.log(`✅ [REDIS] Found user ${decoded.id} in cache`);
        request.auth = decoded;
        request.user = JSON.parse(cachedUser);
        return;
      } else {
        const user = await User.findById(decoded.id);
        if (!user) {
          request.auth = null;
          request.user = null;
          return;
        }

        await redis.set(
          `user:${user._id}`,
          JSON.stringify(user),
          'EX',
          86400
        );

        console.log(`🔵 [DB] Loaded user ${decoded.id} from DB and cached in Redis`);

        request.auth = decoded;
        request.user = user;
      }
    } catch (err) {
      request.auth = null;
      request.user = null;
    }
  } else {
    request.auth = null;
    request.user = null;
  }
}