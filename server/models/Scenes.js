import mongoose from 'mongoose';

const sceneSchema = new mongoose.Schema({
  server_id: mongoose.Schema.Types.ObjectId,
  name: String,
  created_by: {
    type: mongoose.Schema.Types.ObjectId,
    ref: 'User'
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
});

export const Scene = mongoose.models.Scene || mongoose.model('Scene', sceneSchema);
