import { FastifyInstance, FastifyReply, FastifyRequest } from 'fastify';
import { AccessCode } from '../models/AccessCode.js';
import { User } from '../models/User.js';
import { redis } from '../redis.js';

export async function accessRoutes(fastify: FastifyInstance) {
    fastify.post('/verify', async (request: FastifyRequest<{
        Body: { code: string }
    }>, reply: FastifyReply) => {
        const { code } = request.body;
        const { user } = request;

        if (!user) {
            return reply.status(401).send({ 
                success: false, 
                message: 'User not authenticated' 
            });
        }

        if (!code) {
            return reply.status(400).send({ 
                success: false, 
                message: 'Access code is required' 
            });
        }

        try {
            const accessCode = await AccessCode.findOne({ 
                code: code.trim().toUpperCase(),
                isActive: true,
                usedBy: null
            });

            if (!accessCode) {
                return reply.send({ 
                    success: false, 
                    message: 'Invalid or already used access code' 
                });
            }

            // Update the access code as used
            accessCode.usedBy = user._id;
            accessCode.usedAt = Math.floor(Date.now() / 1000);
            accessCode.isActive = false;
            await accessCode.save();

            // Add the 'is_beta' permission to the user
            if (!user.permissions.includes('is_beta')) {
                // Use findByIdAndUpdate instead of user.save()
                await User.findByIdAndUpdate(user._id, {
                    $addToSet: { permissions: 'is_beta' }
                });
                
                // Update the user in Redis cache
                const updatedUser = await User.findById(user._id);
                await redis.set(`user:${user._id}`, JSON.stringify(updatedUser), 'KEEPTTL');
            }

            return reply.send({ 
                success: true, 
                message: 'Access granted successfully' 
            });
        } catch (error) {
            console.error('Access code verification error:', error);
            return reply.status(500).send({ 
                success: false, 
                message: 'Server error while verifying access code' 
            });
        }
    });

    fastify.post('/generate', async (request: FastifyRequest<{
        Body: { count?: number }
    }>, reply: FastifyReply) => {
        const { user } = request;
        const count = request.body.count || 1;

        if (!user || !user.permissions.includes('manage_access_codes')) {
            return reply.status(403).send({ 
                success: false, 
                message: 'Permission denied' 
            });
        }

        if (count < 1 || count > 50) {
            return reply.status(400).send({ 
                success: false, 
                message: 'Count must be between 1 and 50' 
            });
        }

        try {
            const generateCode = () => {
                const chars = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
                let code = '';
                for (let i = 0; i < 8; i++) {
                    code += chars.charAt(Math.floor(Math.random() * chars.length));
                }
                return code;
            };

            const codes = [];
            for (let i = 0; i < count; i++) {
                let code = generateCode();
                
                // Create the access code
                const accessCode = await AccessCode.create({
                    code,
                    createdBy: user._id
                });
                
                codes.push(accessCode);
            }

            return reply.send({ 
                success: true, 
                codes 
            });
        } catch (error) {
            console.error('Access code generation error:', error);
            return reply.status(500).send({ 
                success: false, 
                message: 'Server error while generating access codes' 
            });
        }
    });

    fastify.get('/list', async (request: FastifyRequest, reply: FastifyReply) => {
        const { user } = request;

        if (!user || !user.permissions.includes('manage_access_codes')) {
            return reply.status(403).send({ 
                success: false, 
                message: 'Permission denied' 
            });
        }

        try {
            const codes = await AccessCode.find()
                .sort({ createdAt: -1 })
                .populate('createdBy', 'username')
                .populate('usedBy', 'username');

            return reply.send({ 
                success: true, 
                codes 
            });
        } catch (error) {
            console.error('Access code list error:', error);
            return reply.status(500).send({ 
                success: false, 
                message: 'Server error while retrieving access codes' 
            });
        }
    });
}