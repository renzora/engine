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

// Log the received data for debugging
error_log("Received data: " . print_r($data, true));

$serverId = $data['serverId'] ?? null;

if (!$serverId) {
    echo json_encode([
        'message' => 'Server ID not provided',
        'error' => true
    ]);
    exit();
}

try {
    $collection = $db->scenes;
    $scenes = $collection->find(['server_id' => new MongoDB\BSON\ObjectId($serverId)])->toArray();

    // Convert MongoDB ObjectId to string
    $scenes = array_map(function($scene) {
        $scene['_id'] = (string) $scene['_id'];
        return $scene;
    }, $scenes);

    echo json_encode([
        'message' => 'success',
        'scenes' => $scenes
    ]);

    // Log success response
    error_log("Scenes loaded successfully: " . print_r($scenes, true));
} catch (Exception $e) {
    echo json_encode([
        'message' => 'Error loading scenes',
        'error' => $e->getMessage()
    ]);

    // Log the error
    error_log("Error loading scenes: " . $e->getMessage());
}
?>
