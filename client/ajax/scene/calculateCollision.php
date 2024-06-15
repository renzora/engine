<?php
function updateRoomWithItems($jsonRoomData, $gridX, $gridY) {
    $collisionMap = array_fill(0, $gridY, array_fill(0, $gridX, 1));

    $itemsJsonPath = '../../assets/json/items.json';
    $itemLayoutsJson = file_get_contents($itemsJsonPath);
    $itemLayouts = json_decode($itemLayoutsJson, true);

    $jsonInput = $jsonRoomData;
    $inputData = json_decode($jsonInput, true);

    // Array to keep track of the highest zindex at each position
    $highestZindexAtPosition = array_fill(0, $gridY, array_fill(0, $gridY, -1));

    foreach ($inputData['items'] as $item) {
        $itemId = $item['id'];
        $itemPositions = $item['position'];

        if (isset($itemLayouts[$itemId])) {
            foreach ($itemPositions as $index => $position) {
                $layout = $itemLayouts[$itemId]['layout'][$index];
                $x = (int)$position['x'];
                $y = (int)$position['y'];
                $zindex = (int)$position['zindex'];

                // Check if this item has the highest zindex at this position
                if ($zindex > $highestZindexAtPosition[$y][$x]) {
                    $highestZindexAtPosition[$y][$x] = $zindex;
                    $collisionMap[$y][$x] = $layout['walk'] ? 1 : 0;
                }
            }
        }
    }

    return $collisionMap;
}
?>