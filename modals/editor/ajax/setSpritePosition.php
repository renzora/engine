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

$input = file_get_contents('php://input');
$data = json_decode($input, true);

$sceneId = $data['sceneId'] ?? null;
$startingX = $data['startingX'] ?? null; // Tile-based X coordinate
$startingY = $data['startingY'] ?? null; // Tile-based Y coordinate

if (!$sceneId || $startingX === null || $startingY === null) {
    echo json_encode([
        'message' => 'Invalid input data',
        'error' => true
    ]);
    exit();
}

try {
    $collection = $db->scenes;

    // Convert tile-based coordinates to pixel position
    $startingX = (int)(round(($startingX * 16) / 16) * 16);
    $startingY = (int)(round(($startingY * 16) / 16) * 16);

    $result = $collection->updateOne(
        ['_id' => new MongoDB\BSON\ObjectId($sceneId)],
        ['$set' => [
            'startingX' => $startingX,
            'startingY' => $startingY
        ]]
    );

    echo json_encode([
        'message' => 'Starting position updated successfully.',
        'startingX' => $startingX,
        'startingY' => $startingY,
        'error' => false
    ]);
} catch (Exception $e) {
    echo json_encode([
        'message' => 'Error updating scene starting position',
        'error' => $e->getMessage()
    ]);
}
?>
