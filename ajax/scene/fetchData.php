<?php 
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';

$roomsCollection = $db->rooms;

header('Content-Type: application/json');

try {
    $id = $_GET['id'];

    // MongoDB uses the '_id' field by default. If your ID is stored differently, adjust accordingly.
    // This example assumes your IDs are strings. If using MongoDB's ObjectId, you'd need to wrap $id with new MongoDB\BSON\ObjectId($id)
    $room = $roomsCollection->findOne(['id' => $id], ['projection' => ['_id' => 0]]); // Exclude the MongoDB default '_id' from the result

    if ($room) {
        echo json_encode([
            'name' => $room->name,
            'numX' => $room->numX,
            'numY' => $room->numY,
            'items' => $room->items,
            'renscript' => $room->renscript
        ]);
    } else {
        echo json_encode(['error' => 'not_found']);
    }
} catch (Exception $e) {
    echo json_encode(['error' => $e->getMessage()]);
}
?>