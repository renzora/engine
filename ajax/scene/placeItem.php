<?php
header('Content-Type: application/json');
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
include 'calculateCollision.php';

if($auth) {
    $roomsCollection = $db->rooms;
    $room = $_POST['room'];
    $newItemData = json_decode($_POST['item'], true);

    $itemsJson = file_get_contents('../../assets/json/items.json');
    $items = json_decode($itemsJson, true);
    $item_id = $newItemData['id'];

    if(isset($items[$item_id])) {
        $roomDocument = $roomsCollection->findOne(['id' => $room]);

        if($roomDocument) {
            if ($roomDocument['uid'] == $user->id) {
                $currentItems = $roomDocument['items'];
                $currentItems[] = $newItemData;
                $updatedCollision = updateRoomWithItems($currentItems, $roomDocument['numX'], $roomDocument['numY']);
                
                $updateResult = $roomsCollection->updateOne(
                    ['id' => $room, 'uid' => $user->id],
                    ['$set' => ['items' => $currentItems, 'collision' => $updatedCollision]]
                );

                echo json_encode([
                    "status" => "success",
                    "message" => "Item added successfully",
                    "updatedRoom" => $currentItems,
                    "collisionMap" => $updatedCollision
                ]);
            } else {
                echo json_encode([
                    "status" => "error",
                    "message" => "User not authorized to modify this room"
                ]);
            }
        } else {
            echo json_encode([
                "status" => "error",
                "message" => "Room not found"
            ]);
        }
    } else {
        echo json_encode([
            "status" => "error",
            "message" => "Item ID not found in items.json"
        ]);
    }
} else {
    echo json_encode([
        "status" => "error",
        "message" => "User not authenticated"
    ]);
}
?>