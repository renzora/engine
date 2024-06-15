<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';

if($auth) {
    $marketCollection = $db->market;
    $json = file_get_contents('../assets/json/items.json');
    $items = json_decode($json, true);

    foreach ($items as $item_id => $item_data) {
        $existingItem = $marketCollection->findOne(['item_id' => $item_id]);

        if(!$existingItem) {
            $marketCollection->insertOne([
                'item_id' => $item_id,
                'item_name' => $item_data['name'],
                'item_info' => $item_data['info'],
                'price' => 10,
                'cat' => $item_data['category'],
                'type' => $item_data['type'],
                'preview_room' => 0,
                'active' => 1
            ]);
        }
    }
}
?>