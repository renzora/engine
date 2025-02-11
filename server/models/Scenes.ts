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
  editorLayers?: any;
  nodeData?: { [key: string]: { nodes: any[], connections: any[] } };
}

const sceneSchema = new Schema<IScene>({
  server_id: { type: mongoose.Types.ObjectId, required: true },
  name: { type: String, default: 'new scene' },
  created_by: { type: mongoose.Types.ObjectId, ref: 'User' },
  created_at: { type: Number, default: () => Date.now() },
  roomData: {
    type: Object,
    default: {
      items: [],
    },
  },
  public: { type: Number, default: 0 },
  width: { type: Number, default: 640 },
  height: { type: Number, default: 640 },
  startingX: { type: Number, default: 288 },
  startingY: { type: Number, default: 208 },
  bg: { type: String, default: '' },
  facing: { type: String, default: 's' },
  fireflys: { type: Number, default: 0 },
  clouds: { type: Number, default: 0 },
  rain: { type: Number, default: 0 },
  snow: { type: Number, default: 0 },
  order: { type: Number, default: 0 },
  editorLayers: {
    type: Array,
    default: [], 
  },
  nodeData: {
    type: Map,
    of: {
      nodes: [{ type: Object }],
      connections: [{ type: Object }]
    },
    default: {}
  }
});

export const Scene =
  mongoose.models.Scene || mongoose.model<IScene>('Scene', sceneSchema);