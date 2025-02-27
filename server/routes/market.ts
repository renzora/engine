import { FastifyInstance, FastifyReply, FastifyRequest } from 'fastify';
import { User } from '../models/User.js';
import { StoreCategory } from '../models/StoreCategory.js';
import { StoreItem } from '../models/StoreItem.js';
import { redis } from '../redis.js';
import { Voucher } from '../models/Voucher.js';
import { Pack } from '../models/Pack.js';
import { PackOpening } from '../models/PackOpening.js';
import mongoose from 'mongoose';

// Helper function to generate random voucher code
function generateVoucherCode(length: number = 12): string {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
    let code = '';
    for (let i = 0; i < length; i++) {
        code += chars.charAt(Math.floor(Math.random() * chars.length));
    }
    return code;
}

export async function marketRoutes(fastify: FastifyInstance) {
   // Store Categories
   fastify.get('/categories', async (request: FastifyRequest, reply: FastifyReply) => {
       try {
           const categories = await StoreCategory.find().sort({ order: 1, created: -1 });
           return reply.send(categories);
       } catch (error) {
           return reply.status(500).send({ error: 'Failed to load categories' });
       }
   });

   fastify.get('/categories/:id', async (request: FastifyRequest<{
       Params: { id: string }
   }>, reply: FastifyReply) => {
       try {
           const category = await StoreCategory.findById(request.params.id);
           if (!category) {
               return reply.status(404).send({ error: 'Category not found' });
           }
           return reply.send(category);
       } catch (error) {
           return reply.status(500).send({ error: 'Failed to load category' });
       }
   });

   fastify.post('/categories', async (request: FastifyRequest<{
       Body: {
           name: string;
           description?: string;
           active: boolean;
       }
   }>, reply: FastifyReply) => {
       try {
           const category = new StoreCategory(request.body);
           await category.save();
           return reply.send(category);
       } catch (error) {
           return reply.status(500).send({ error: 'Failed to create category' });
       }
   });

   fastify.put('/categories/:id', async (request: FastifyRequest<{
       Params: { id: string };
       Body: {
           name: string;
           description?: string;
           active: boolean;
       }
   }>, reply: FastifyReply) => {
       try {
           const category = await StoreCategory.findByIdAndUpdate(
               request.params.id,
               request.body,
               { new: true }
           );
           if (!category) {
               return reply.status(404).send({ error: 'Category not found' });
           }
           return reply.send(category);
       } catch (error) {
           return reply.status(500).send({ error: 'Failed to update category' });
       }
   });

   fastify.delete('/categories/:id', async (request: FastifyRequest<{
       Params: { id: string }
   }>, reply: FastifyReply) => {
       try {
           const category = await StoreCategory.findByIdAndDelete(request.params.id);
           if (!category) {
               return reply.status(404).send({ error: 'Category not found' });
           }
           await StoreItem.deleteMany({ categoryId: request.params.id });
           return reply.send({ success: true });
       } catch (error) {
           return reply.status(500).send({ error: 'Failed to delete category' });
       }
   });

   // Store Balance
   fastify.get('/balance', async (request: FastifyRequest, reply: FastifyReply) => {
       try {
           const { auth } = request;
           if (!auth) {
               return reply.status(401).send({ error: 'Not authenticated' });
           }

           const cachedUser = await redis.get(`user:${auth.id}`);
           if (cachedUser) {
               const user = JSON.parse(cachedUser);
               return reply.send({ 
                   coins: user.coins || 0,
                   success: true 
               });
           }

           const user = await User.findById(auth.id);
           if (!user) {
               return reply.status(404).send({ error: 'User not found' });
           }

           await redis.set(`user:${user._id}`, JSON.stringify(user), 'EX', 86400);

           return reply.send({ 
               coins: user.coins || 0,
               success: true 
           });

       } catch (error) {
           return reply.status(500).send({ error: 'Failed to load balance' });
       }
   });

   fastify.post('/vouchers/redeem', async (request: FastifyRequest<{
    Body: {
        code: string;
    }
}>, reply: FastifyReply) => {
    try {
        const { auth } = request;
        if (!auth) {
            return reply.status(401).send({ error: 'Not authenticated' });
        }

        const voucher = await Voucher.findOne({
            code: request.body.code.toUpperCase(),
            active: true,
            usedBy: { $exists: false },
            $or: [
                { expiresAt: { $exists: false } },
                { expiresAt: { $gt: Math.floor(Date.now() / 1000) } }
            ]
        });

        if (!voucher) {
            return reply.status(404).send({ error: 'Invalid or expired voucher code' });
        }

        const user = await User.findById(auth.id);
        if (!user) {
            return reply.status(404).send({ error: 'User not found' });
        }

        user.coins = (user.coins || 0) + voucher.coins;
        await user.save();

        voucher.active = false;
        voucher.usedBy = user._id;
        voucher.usedAt = Math.floor(Date.now() / 1000);
        await voucher.save();

        await redis.set(`user:${user._id}`, JSON.stringify(user), 'EX', 86400);

        return reply.send({
            success: true,
            coinsAdded: voucher.coins,
            newBalance: user.coins
        });

    } catch (error) {
        return reply.status(500).send({ error: 'Failed to redeem voucher' });
    }
});

   // Store Items
   fastify.get('/items', async (request: FastifyRequest<{
       Querystring: {
           category?: string;
       }
   }>, reply: FastifyReply) => {
       try {
           const { category } = request.query;
           
           let query: any = { active: true };
           
           switch(category) {
               case 'popular':
                   return await StoreItem.aggregate([
                       { $match: { active: true } },
                       { $sample: { size: 8 } }
                   ]);
                   
               case 'packs':
                   query.type = 'pack';
                   break;
                   
               case 'coins':
               case 'voucher':
                   return reply.send([]);
                   
               default:
                   if (category) {
                       query.categoryId = new mongoose.Types.ObjectId(category);
                   }
           }

           const items = await StoreItem.find(query)
               .populate('categoryId')
               .sort({ order: 1, created: -1 });
               
           return reply.send(items);

       } catch (error) {
           fastify.log.error('Error loading store items:', error);
           return reply.status(500).send({ error: 'Failed to load store items' });
       }
   });

   fastify.get('/items/:id', async (request: FastifyRequest<{
       Params: {
           id: string;
       }
   }>, reply: FastifyReply) => {
       try {
           const item = await StoreItem.findById(request.params.id)
               .populate('categoryId');
               
           if (!item) {
               return reply.status(404).send({ error: 'Item not found' });
           }
           
           return reply.send(item);
           
       } catch (error) {
           fastify.log.error('Error loading store item:', error);
           return reply.status(500).send({ error: 'Failed to load store item' });
       }
   });

   fastify.post('/items', async (request: FastifyRequest<{
       Body: {
           name: string;
           description?: string;
           categoryId: string;
           objectDataId: string;
           price: number;
           active: boolean;
           type?: string;
       }
   }>, reply: FastifyReply) => {
       try {
           const item = new StoreItem(request.body);
           await item.save();
           return reply.send(item);
       } catch (error) {
           return reply.status(500).send({ error: 'Failed to create item' });
       }
   });

   fastify.put('/items/:id', async (request: FastifyRequest<{
       Params: { id: string };
       Body: {
           name: string;
           description?: string;
           categoryId: string;
           objectDataId: string;
           price: number;
           active: boolean;
           type?: string;
       }
   }>, reply: FastifyReply) => {
       try {
           const item = await StoreItem.findByIdAndUpdate(
               request.params.id,
               request.body,
               { new: true }
           );
           if (!item) {
               return reply.status(404).send({ error: 'Item not found' });
           }
           return reply.send(item);
       } catch (error) {
           return reply.status(500).send({ error: 'Failed to update item' });
       }
   });

   fastify.delete('/items/:id', async (request: FastifyRequest<{
       Params: { id: string }
   }>, reply: FastifyReply) => {
       try {
           const item = await StoreItem.findByIdAndDelete(request.params.id);
           if (!item) {
               return reply.status(404).send({ error: 'Item not found' });
           }
           return reply.send({ success: true });
       } catch (error) {
           return reply.status(500).send({ error: 'Failed to delete item' });
       }
   });

   fastify.post('/items/:id/buy', async (request: FastifyRequest<{
       Params: {
           id: string;
       }
   }>, reply: FastifyReply) => {
       try {
           const { auth } = request;
           if (!auth) {
               return reply.status(401).send({ error: 'Not authenticated' });
           }

           const item = await StoreItem.findById(request.params.id);
           if (!item) {
               return reply.status(404).send({ error: 'Item not found' });
           }

           const user = await User.findById(auth.id);
           if (!user) {
               return reply.status(404).send({ error: 'User not found' });
           }

           if ((user.coins || 0) < item.price) {
               return reply.status(400).send({ error: 'Insufficient coins' });
           }

           user.coins = (user.coins || 0) - item.price;
           await user.save();

           await redis.set(`user:${user._id}`, JSON.stringify(user), 'EX', 86400);

           return reply.send({ 
               success: true,
               newBalance: user.coins
           });

       } catch (error) {
           fastify.log.error('Error purchasing item:', error);
           return reply.status(500).send({ error: 'Failed to purchase item' });
       }
   });

   // Voucher routes
fastify.get('/vouchers', async (request: FastifyRequest, reply: FastifyReply) => {
    try {
        const vouchers = await Voucher.find()
            .sort({ created: -1 })
            .populate('usedBy', 'username');
        return reply.send(vouchers);
    } catch (error) {
        return reply.status(500).send({ error: 'Failed to load vouchers' });
    }
});

fastify.post('/vouchers', async (request: FastifyRequest<{
    Body: {
        coins: number;
        expiresAt?: number;
    }
}>, reply: FastifyReply) => {
    try {
        const code = generateVoucherCode();
        const voucher = new Voucher({
            code,
            coins: request.body.coins,
            expiresAt: request.body.expiresAt
        });
        await voucher.save();
        return reply.send(voucher);
    } catch (error) {
        return reply.status(500).send({ error: 'Failed to create voucher' });
    }
});

fastify.delete('/vouchers/:id', async (request: FastifyRequest<{
    Params: { id: string }
}>, reply: FastifyReply) => {
    try {
        const voucher = await Voucher.findByIdAndDelete(request.params.id);
        if (!voucher) {
            return reply.status(404).send({ error: 'Voucher not found' });
        }
        return reply.send({ success: true });
    } catch (error) {
        return reply.status(500).send({ error: 'Failed to delete voucher' });
    }
});

fastify.post('/packs/open', async (request: FastifyRequest<{
    Body: {
        packType: 'basic' | 'elite' | 'legendary'
    }
}>, reply: FastifyReply) => {
    try {
        const { auth } = request;
        if (!auth) {
            return reply.status(401).send({ error: 'Not authenticated' });
        }

        const packPrices = {
            basic: 300,
            elite: 800,
            legendary: 1500
        };

        const packItemCounts = {
            basic: 20,
            elite: 20,
            legendary: 20
        };

        const user = await User.findById(auth.id);
        if (!user) {
            return reply.status(404).send({ error: 'User not found' });
        }

        const price = packPrices[request.body.packType];
        if ((user.coins || 0) < price) {
            return reply.status(400).send({ error: 'Insufficient coins' });
        }

        const items = await StoreItem.find({ active: true });

        const rarityChances = {
            basic: {
                Common: 0.80,
                Uncommon: 0.15,
                Rare: 0.04,
                Epic: 0.007,
                Legendary: 0.003
            },
            elite: {
                Common: 0.60,
                Uncommon: 0.25,
                Rare: 0.10,
                Epic: 0.04,
                Legendary: 0.01
            },
            legendary: {
                Common: 0.35,
                Uncommon: 0.35,
                Rare: 0.20,
                Epic: 0.08,
                Legendary: 0.02
            }
        };

        const selectedItems = new Set();
        const availableItems = [...items];

        while (selectedItems.size < packItemCounts[request.body.packType] && availableItems.length > 0) {
            const rand = Math.random();
            let currentProb = 0;
            let selectedRarity = 'Common';

            for (const [rarity, chance] of Object.entries(rarityChances[request.body.packType])) {
                currentProb += chance;
                if (rand <= currentProb) {
                    selectedRarity = rarity;
                    break;
                }
            }

            const possibleItems = availableItems.filter(item => item.rarity === selectedRarity);
            if (possibleItems.length > 0) {
                const randomIndex = Math.floor(Math.random() * possibleItems.length);
                const selectedItem = possibleItems[randomIndex];
                
                selectedItems.add(selectedItem);
                
                const itemIndex = availableItems.findIndex(item => item._id.equals(selectedItem._id));
                if (itemIndex !== -1) {
                    availableItems.splice(itemIndex, 1);
                }
            }
        }

        user.coins = (user.coins || 0) - price;
        await user.save();

        await redis.set(`user:${user._id}`, JSON.stringify(user), 'EX', 86400);

        return reply.send({ 
            items: Array.from(selectedItems),
            newBalance: user.coins 
        });

    } catch (error) {
        fastify.log.error('Error opening pack:', error);
        return reply.status(500).send({ error: 'Failed to open pack' });
    }
});
}