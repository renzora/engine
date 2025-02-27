import mongoose, { Schema, Document } from 'mongoose';

export interface IStoreItem extends Document {
    categoryId: mongoose.Types.ObjectId;
    objectDataId: string;
    name: string;
    description: string;
    price: number;
    active: boolean;
    order: number;
    created: number;
    rarity: 'Common' | 'Uncommon' | 'Rare' | 'Epic' | 'Legendary';
}

const storeItemSchema = new Schema<IStoreItem>({
    categoryId: { type: mongoose.Schema.Types.ObjectId, ref: 'StoreCategory', required: true },
    objectDataId: { type: String, required: true },
    name: { type: String, required: true },
    description: { type: String },
    price: { type: Number, required: true },
    active: { type: Boolean, default: true },
    order: { type: Number, default: 0 },
    rarity: { 
        type: String, 
        enum: ['Common', 'Uncommon', 'Rare', 'Epic', 'Legendary'],
        default: 'Common'
    },
    created: { type: Number, default: () => Math.floor(Date.now() / 1000) }
});

export const StoreItem = mongoose.model<IStoreItem>('StoreItem', storeItemSchema);