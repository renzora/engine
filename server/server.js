const express = require('express');
const path = require('path');
const cors = require('cors');
const cookieParser = require('cookie-parser');
const nunjucks = require('nunjucks');
const { connectDB } = require('./database');
const authMiddleware = require('./middleware/auth');
const http = require('http');
const { initializeWebSocket } = require('./websocket');

const app = express();
const server = http.createServer(app);

// Configure Nunjucks
// In server.js
const viewPaths = [
    path.join(__dirname, '../client'),
    path.join(__dirname, '../client/plugins')
];

nunjucks.configure(viewPaths, {
    autoescape: true,
    express: app,
    watch: true,
    noCache: process.env.NODE_ENV === 'development'
});

// Middleware
app.use(cors({
    origin: process.env.ALLOWED_ORIGINS?.split(',') || ['http://localhost', 'http://localhost:80'],
    credentials: true
}));
app.use(cookieParser());
app.use(express.json());
app.use(express.urlencoded({ extended: true }));

// Auth middleware
app.use(authMiddleware);

// Routes
const authRoutes = require('./routes/auth');
const pluginRoutes = require('./routes/plugin');
const scenesRoutes = require('./routes/scenes');


app.use((req, res, next) => {
    console.log('Incoming request:', {
        method: req.method,
        url: req.url,
        path: req.path
    });
    next();
});

app.use('/api/auth', authRoutes);
app.use('/api/plugins', pluginRoutes);
app.use('/api/scenes', scenesRoutes);

// Health check endpoint
app.get('/health', (req, res) => {
    res.status(200).json({ status: 'ok' });
});

// Error handling middleware
app.use((err, req, res, next) => {
    console.error(err.stack);
    res.status(500).json({
        message: 'Something went wrong!',
        error: process.env.NODE_ENV === 'development' ? err.message : {}
    });
});

// Initialize WebSocket
initializeWebSocket(server);

// Connect to MongoDB and start server
const PORT = process.env.PORT || 3000;

async function startServer() {
    try {
        await connectDB();
        server.listen(PORT, '0.0.0.0', () => {  // Added host binding
            console.log(`Server running on port ${PORT}`);
        });
    } catch (error) {
        console.error('Failed to start server:', error);
        process.exit(1);
    }
}

startServer();

// Handle graceful shutdown
process.on('SIGTERM', () => {
    console.log('SIGTERM received. Shutting down gracefully...');
    server.close(() => {
        console.log('Server closed');
        process.exit(0);
    });
});

// Handle uncaught exceptions
process.on('uncaughtException', (error) => {
    console.error('Uncaught Exception:', error);
    process.exit(1);
});

// Handle unhandled promise rejections
process.on('unhandledRejection', (reason, promise) => {
    console.error('Unhandled Rejection at:', promise, 'reason:', reason);
    process.exit(1);
});