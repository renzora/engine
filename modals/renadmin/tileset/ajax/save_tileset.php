<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';

ini_set('display_errors', 1);
ini_set('display_startup_errors', 1);
error_reporting(E_ALL);

header('Content-Type: application/json');

// Define the log file path
$logFile = $_SERVER['DOCUMENT_ROOT'] . '/modals/renadmin/tileset/ajax/log.txt';

if ($auth) {
    if ($_SERVER['REQUEST_METHOD'] == 'POST') {
        try {
            // Log the received POST request
            $timestamp = date("Y-m-d H:i:s");
            file_put_contents($logFile, "[{$timestamp}] - Received POST request.\n", FILE_APPEND);

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

            // Log the base64 image data
            file_put_contents($logFile, "[{$timestamp}] - Base64 Image Data: " . $imageDataBase64 . "\n", FILE_APPEND);

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

            // Temporarily set permissions to 777
            chmod($objectDataFile, 0777);

            $objectData = json_decode(file_get_contents($objectDataFile), true);
            if (json_last_error() !== JSON_ERROR_NONE) {
                throw new Exception('Failed to decode object data JSON: ' . json_last_error_msg());
            }

            // Log the object data load success
            file_put_contents($logFile, "[{$timestamp}] - Object data loaded successfully.\n", FILE_APPEND);

            // Load the meta tile count
            $metaDataFile = $_SERVER['DOCUMENT_ROOT'] . '/assets/json/meta.json';
            if (!file_exists($metaDataFile)) {
                throw new Exception('Meta data file not found.');
            }

            chmod($metaDataFile, 0777);

            $metaData = json_decode(file_get_contents($metaDataFile), true);
            if (json_last_error() !== JSON_ERROR_NONE) {
                throw new Exception('Failed to decode meta data JSON: ' . json_last_error_msg());
            }

            // Generate a unique ID for the new object
            $uniqueId = uniqid();

            // Calculate the initial index for the new tiles
            $initialTileIndex = $metaData['tile_count'];
            file_put_contents($logFile, "[{$timestamp}] - Initial tile count: {$initialTileIndex}\n", FILE_APPEND);

            // Populate the 'i' array with the correct indices
            $newObject['i'] = [];
            foreach ($aCoords as $index => $a) {
                $newObject['i'][] = $initialTileIndex + $index;
            }

            // Log the updated newObject with populated indices
            file_put_contents($logFile, "[{$timestamp}] - Updated newObject: " . json_encode($newObject) . "\n", FILE_APPEND);

            // Add the new object to the items array with the unique ID as the key
            $objectData[$uniqueId] = [$newObject];

            // Update the tile count in the meta section
            $metaData['tile_count'] += count($aCoords);
            file_put_contents($logFile, "[{$timestamp}] - New tile count: {$metaData['tile_count']}\n", FILE_APPEND);

            // Save updated object data without JSON_PRETTY_PRINT to avoid whitespace
            if (file_put_contents($objectDataFile, json_encode($objectData)) === false) {
                throw new Exception('Failed to save object data.');
            }

            // Save updated meta data
            if (file_put_contents($metaDataFile, json_encode($metaData)) === false) {
                throw new Exception('Failed to save meta data.');
            }

            // Set the file permissions back to read-only
            chmod($objectDataFile, 0444);
            chmod($metaDataFile, 0444);

            // Log the object data and meta data save success
            file_put_contents($logFile, "[{$timestamp}] - Object data and meta data saved successfully.\n", FILE_APPEND);

            // Load existing tileset image
            $tilesetImagePath = $_SERVER['DOCUMENT_ROOT'] . '/assets/img/tiles/gen1.png';
            if (!file_exists($tilesetImagePath)) {
                throw new Exception('Tileset image file not found.');
            }

            // Temporarily set permissions to 777
            chmod($tilesetImagePath, 0777);

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

                // Convert the tile image to base64
                ob_start();
                imagepng($tileImage);
                $tileImageData = ob_get_contents();
                ob_end_clean();
                $tileImageBase64 = 'data:image/png;base64,' . base64_encode($tileImageData);

                // Log the base64 image data of each tile with x and y positions
                file_put_contents($logFile, "[{$timestamp}] - Tile Index {$index}, SrcX: {$srcX}, SrcY: {$srcY} Base64 Image Data: " . $tileImageBase64 . "\n", FILE_APPEND);

                // Calculate the destination position in the tileset image
                $destX = (($initialTileIndex + $index) % $tilesPerRow) * $tileSize;
                $destY = floor(($initialTileIndex + $index) / $tilesPerRow) * $tileSize;

                // Log the destination position
                file_put_contents($logFile, "[{$timestamp}] - Destination Position - Tile Index {$index}, X: {$destX}, Y: {$destY}\n", FILE_APPEND);

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

            // Convert the saved tileset image back to base64 for logging
            ob_start();
            imagepng($tilesetImage);
            $savedImageData = ob_get_contents();
            ob_end_clean();
            $savedImageBase64 = 'data:image/png;base64,' . base64_encode($savedImageData);

            // Log the base64 image data of the saved image
            file_put_contents($logFile, "[{$timestamp}] - Saved Base64 Image Data: " . $savedImageBase64 . "\n", FILE_APPEND);

            // Set the file permissions back to read-only
            chmod($tilesetImagePath, 0444);

            imagedestroy($tilesetImage);
            imagedestroy($uploadedImage);

            file_put_contents($logFile, "[{$timestamp}] - Tileset updated successfully.\n", FILE_APPEND);

            echo json_encode(['success' => true]);
        } catch (Exception $e) {
            // Add detailed error information for meta data save failure
            file_put_contents($logFile, "[{$timestamp}] - Error: " . $e->getMessage() . "\n", FILE_APPEND);
            if ($e->getMessage() == 'Failed to save meta data.') {
                file_put_contents($logFile, "[{$timestamp}] - Meta data: " . json_encode($metaData) . "\n", FILE_APPEND);
            }
            echo json_encode(['success' => false, 'message' => $e->getMessage()]);
        }
    } else {
        echo json_encode(['success' => false, 'message' => 'Invalid request method.']);
    }
} else {
    echo json_encode(['success' => false, 'message' => 'Unauthorized.']);
}
?>
