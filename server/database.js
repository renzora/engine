const mongoose = require('mongoose');
require('dotenv').config();

const mongoUrl = `mongodb://${encodeURIComponent(process.env.MONGO_USERNAME)}:${encodeURIComponent(process.env.MONGO_PASSWORD)}@${process.env.MONGO_HOST}:${process.env.MONGO_PORT || '27017'}/${process.env.MONGO_DATABASE}?authSource=admin`;

/**
 * Connect to MongoDB using Mongoose
 * @returns {Promise<mongoose.Connection>}
 */
async function connectDB() {
    try {
        console.log(`Connecting to MongoDB at ${process.env.MONGO_HOST}:${process.env.MONGO_PORT}...`);
        await mongoose.connect(mongoUrl, {
            useNewUrlParser: true,
            useUnifiedTopology: true,
            serverSelectionTimeoutMS: 5000,
            socketTimeoutMS: 45000,
        });

        mongoose.connection.on('error', (err) => {
            console.error('MongoDB connection error:', err);
        });

        mongoose.connection.on('disconnected', () => {
            console.log('MongoDB connection disconnected');
        });

        console.log(`Connected to MongoDB database: ${process.env.MONGO_DATABASE}`);
        return mongoose.connection;
    } catch (error) {
        console.error('MongoDB connection error:', error.message);
        process.exit(1);
    }
}

module.exports = { connectDB };