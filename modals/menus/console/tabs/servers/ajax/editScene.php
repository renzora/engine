<?php
header('Content-type: application/json');
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';

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
$name = $data['name'] ?? '';
$userId = (int) $user->id; // Assuming playerId is stored in the JWT payload

if ($sceneId && $name) {
    try {
        $collection = $db->scenes;

        // Fetch the scene to check ownership and get server ID
        $scene = $collection->findOne(['_id' => new MongoDB\BSON\ObjectId($sceneId)]);

        if ($scene && $scene['created_by'] === $userId) {
            $updateResult = $collection->updateOne(
                ['_id' => new MongoDB\BSON\ObjectId($sceneId)],
                ['$set' => ['name' => $name]]
            );

            if ($updateResult->getModifiedCount() > 0) {
                echo json_encode(['message' => 'success', 'server_id' => (string)$scene['server_id']]);
            } else {
                echo json_encode(['message' => 'No documents were modified', 'server_id' => (string)$scene['server_id']]);
            }
        } else {
            echo json_encode(['message' => 'Unauthorized']);
        }
    } catch (Exception $e) {
        echo json_encode([
            'message' => 'Error updating scene',
            'error' => $e->getMessage()
        ]);
    }
} else {
    echo json_encode(['message' => 'Invalid input']);
}
?>
