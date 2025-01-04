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
$width = $data['width'] ?? null;
$height = $data['height'] ?? null;

if (!$sceneId || $width === null || $height === null) {
    echo json_encode([
        'message' => 'Invalid input data',
        'error' => true
    ]);
    exit();
}

try {
    $collection = $db->scenes;

    $result = $collection->updateOne(
        ['_id' => new MongoDB\BSON\ObjectId($sceneId)],
        ['$set' => [
            'width' => (int)$width,
            'height' => (int)$height
        ]]
    );

    echo json_encode([
        'message' => 'Scene dimensions updated successfully.',
        'width' => $width,
        'height' => $height,
        'error' => false
    ]);
} catch (Exception $e) {
    echo json_encode([
        'message' => 'Error updating scene dimensions',
        'error' => $e->getMessage()
    ]);
}
?>
