// server/routes/ajax.js
const express = require('express');
const path = require('path');
const fs = require('fs').promises;
const router = express.Router();

router.post('*', async (req, res) => {
    try {
        // Remove '/api/ajax' from the beginning of the path
        const relativePath = req.path;
        
        // Construct the full path to the template in the client directory
        const templatePath = path.join('client', relativePath);
        
        // Check if template exists
        try {
            await fs.access(path.join(process.cwd(), templatePath));
        } catch (error) {
            return res.status(404).send('Template not found');
        }

        // Prevent directory traversal
        const normalizedPath = path.normalize(templatePath);
        if (!normalizedPath.startsWith('client/')) {
            return res.status(403).send('Access denied');
        }

        // Render the template using nunjucks
        res.render(templatePath, {
            // Add any template data here if needed
        });

    } catch (error) {
        console.error('Template rendering error:', error);
        res.status(500).send('Failed to render template');
    }
});

module.exports = router;