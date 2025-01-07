<?php
header('Content-Type: application/json');
include $_SERVER['DOCUMENT_ROOT'] . '/config/db.php';

use Firebase\JWT\JWT;
use Firebase\JWT\Key;

function generateSessionId() {
    // Generate a random session ID
    return uniqid('session_', true);
}

// Check for the presence of the token
if (!isset($_COOKIE['renaccount'])) {
    // Generate a session ID and return it
    $sessionId = generateSessionId();

    // Optionally store the session ID in the database if needed
    // Example:
    // $db->selectCollection('sessions')->insertOne(['session_id' => $sessionId, 'created_at' => new MongoDB\BSON\UTCDateTime()]);

    echo json_encode(['playerid' => $sessionId]);
    exit;
}

try {
    // Decode the JWT
    $decoded = JWT::decode($_COOKIE['renaccount'], new Key($_ENV['JWT_KEY'], 'HS256'));

    // Validate the player ID
    if (!isset($decoded->id) || !is_numeric($decoded->id)) {
        throw new Exception('Invalid player ID format');
    }

    // Use `$db` to access the `users` collection
    $playerId = (int)$decoded->id;
    $user = $db->selectCollection('users')->findOne(['_id' => $playerId]);

    if (!$user) {
        throw new Exception('Player not found');
    }

    // Respond with the public player ID
    echo json_encode(['playerid' => $playerId]);
    exit;

} catch (\Firebase\JWT\ExpiredException $e) {
    // Handle expired token gracefully
    echo json_encode(['playerid' => null]);
    exit;

} catch (Exception $e) {
    // Generate a session ID if an error occurs
    $sessionId = generateSessionId();

    // Optionally store the session ID in the database if needed
    // Example:
    // $db->selectCollection('sessions')->insertOne(['session_id' => $sessionId, 'created_at' => new MongoDB\BSON\UTCDateTime()]);

    echo json_encode(['playerid' => $sessionId]);
    exit;
}
