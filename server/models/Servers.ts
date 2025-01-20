import mongoose, { Schema, Document } from 'mongoose';

export interface IServer extends Document {
  name: string;
  public: boolean;
  events: boolean;
  created_by: mongoose.Types.ObjectId;
  created_at: Date;
}

const serverSchema = new Schema<IServer>({
  name: { type: String, required: true },
  public: { type: Boolean, default: true },
  events: { type: Boolean, default: false },
  created_by: { type: Schema.Types.ObjectId, ref: 'User', required: true },
  created_at: { type: Date, default: Date.now },
});

export const Servers =
  mongoose.models.Servers || mongoose.model<IServer>('Servers', serverSchema);
