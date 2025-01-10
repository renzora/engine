<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config/db.php';

ini_set('display_errors', 1);
ini_set('display_startup_errors', 1);
error_reporting(E_ALL);

header('Content-Type: application/json');

if ($auth) {
    if ($_SERVER['REQUEST_METHOD'] == 'POST') {
        try {
            // Log the received POST request
            $timestamp = date("Y-m-d H:i:s");

            // Get the JSON input
            $input = json_decode(file_get_contents('php://input'), true);
            if (json_last_error() !== JSON_ERROR_NONE) {
                throw new Exception('Failed to decode JSON input: ' . json_last_error_msg());
            }

            // Extract data from the input
            $newObject = $input['newObject'];
            $aCoords = $input['newObject']['a'];
            $bCoords = $input['newObject']['b'];
            $imageDataBase64 = $input['imageData'];

            // Decode the base64 image data
            $imageData = base64_decode(preg_replace('#^data:image/\w+;base64,#i', '', $imageDataBase64));
            if ($imageData === false) {
                throw new Exception('Failed to decode base64 image data.');
            }

            // Create a new image from the decoded image data
            $uploadedImage = imagecreatefromstring($imageData);
            if (!$uploadedImage) {
                throw new Exception('Failed to create image from uploaded data.');
            }

            // Load existing object data
            $objectDataFile = $_SERVER['DOCUMENT_ROOT'] . '/assets/json/objectData.json';
            if (!file_exists($objectDataFile)) {
                throw new Exception('Object data file not found.');
            }

            $objectData = json_decode(file_get_contents($objectDataFile), true);
            if (json_last_error() !== JSON_ERROR_NONE) {
                throw new Exception('Failed to decode object data JSON: ' . json_last_error_msg());
            }

            // Load the meta tile count
            $metaDataFile = $_SERVER['DOCUMENT_ROOT'] . '/assets/json/meta.json';
            if (!file_exists($metaDataFile)) {
                throw new Exception('Meta data file not found.');
            }

            $metaData = json_decode(file_get_contents($metaDataFile), true);
            if (json_last_error() !== JSON_ERROR_NONE) {
                throw new Exception('Failed to decode meta data JSON: ' . json_last_error_msg());
            }

            // Generate a unique ID for the new object
            $uniqueId = uniqid();

            // Calculate the initial index for the new tiles
            $initialTileIndex = $metaData['tile_count'];

            // Populate the 'i' field with the correct index range as a string
            $startIndex = $initialTileIndex;
            $endIndex = $initialTileIndex + count($aCoords) - 1;
            $newObject['i'] = ["{$startIndex}-{$endIndex}"];

            // Calculate the row (`b`) and column (`a`) counts based on the provided coordinates
            $uniqueXValues = array_unique($aCoords);
            $uniqueYValues = array_unique($bCoords);

            $columnCount = count($uniqueXValues);  // This becomes the value for `a`
            $rowCount = count($uniqueYValues);     // This becomes the value for `b`

            $newObject['a'] = $columnCount;
            $newObject['b'] = $rowCount;

            // Add the new object to the items array with the unique ID as the key
            $objectData[$uniqueId] = [$newObject];

            // Modify 'a' and 'b' values in the object data before saving the JSON
            $objectDataWithAdjustedAB = $objectData;

            // Adjust the 'a' and 'b' values for the new object inside the objectData
            $objectDataWithAdjustedAB[$uniqueId][0]['a'] -= 1;
            $objectDataWithAdjustedAB[$uniqueId][0]['b'] -= 1;

            // Update the tile count in the meta section
            $metaData['tile_count'] += count($aCoords);

            // Save updated object data with adjusted 'a' and 'b' values
            if (file_put_contents($objectDataFile, json_encode($objectDataWithAdjustedAB)) === false) {
                throw new Exception('Failed to save object data.');
            }

            // Save updated meta data
            if (file_put_contents($metaDataFile, json_encode($metaData)) === false) {
                throw new Exception('Failed to save meta data.');
            }

            // Load existing tileset image
            $tilesetImagePath = $_SERVER['DOCUMENT_ROOT'] . '/assets/img/sheets/gen1.png';
            if (!file_exists($tilesetImagePath)) {
                throw new Exception('Tileset image file not found.');
            }

            $tilesetImage = imagecreatefrompng($tilesetImagePath);
            if (!$tilesetImage) {
                throw new Exception('Failed to create image from tileset.');
            }

            imagesavealpha($tilesetImage, true);
            imagealphablending($tilesetImage, true); // Enable alpha blending

            $tileSize = 16; // Tile size is 16x16
            $tilesPerRow = 150;

            // Calculate required rows
            $currentTileCount = imagesy($tilesetImage) / $tileSize * $tilesPerRow;
            $newTileCount = $initialTileIndex + count($aCoords);
            $requiredRows = ceil($newTileCount / $tilesPerRow);
            $currentRows = imagesy($tilesetImage) / $tileSize;

            // Resize the tileset image if needed
            if ($requiredRows > $currentRows) {
                $newHeight = $requiredRows * $tileSize;
                $resizedTilesetImage = imagecreatetruecolor(imagesx($tilesetImage), $newHeight);
                imagesavealpha($resizedTilesetImage, true);
                $transparent = imagecolorallocatealpha($resizedTilesetImage, 0, 0, 0, 127);
                imagefill($resizedTilesetImage, 0, 0, $transparent);
                imagecopy($resizedTilesetImage, $tilesetImage, 0, 0, 0, 0, imagesx($tilesetImage), imagesy($tilesetImage));
                imagedestroy($tilesetImage);
                $tilesetImage = $resizedTilesetImage;
            }

            // Process each tile based on a and b coordinates
            foreach ($aCoords as $index => $a) {
                $b = $bCoords[$index];
                
                $srcX = $a * $tileSize;
                $srcY = $b * $tileSize;

                // Extract individual tile as an image
                $tileImage = imagecreatetruecolor($tileSize, $tileSize);
                imagesavealpha($tileImage, true);
                $transparent = imagecolorallocatealpha($tileImage, 0, 0, 0, 127);
                imagefill($tileImage, 0, 0, $transparent);

                if (!imagecopy($tileImage, $uploadedImage, 0, 0, $srcX, $srcY, $tileSize, $tileSize)) {
                    throw new Exception("Failed to copy tile from uploaded image at X: {$srcX}, Y: {$srcY}.");
                }

                // Calculate the destination position in the tileset image
                $destX = (($initialTileIndex + $index) % $tilesPerRow) * $tileSize;
                $destY = floor(($initialTileIndex + $index) / $tilesPerRow) * $tileSize;

                // Copy the tile to the tileset image
                if (!imagecopy($tilesetImage, $tileImage, $destX, $destY, 0, 0, $tileSize, $tileSize)) {
                    throw new Exception('Failed to copy tile to tileset image.');
                }

                // Destroy the tile image resource
                imagedestroy($tileImage);
            }

            // Save the updated tileset image
            if (!imagepng($tilesetImage, $tilesetImagePath)) {
                throw new Exception('Failed to save updated tileset image.');
            }

            imagedestroy($tilesetImage);
            imagedestroy($uploadedImage);

            echo json_encode(['success' => true]);
        } catch (Exception $e) {
            echo json_encode(['success' => false, 'message' => $e->getMessage()]);
        }
    } else {
        echo json_encode(['success' => false, 'message' => 'Invalid request method.']);
    }
} else {
    echo json_encode(['success' => false, 'message' => 'Unauthorized.']);
}
?>
