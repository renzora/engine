// server/routes/plugin.js
const express = require('express');
const router = express.Router();
const path = require('path');
const fs = require('fs').promises;

router.get('*', async (req, res) => {
    try {
        // Remove '/api/plugins' from the path to get the relative path
        const pluginPath = req.path;
        const templatePath = `plugins${pluginPath}`;
        
        // Construct absolute path
        const fullPath = path.join(__dirname, '../../client', templatePath);
        
        console.log('Plugin request:', {
            requestPath: req.path,
            templatePath,
            fullPath
        });

        // Check if file exists
        try {
            await fs.access(fullPath);
            console.log('Template found:', fullPath);
        } catch (err) {
            console.error('Template not found:', fullPath);
            return res.status(404).json({
                error: 'Template not found',
                path: templatePath
            });
        }

        // Render the template
        res.render(templatePath, {
            auth: req.auth,
            // Extract plugin name from path for id
            id: req.path.split('/')[1] + '_window'
        });

    } catch (error) {
        console.error('Plugin route error:', {
            error: error.message,
            stack: error.stack
        });
        
        res.status(500).json({
            error: 'Failed to load plugin',
            message: error.message
        });
    }
});

module.exports = router;