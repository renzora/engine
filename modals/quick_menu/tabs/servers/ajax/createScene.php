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

$serverId = $data['serverId'] ?? null;
$name = $data['name'] ?? null;

if (!$serverId) {
    echo json_encode([
        'message' => 'Server ID not provided',
        'error' => true
    ]);
    exit();
}

if (!$name) {
    echo json_encode([
        'message' => 'Scene name not provided',
        'error' => true
    ]);
    exit();
}

try {
    $collection = $db->scenes;
    $result = $collection->insertOne([
        'server_id' => new MongoDB\BSON\ObjectId($serverId),
        'name' => $name,
        'created_by' => (int)$user->id,
        'created_at' => time(),
        'roomData' => new stdClass() // Set roomData as an empty object
    ]);

    echo json_encode([
        'message' => 'success',
        'scene_id' => (string)$result->getInsertedId(),
        'server_id' => $serverId
    ]);

    // Log success response
    error_log("Scene created successfully: " . print_r($result, true));
} catch (Exception $e) {
    echo json_encode([
        'message' => 'Error creating scene',
        'error' => $e->getMessage()
    ]);

    // Log the error
    error_log("Error creating scene: " . $e->getMessage());
}
?>
