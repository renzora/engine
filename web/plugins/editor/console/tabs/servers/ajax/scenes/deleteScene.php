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

$sceneId = $data['id'] ?? null;
$userId = (int) $user->id; // Assuming playerId is stored in the JWT payload

if ($sceneId) {
    try {
        $collection = $db->scenes;

        // Fetch the scene to check ownership and get server ID
        $scene = $collection->findOne(['_id' => new MongoDB\BSON\ObjectId($sceneId)]);

        if ($scene && $scene['created_by'] === $userId) {
            $serverId = (string)$scene['server_id'];
            $deleteResult = $collection->deleteOne(['_id' => new MongoDB\BSON\ObjectId($sceneId)]);

            if ($deleteResult->getDeletedCount() > 0) {
                echo json_encode(['message' => 'success', 'server_id' => $serverId]);
            } else {
                echo json_encode(['message' => 'Error deleting scene']);
            }
        } else {
            echo json_encode(['message' => 'Unauthorized']);
        }
    } catch (Exception $e) {
        echo json_encode([
            'message' => 'Error deleting scene',
            'error' => $e->getMessage()
        ]);
    }
} else {
    echo json_encode(['message' => 'Invalid input']);
}
?>
