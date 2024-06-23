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

$serverId = $data['id'] ?? null;
$name = $data['name'] ?? '';
$userId = (int) $user->id; // Assuming playerId is stored in the JWT payload

if ($serverId && $name) {
    try {
        $collection = $db->servers;

        // Fetch the server to check ownership
        $server = $collection->findOne(['_id' => new MongoDB\BSON\ObjectId($serverId)]);

        if ($server && $server['created_by'] === $userId) {
            $updateResult = $collection->updateOne(
                ['_id' => new MongoDB\BSON\ObjectId($serverId)],
                ['$set' => ['name' => $name]]
            );

            if ($updateResult->getModifiedCount() > 0) {
                echo json_encode(['message' => 'success']);
            } else {
                echo json_encode([
                    'message' => 'success',
                    'error' => 'No documents were modified.'
                ]);
            }
        } else {
            echo json_encode([
                'message' => 'Unauthorized',
                'error' => 'You do not have permission to update this server.'
            ]);
        }
    } catch (Exception $e) {
        echo json_encode([
            'message' => 'Error updating server',
            'error' => $e->getMessage()
        ]);
    }
} else {
    echo json_encode([
        'message' => 'Invalid input',
        'error' => 'Server ID or name is missing.'
    ]);
}
?>
