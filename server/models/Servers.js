import mongoose from 'mongoose';

const serverSchema = new mongoose.Schema({
    name: {
        type: String,
        required: true,
    },
    public: {
        type: Boolean,
        default: true,
    },
    events: {
        type: Boolean,
        default: false,
    },
    created_by: {
        type: mongoose.Schema.Types.ObjectId,
        ref: 'User',
        required: true,
    },
    created_at: {
        type: Date,
        default: Date.now,
    },
});

const Servers = mongoose.models.Server || mongoose.model('Servers', serverSchema);

export { Servers };
