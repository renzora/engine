const express = require('express');
const router = express.Router();
const mongoose = require('mongoose');
const Scene = require('../models/Scene');

router.get('/', async (req, res) => {
    try {
        const sceneId = req.query.scene_id;
        
        if (!sceneId) {
            return res.status(400).json({ message: 'Scene ID is required' });
        }

        if (!mongoose.Types.ObjectId.isValid(sceneId)) {
            return res.status(400).json({ message: 'Invalid scene ID format' });
        }

        const scene = await Scene.findById(sceneId).exec();
        
        if (!scene) {
            return res.status(404).json({ message: 'Scene not found' });
        }

        res.json({
            message: 'success',
            roomData: scene.roomData,
            sceneid: scene._id,
            server_id: scene.server_id,
            width: scene.width,
            height: scene.height,
            startingX: scene.startingX,
            startingY: scene.startingY,
            bg: scene.bg
        });
    } catch (error) {
        console.error('Scene fetch error:', error);
        res.status(500).json({ 
            message: 'Error fetching scene', 
            error: error.message 
        });
    }
});

module.exports = router;