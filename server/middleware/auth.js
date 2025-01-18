import jwt from 'jsonwebtoken';
import { User } from '../models/User.js';

export const authMiddleware = async (request, reply) => {
  const token = request.cookies?.renaccount;
  if (token) {
    try {
      const decoded = jwt.verify(token, process.env.JWT_SECRET);
      const user = await User.findById(decoded.id);
      if (!user) {
        request.auth = null;
        return;
      }

      request.auth = decoded;
      request.user = user;
    } catch (err) {
      request.auth = null;
      request.user = null;
    }
  } else {
    request.auth = null;
    request.user = null;
  }
};
