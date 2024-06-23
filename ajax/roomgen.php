<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';

$mode = $_GET['mode'] ?? 'scene';
$zindexFilter = isset($_GET['zindex']) ? (int)$_GET['zindex'] : null;
$bgUrl = isset($_GET['bg']) ? $_GET['bg'] : null;
$sizeParam = $_GET['size'] ?? 'm';

$tileWidth = 16;
$tileHeight = 16;

// Size multipliers for different size parameters
$sizeMultipliers = [
    's' => 0.5, // Small
    'm' => 1,   // Medium
    'l' => 1.5, // Large
    'xl' => 2,  // Extra Large
    '2xl' => 5, // 2x Extra Large
    '3xl' => 10 // 3x Extra Large
];

// Load the tileset image and JSON data for tiles
$tilesetImage = imagecreatefrompng('../assets/img/sprites/items.png');
$jsonTiles = file_get_contents('../assets/json/items.json');
$tiles = json_decode($jsonTiles, true);

function drawTile($destImage, $sourceImage, $tileIndex, $x, $y, $tileWidth, $tileHeight, $originalTileWidth, $originalTileHeight) {
    $tilesPerRow = imagesx($sourceImage) / $originalTileWidth;
    $srcX = ($tileIndex % $tilesPerRow) * $originalTileWidth;
    $srcY = floor($tileIndex / $tilesPerRow) * $originalTileHeight;
    imagecopyresized($destImage, $sourceImage, $x * $tileWidth, $y * $tileHeight, $srcX, $srcY, $tileWidth, $tileHeight, $originalTileWidth, $originalTileHeight);
}

function getBrightness($rgb) {
    $r = ($rgb >> 16) & 0xFF;
    $g = ($rgb >> 8) & 0xFF;
    $b = $rgb & 0xFF;
    return ($r * 0.299) + ($g * 0.587) + ($b * 0.114);
}

function getMedianColor($image) {
    $width = imagesx($image);
    $height = imagesy($image);
    $brightnesses = [];

    for ($x = 0; $x < $width; $x++) {
        for ($y = 0; $y < $height; $y++) {
            $rgb = imagecolorat($image, $x, $y);
            if(($rgb >> 24) == 0xFF) continue; // Skip fully transparent pixels
            $brightnesses[$rgb] = getBrightness($rgb);
        }
    }

    asort($brightnesses);
    $medianIndex = floor(count($brightnesses) / 2);
    reset($brightnesses);
    for($i = 0; $i < $medianIndex; $i++) next($brightnesses);
    $medianRgb = key($brightnesses);

    return $medianRgb;
}

$gridImage = null;

