<?php
header('Content-Type: application/json');
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';

$roomsCollection = $db->rooms; // Assuming the collection is named 'rooms'
$inventoryCollection = $db->inventory; // Assuming the collection for inventory

include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
include 'calculateCollision.php';

if($auth) {
    $x = $_POST['x'];
    $y = $_POST['y'];
    $itemId = $_POST['item_id'];
    $room_id = $_POST['room_id'];

    $room = $roomsCollection->findOne(['id' => $room_id, 'uid' => $user->id]);

    if(!$room) {
        echo json_encode(['status' => 'error', 'message' => 'Room does not exist']);
    } else {
        $itemsData = $room['items'];
        $pickedUpItemId = null;

        foreach ($itemsData as $key => $item) {
            if ($item['id'] == $itemId) {
                foreach ($item['position'] as $position) {
                    if ($position['x'] == $x && $position['y'] == $y) {
                        $pickedUpItemId = $item['id'];
                        unset($itemsData[$key]);
                        break 2;
                    }
                }
            }
        }

        $itemsData = array_values($itemsData);

        $updatedCollision = updateRoomWithItems($itemsData, $room['numX'], $room['numY']);
        $roomsCollection->updateOne(['id' => $room_id], ['$set' => ['items' => $itemsData, 'collision' => $updatedCollision]]);

        if($pickedUpItemId !== null) {
            $inventoryItem = $inventoryCollection->findOne(['item_id' => $pickedUpItemId, 'uid' => $user->id]);

            if($inventoryItem) {
                $newQuantity = $inventoryItem['quantity'] + 1;
                $inventoryCollection->updateOne(['item_id' => $pickedUpItemId, 'uid' => $user->id], ['$set' => ['quantity' => $newQuantity]]);
            } else {
                $inventoryCollection->insertOne(['item_id' => $pickedUpItemId, 'uid' => $user->id, 'quantity' => 1]);
            }
        }

        echo json_encode([
            'status' => 'item_removed',
            'updatedRoom' => $itemsData,
            'collisionMap' => $updatedCollision
        ]);
    }
}
?>