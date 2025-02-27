import mongoose, { Schema, Document } from 'mongoose';

export interface IAccessCode extends Document {
    code: string;
    createdBy: mongoose.Types.ObjectId;
    createdAt: number;
    usedBy: mongoose.Types.ObjectId | null;
    usedAt: number | null;
    isActive: boolean;
}

const accessCodeSchema = new Schema<IAccessCode>({
    code: {
        type: String,
        required: true,
        unique: true,
        uppercase: true,
        minlength: 6,
        maxlength: 12
    },
    createdBy: {
        type: mongoose.Schema.Types.ObjectId,
        ref: 'User',
        required: true
    },
    createdAt: {
        type: Number,
        default: () => Math.floor(Date.now() / 1000)
    },
    usedBy: {
        type: mongoose.Schema.Types.ObjectId,
        ref: 'User',
        default: null
    },
    usedAt: {
        type: Number,
        default: null
    },
    isActive: {
        type: Boolean,
        default: true
    }
});

export const AccessCode = mongoose.models.AccessCode || mongoose.model<IAccessCode>('AccessCode', accessCodeSchema);