<?php
header('Content-type: application/json');
include $_SERVER['DOCUMENT_ROOT'] . '/config/db.php';

$scene_id = $_POST['scene_id'] ?? null;

if (!$scene_id) {
    echo json_encode([
        'message' => 'Scene ID not provided',
        'error' => true
    ]);
    exit();
}

try {
    // Ensure scene_id is a valid ObjectId
    $scene_id = new MongoDB\BSON\ObjectId($scene_id);
    $collection = $db->scenes;
    $scene = $collection->findOne(['_id' => $scene_id]);

    if ($scene) {
        echo json_encode([
            'message' => 'success',
            'name' => $scene['name'],
            'roomData' => $scene['roomData'],
            'sceneid' => (string) $scene['_id'],
            'server_id' => isset($scene['server_id']) ? (string) $scene['server_id'] : null,
            'startingX' => isset($scene['startingX']) ? (int) $scene['startingX'] : 0,
            'startingY' => isset($scene['startingY']) ? (int) $scene['startingY'] : 0,
            'width' => isset($scene['width']) ? (int) $scene['width'] : 1280,
            'height' => isset($scene['height']) ? (int) $scene['height'] : 944,
            'bg' => isset($scene['bg']) ? $scene['bg'] : 'grass',
            'facing' => isset($scene['facing']) ? $scene['facing'] : 'S'
        ]);
    } else {
        echo json_encode([
            'message' => 'Scene not found',
            'error' => true
        ]);
    }
} catch (Exception $e) {
    echo json_encode([
        'message' => 'Error fetching scene data',
        'error' => $e->getMessage()
    ]);
}
?>
