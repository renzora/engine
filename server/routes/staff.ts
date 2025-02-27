import { FastifyInstance, FastifyReply, FastifyRequest } from 'fastify';
import { User } from '../models/User.js';
import { Note } from '../models/Note.js';
import { Permission } from '../models/Permissions.js';
import { redis } from '../redis.js';
import mongoose from 'mongoose';

export async function staffRoutes(fastify: FastifyInstance) {
   // View route
   fastify.get('/', async (request: FastifyRequest, reply: FastifyReply) => {
       return reply.view('staff/index.njk');
   });

   // Get permissions
   fastify.get('/permissions', async (request: FastifyRequest, reply: FastifyReply) => {
    try {
        const permissions = await Permission.find().sort({ key: 1 })
        return reply.send(permissions)
    } catch (error) {
        return reply.status(500).send({ error: 'Failed to load permissions' })
    }
});

fastify.get('/users/search', async (request: FastifyRequest<{
    Querystring: { q: string }
}>, reply: FastifyReply) => {
    const query = request.query.q

    if (!query || query.length < 2) {
        return reply.send([])
    }

    try {
        const users = await User.find({
            username: { 
                $regex: query, 
                $options: 'i' 
            }
        })
        .select('username')
        .limit(10)

        return reply.send(users)
    } catch (error) {
        return reply.status(500).send({ error: 'Search failed' })
    }
});

fastify.get('/users/:id', async (request: FastifyRequest<{
    Params: { id: string }
}>, reply: FastifyReply) => {
    try {
        const user = await User.findById(request.params.id)
        if (!user) {
            return reply.status(404).send({ error: 'User not found' })
        }
        return reply.send(user)
    } catch (error) {
        return reply.status(500).send({ error: 'Failed to load user' })
    }
});

fastify.get('/users/:id/notes', async (request: FastifyRequest<{
    Params: { id: string }
}>, reply: FastifyReply) => {
    try {
        const notes = await Note.find({ 
            profile_uid: request.params.id 
        }).sort({ time: -1 })
        
        return reply.send(notes)
    } catch (error) {
        return reply.status(500).send({ error: 'Failed to load notes' })
    }
});

fastify.post('/users/:id/notes', async (request: FastifyRequest<{
 Params: { id: string }
 Body: { note: string }
}>, reply: FastifyReply) => {
 try {
     if (!request.body.note) {
         return reply.status(400).send({ error: 'Note content is required' })
     }

     const note = new Note({
         profile_uid: new mongoose.Types.ObjectId(request.params.id),
         note: request.body.note,
         author: request.user?.username,
         time: Math.floor(Date.now() / 1000)
     })
     
     await note.save()
     return reply.send(note)
 } catch (error) {
     console.error('Note creation error:', error)
     return reply.status(500).send({ error: 'Failed to add note' })
 }
});

fastify.put('/users/:id', async (request: FastifyRequest<{
    Params: { id: string }
    Body: {
        username: string
        email: string
        permissions: string[]
    }
}>, reply: FastifyReply) => {
    try {
        const user = await User.findByIdAndUpdate(
            request.params.id,
            request.body,
            { new: true }
        )
        if (!user) {
            return reply.status(404).send({ error: 'User not found' })
        }

        // Update Redis cache with new user data while maintaining their session
        await redis.set(`user:${user._id}`, JSON.stringify(user), 'KEEPTTL')

        return reply.send(user)
    } catch (error) {
        return reply.status(500).send({ error: 'Failed to update user' })
    }
});
}