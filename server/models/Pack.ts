import mongoose, { Schema, Document } from 'mongoose';

export interface IPack extends Document {
    userId: mongoose.Types.ObjectId;
    type: 'basic' | 'elite' | 'legendary';
    itemIds: mongoose.Types.ObjectId[];
    openedAt: number;
    created: number;
}

const packSchema = new Schema<IPack>({
    userId: { type: mongoose.Schema.Types.ObjectId, ref: 'User', required: true },
    type: { 
        type: String, 
        enum: ['basic', 'elite', 'legendary'], 
        required: true 
    },
    itemIds: [{ 
        type: mongoose.Schema.Types.ObjectId, 
        ref: 'StoreItem' 
    }],
    openedAt: { type: Number, required: true },
    created: { type: Number, default: () => Math.floor(Date.now() / 1000) }
});

export const Pack = mongoose.model<IPack>('Pack', packSchema);