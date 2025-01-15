const jwt = require('jsonwebtoken');
require('dotenv').config();

const authMiddleware = (req, res, next) => {
    const token = req.cookies.renaccount;
    
    if (token) {
        try {
            const decoded = jwt.verify(token, process.env.JWT_SECRET);
            req.auth = decoded;
        } catch (err) {
            req.auth = null;
        }
    } else {
        req.auth = null;
    }
    
    next();
};

module.exports = authMiddleware;