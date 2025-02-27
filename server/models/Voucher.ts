import mongoose, { Schema, Document } from 'mongoose';

export interface IVoucher extends Document {
    code: string;
    coins: number;
    active: boolean;
    usedBy?: mongoose.Types.ObjectId;
    usedAt?: number;
    created: number;
    expiresAt?: number;
}

const voucherSchema = new Schema<IVoucher>({
    code: { type: String, required: true, unique: true },
    coins: { type: Number, required: true },
    active: { type: Boolean, default: true },
    usedBy: { type: mongoose.Schema.Types.ObjectId, ref: 'User' },
    usedAt: { type: Number },
    created: { type: Number, default: () => Math.floor(Date.now() / 1000) },
    expiresAt: { type: Number }
});

export const Voucher = mongoose.model<IVoucher>('Voucher', voucherSchema);