if ($mode === 'scene') {
    // Fetch room data
    $find_room = $db->prepare("SELECT * FROM rooms WHERE id = :id");
    $find_room->execute([':id' => $_GET['id']]);
    $room = $find_room->fetch(PDO::FETCH_OBJ);

    // Use room dimensions for grid size if available
    $gridColumns = $room->numX ?? 30; // Default to 30 if not set
    $gridRows = $room->numY ?? 30;    // Default to 30 if not set
    $gridImage = imagecreatetruecolor($gridColumns * $tileWidth, $gridRows * $tileHeight);
    $transparency = imagecolorallocatealpha($gridImage, 0, 0, 0, 127);
    imagefill($gridImage, 0, 0, $transparency);
    imagesavealpha($gridImage, true);

    // Fetch room items and stitch the scene
    $jsonItems = $room->items;
    $items = json_decode($jsonItems, true)['items'];

    foreach ($items as $item) {
        $id = $item['id'];
        $tileIndices = $tiles[$id]['tiles'];
        $layout = $tiles[$id]['layout'];

        foreach ($item['position'] as $index => $position) {
            if (isset($layout[$index]['zindex'])) {
                $itemZindex = $layout[$index]['zindex'];
            } else {
                $itemZindex = null;
            }

            if ($zindexFilter === null || $itemZindex == $zindexFilter) {
                $tileIndex = $tileIndices[$index];
                drawTile($gridImage, $tilesetImage, $tileIndex, $position['x'], $position['y'], $tileWidth, $tileHeight, $tileWidth, $tileHeight);
            }
        }
    }

    // Check for cropping
    if (isset($_GET['crop'])) {
        $cropWidth = 150;
        $cropHeight = 150;
        $centerX = (imagesx($gridImage) - $cropWidth) / 2;
        $centerY = (imagesy($gridImage) - $cropHeight) / 2;

        $croppedImage = imagecreatetruecolor($cropWidth, $cropHeight);
        imagecopy($croppedImage, $gridImage, 0, 0, $centerX, $centerY, $cropWidth, $cropHeight);
        imagedestroy($gridImage);
        $gridImage = $croppedImage;
    }

    // Apply scaling
    $sizeMultiplier = $sizeMultipliers[$sizeParam] ?? 1;
    $scaledWidth = imagesx($gridImage) * $sizeMultiplier;
    $scaledHeight = imagesy($gridImage) * $sizeMultiplier;

    $scaledImage = imagecreatetruecolor($scaledWidth, $scaledHeight);
    imagecopyresized($scaledImage, $gridImage, 0, 0, 0, 0, $scaledWidth, $scaledHeight, imagesx($gridImage), imagesy($gridImage));
    imagedestroy($gridImage);
    $gridImage = $scaledImage;
} elseif ($mode === 'item' && isset($_GET['item_id'])) {
    // Item mode logic
    $sizeMultiplier = isset($sizeMultipliers[$sizeParam]) ? $sizeMultipliers[$sizeParam] : 1;
    $scaledTileWidth = $tileWidth * $sizeMultiplier;
    $scaledTileHeight = $tileHeight * $sizeMultiplier;

    $itemId = $_GET['item_id'];
    if (isset($tiles[$itemId])) {
        $itemData = $tiles[$itemId];
        $tileIndices = $itemData['tiles'];
        $layout = $itemData['layout'];

        $maxX = max(array_column($layout, 'x'));
        $maxY = max(array_column($layout, 'y'));
        $itemWidth = ($maxX + 1) * $scaledTileWidth;
        $itemHeight = ($maxY + 1) * $scaledTileHeight;

        $gridImage = imagecreatetruecolor($itemWidth, $itemHeight);
        $transparency = imagecolorallocatealpha($gridImage, 0, 0, 0, 127);
        imagefill($gridImage, 0, 0, $transparency);
        imagesavealpha($gridImage, true);

        foreach ($tileIndices as $index => $tileIndex) {
            $posX = $layout[$index]['x'];
            $posY = $layout[$index]['y'];
            drawTile($gridImage, $tilesetImage, $tileIndex, $posX, $posY, $scaledTileWidth, $scaledTileHeight, $tileWidth, $tileHeight);
        }

        $backgroundImage = imagecreatetruecolor(imagesx($gridImage), imagesy($gridImage));
        imagesavealpha($backgroundImage, true);
        $transparency = imagecolorallocatealpha($backgroundImage, 0, 0, 0, 127);
        imagefill($backgroundImage, 0, 0, $transparency);
    
        if ($bgUrl) { // If bg=1, use median color as background
            $medianColor = getMedianColor($gridImage);
            $backgroundColor = imagecolorallocate($backgroundImage, ($medianColor >> 16) & 0xFF, ($medianColor >> 8) & 0xFF, $medianColor & 0xFF);
            imagefill($backgroundImage, 0, 0, $backgroundColor);
        }
        // No else block needed since transparency is set by default
    
        imagecopy($backgroundImage, $gridImage, 0, 0, 0, 0, imagesx($gridImage), imagesy($gridImage));
        $gridImage = $backgroundImage; // Use the backgr
    }
}

// Output the image
if ($gridImage) {
    header('Content-Type: image/png');
    imagepng($gridImage, null, 9);
    imagedestroy($gridImage);
}

imagedestroy($tilesetImage);
?>