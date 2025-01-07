<?php
header('Content-type: application/json');
include $_SERVER['DOCUMENT_ROOT'] . '/config/db.php';

if (!$auth) {
    echo json_encode([
        'message' => 'Unauthorized',
        'error' => true
    ]);
    exit();
}

// Retrieve the raw POST data
$input = file_get_contents('php://input');
$data = json_decode($input, true);

$serverId = $data['id'] ?? null;
$userId = (int) $user->id; // Assuming playerId is stored in the JWT payload

if ($serverId) {
    try {
        $serverCollection = $db->servers;
        $scenesCollection = $db->scenes;

        // Fetch the server to check ownership
        $server = $serverCollection->findOne(['_id' => new MongoDB\BSON\ObjectId($serverId)]);

        if ($server && $server->created_by === $userId) {
            // Delete scenes related to the server
            $deleteScenesResult = $scenesCollection->deleteMany(['server_id' => new MongoDB\BSON\ObjectId($serverId)]);

            // Delete the server
            $deleteServerResult = $serverCollection->deleteOne(['_id' => new MongoDB\BSON\ObjectId($serverId)]);

            if ($deleteServerResult->getDeletedCount() > 0) {
                echo json_encode(['message' => 'success']);
            } else {
                echo json_encode(['message' => 'Error deleting server']);
            }
        } else {
            echo json_encode(['message' => 'Unauthorized']);
        }
    } catch (Exception $e) {
        echo json_encode([
            'message' => 'Error deleting server',
            'error' => $e->getMessage()
        ]);
    }
} else {
    echo json_encode(['message' => 'Invalid input']);
}
?>
