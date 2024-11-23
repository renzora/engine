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
$isPublic = $data['isPublic'] ?? true; // New field to check if the scene should be public, defaults to true

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
    
    // Set public to 1 if isPublic is true, otherwise 0
    $publicValue = $isPublic ? 1 : 0;
    
    $result = $collection->insertOne([
        'server_id' => new MongoDB\BSON\ObjectId($serverId),
        'name' => $name,
        'created_by' => (int)$user->id,
        'created_at' => time(),
        'roomData' => new stdClass(), // Set roomData as an empty object
        'public' => $publicValue, // Add the public field with the determined value
        'width' => 1280,
        'height' => 944,
        'startingX' => 0, // Add startingX with a value of 0
        'startingY' => 0,  // Add startingY with a value of 0
        'bg' => 'grass',
        'facing' => 's'
    ]);

    // Get the inserted scene's ID
    $newSceneId = (string)$result->getInsertedId();

    // Properly encode the success response
    echo json_encode([
        'message' => 'success',
        'scene' => [
            'id' => $newSceneId,
            'name' => $name,
            'server_id' => $serverId,
            'created_by' => (int)$user->id,
            'created_at' => time(),
            'public' => $publicValue, // Include public in the response
            'width' => 1280,
            'height' => 944,
            'startingX' => 0, // Include startingX in the response
            'startingY' => 0,  // Include startingY in the response
            'bg' => 'grass',
            'facing' => 's'
        ],
        'server_id' => $serverId,
        'error' => false // Ensure 'error' is false to indicate success
    ]);

    // Log success response
    error_log("Scene created successfully: " . print_r($result, true));
} catch (Exception $e) {
    // Encode the error response
    echo json_encode([
        'message' => 'Error creating scene',
        'error' => $e->getMessage()
    ]);

    // Log the error
    error_log("Error creating scene: " . $e->getMessage());
}
?>
