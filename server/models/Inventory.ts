import mongoose, { Schema, Document } from 'mongoose';

export interface IInventoryItem extends Document {
    userId: mongoose.Types.ObjectId;
    itemId: mongoose.Types.ObjectId;
    obtained: number;
    source: 'pack' | 'purchase' | 'gift';
    packType?: 'basic' | 'elite' | 'legendary';
}

const inventorySchema = new Schema<IInventoryItem>({
    userId: { type: mongoose.Schema.Types.ObjectId, ref: 'User', required: true },
    itemId: { type: mongoose.Schema.Types.ObjectId, ref: 'StoreItem', required: true },
    obtained: { type: Number, default: () => Math.floor(Date.now() / 1000) },
    source: { type: String, enum: ['pack', 'purchase', 'gift'], required: true },
    packType: { type: String, enum: ['basic', 'elite', 'legendary'] }
});

export const Inventory = mongoose.model<IInventoryItem>('Inventory', inventorySchema);