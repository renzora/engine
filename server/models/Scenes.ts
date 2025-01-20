import mongoose, { Schema, Document } from 'mongoose';

export interface IScene extends Document {
  server_id: mongoose.Types.ObjectId;
  name: string;
  created_by: mongoose.Types.ObjectId;
  created_at: number;
  roomData?: any;
  public?: number;
  width?: number;
  height?: number;
  startingX?: number;
  startingY?: number;
  bg?: string;
  facing?: string;
  fireflys?: number;
  clouds?: number;
  rain?: number;
  snow?: number;
  order?: number;
}

const sceneSchema = new Schema<IScene>({
  server_id: Schema.Types.ObjectId,
  name: String,
  created_by: {
    type: Schema.Types.ObjectId,
    ref: 'User',
  },
  created_at: Number,
  roomData: Object,
  public: Number,
  width: Number,
  height: Number,
  startingX: Number,
  startingY: Number,
  bg: String,
  facing: String,
  fireflys: Number,
  clouds: Number,
  rain: Number,
  snow: Number,
  order: Number,
});

export const Scene =
  mongoose.models.Scene || mongoose.model<IScene>('Scene', sceneSchema);
