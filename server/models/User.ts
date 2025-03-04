import mongoose, { Schema, Document } from 'mongoose';

export interface IUser extends Document {
    username: string;
    password: string;
    email: string;
    permissions: string[];
    coins: number;  // Add this line
    created: number;
}

const userSchema = new Schema<IUser>({
    username: {
        type: String,
        required: true,
        unique: true,
        match: /^[a-zA-Z0-9._]+$/,
        minlength: 3,
        maxlength: 20,
    },
    password: { 
        type: String, 
        required: true 
    },
    email: { 
        type: String, 
        required: true 
    },
    permissions: [{ 
        type: String 
    }],
    coins: {
        type: Number,
        default: 0,
        min: 0
    },
    created: { 
        type: Number, 
        default: () => Math.floor(Date.now() / 1000) 
    }
});

export const User = mongoose.models.User || mongoose.model<IUser>('User', userSchema);