import mongoose, { Schema, Document } from 'mongoose';

export interface IStoreCategory extends Document {
    name: string;
    description: string;
    active: boolean;
    order: number;
    created: number;
}

const storeCategorySchema = new Schema<IStoreCategory>({
    name: { type: String, required: true },
    description: { type: String },
    active: { type: Boolean, default: true },
    order: { type: Number, default: 0 },
    created: { type: Number, default: () => Math.floor(Date.now() / 1000) }
});

export const StoreCategory = mongoose.model<IStoreCategory>('StoreCategory', storeCategorySchema);