import mongoose, { Schema, Document } from 'mongoose';

export interface IPackOpening extends Document {
    userId: mongoose.Types.ObjectId;
    packId: mongoose.Types.ObjectId;
    items: Array<{
        itemId: mongoose.Types.ObjectId;
        name: string;
        objectDataId: string;
        rarity: 'Common' | 'Uncommon' | 'Rare' | 'Epic' | 'Legendary';
    }>;
    opened: boolean;
    openedAt?: number;
    created: number;
}

const packOpeningSchema = new Schema<IPackOpening>({
    userId: { type: mongoose.Schema.Types.ObjectId, ref: 'User', required: true },
    packId: { type: mongoose.Schema.Types.ObjectId, ref: 'Pack', required: true },
    items: [{
        itemId: { type: mongoose.Schema.Types.ObjectId, ref: 'StoreItem' },
        name: { type: String, required: true },
        objectDataId: { type: String, required: true },
        rarity: { 
            type: String, 
            enum: ['Common', 'Uncommon', 'Rare', 'Epic', 'Legendary'], 
            required: true 
        }
    }],
    opened: { type: Boolean, default: false },
    openedAt: { type: Number },
    created: { type: Number, default: () => Math.floor(Date.now() / 1000) }
});

export const PackOpening = mongoose.model<IPackOpening>('PackOpening', packOpeningSchema);