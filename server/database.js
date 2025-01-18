import mongoose from 'mongoose';

const connectDB = async () => {
    try {
        const { MONGO_HOST, MONGO_PORT, MONGO_DATABASE, MONGO_USERNAME, MONGO_PASSWORD } = process.env;

        let mongoURI;
        if (MONGO_USERNAME && MONGO_PASSWORD) {
            mongoURI = `mongodb://${MONGO_USERNAME}:${encodeURIComponent(
                MONGO_PASSWORD
            )}@${MONGO_HOST}:${MONGO_PORT}/${MONGO_DATABASE}?authSource=admin`;
        } else {
            mongoURI = `mongodb://${MONGO_HOST}:${MONGO_PORT}/${MONGO_DATABASE}`;
        }

        const conn = await mongoose.connect(mongoURI, {
            useNewUrlParser: true,
            useUnifiedTopology: true,
        });

        console.log(`MongoDB Connected: ${conn.connection.host}`);
    } catch (err) {
        console.error(`Error: ${err.message}`);
        process.exit(1);
    }
};

export { connectDB };
