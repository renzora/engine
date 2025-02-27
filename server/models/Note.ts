import mongoose, { Schema, Document } from 'mongoose';

export interface INote extends Document {
  profile_uid: mongoose.Types.ObjectId;
  note: string;
  author: string;
  time: number;
}

const noteSchema = new Schema<INote>({
  profile_uid: { type: Schema.Types.ObjectId, ref: 'User', required: true },
  note: { type: String, required: true },
  author: { type: String, required: true },
  time: { type: Number, default: () => Math.floor(Date.now() / 1000) },
});

export const Note = mongoose.models.Note || mongoose.model<INote>('Note', noteSchema);