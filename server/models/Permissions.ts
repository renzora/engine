import mongoose, { Schema, Document } from 'mongoose';

export interface IPermission extends Document {
    key: string;
    description: string;
}

const permissionSchema = new Schema<IPermission>({
    key: { 
        type: String, 
        required: true, 
        unique: true 
    },
    description: { 
        type: String, 
        required: true 
    }
});

export const Permission = mongoose.models.Permission || mongoose.model<IPermission>('Permission', permissionSchema);