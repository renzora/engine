<?php
ini_set('display_errors', 1);
error_reporting(E_ALL);

function hexToRgb($hex) {
    $hex = str_replace("#", "", $hex);
    if(strlen($hex) == 3) {
        $r = hexdec(substr($hex,0,1).substr($hex,0,1));
        $g = hexdec(substr($hex,1,1).substr($hex,1,1));
        $b = hexdec(substr($hex,2,1).substr($hex,2,1));
    } else {
        $r = hexdec(substr($hex,0,2));
        $g = hexdec(substr($hex,2,2));
        $b = hexdec(substr($hex,4,2));
    }
    return array('r' => $r, 'g' => $g, 'b' => $b);
}

function replaceColors($image, $toReplace, $replacements) {
    foreach ($toReplace as $index => $hexColor) {
        $color = hexToRgb($hexColor);
        $targetColor = imagecolorallocate($image, $color['r'], $color['g'], $color['b']);
        $width = imagesx($image);
        $height = imagesy($image);
        for ($x = 0; $x < $width; $x++) {
            for ($y = 0; $y < $height; $y++) {
                if (imagecolorat($image, $x, $y) === $targetColor) {
                    $replacement = hexToRgb($replacements[$index]);
                    $newColor = imagecolorallocate($image, $replacement['r'], $replacement['g'], $replacement['b']);
                    imagesetpixel($image, $x, $y, $newColor);
                }
            }
        }
    }
}

$body = isset($_GET['body']) ? (int)$_GET['body'] : 1;
$skinFilename = ($body === 0) ? './character_head.png' : './character.png';
$skinImage = imagecreatefrompng($skinFilename);
if ($skinImage === false) {
    die('Failed to load skin image');
}

$skinToReplace = ['BF8B78', 'B57972', 'C49D85'];
$hairToReplace = ['AB6736', 'B37B3F', 'CC9659'];

$skinColorPresets = [
    1 => ['FFCBB0', 'F6AE9F', 'FFDBBA'], // Light skin tones
    2 => ['FFB893', 'F69784', 'FFCCA8'], // Medium light skin tones
    3 => ['F0AE80', 'E6976D', 'F5C796'], // Olive skin tones
    4 => ['CDB57A', 'C49C65', 'E2D696'], // Tan skin tones
    5 => ['BF8B78', 'B57972', 'C49D85'], // Medium skin tones
    6 => ['BB845C', 'B77455', 'C29465'], // Brown skin tones
    7 => ['E6B8D7', 'D99ACB', 'F4C8E0'], // Fantasy skin tones (e.g., pink)
    8 => ['BAB8D7', '9FAAD6', 'CCC4EC'], // Another fantasy example (e.g., light blue)
    9 => ['5B402D', '614430', '6F4E37'] // Dark skin tones
];

$hairColorPresets = [
    1 => ['AA6635', 'BC8548', 'D89F59'], // Brown hair
    2 => ['090A0A', '141616', '202424'], // Black hair
    3 => ['2C3E50', '34495E', '394A67'], // Dark blue hair
    4 => ['800020', '930025', 'AB002B'], // Burgundy hair
    5 => ['F44336', 'F65036', 'FD523D'], // Red hair
    6 => ['ECC548', 'F7CE4B', 'FFD44E'], // Blonde hair
    7 => ['CDDC39', 'D9E93C', 'E2F23F'] // Green hair
];

$tone = isset($_GET['tone']) ? (int)$_GET['tone'] : 1;
$hairStyle = isset($_GET['hairstyle']) ? (int)$_GET['hairstyle'] : 1;
$hairColor = isset($_GET['haircolor']) ? (int)$_GET['haircolor'] : 1;
$outfit = isset($_GET['outfit']) ? (int)$_GET['outfit'] : 1;

$hairFilename = "./hair/{$hairStyle}.png";
$hairImage = imagecreatefrompng($hairFilename);
if ($hairImage === false) {
    die('Failed to load hair image');
}

$outfitFilename = './outfit/'.$outfit.'.png';
if ($body !== 0) {
    $outfitImage = imagecreatefrompng($outfitFilename);
    if ($outfitImage === false) {
        die('Failed to load outfit image');
    }
    imagecopy($skinImage, $outfitImage, 0, 0, 0, 0, imagesx($outfitImage), imagesy($outfitImage));
    imagedestroy($outfitImage);
}

replaceColors($skinImage, $skinToReplace, $skinColorPresets[$tone]);

if(isset($hairColorPresets[$hairColor])) {
    replaceColors($hairImage, $hairToReplace, $hairColorPresets[$hairColor]);
} else {
    die('Invalid hair color');
}

imagecopy($skinImage, $hairImage, 0, 0, 0, 0, imagesx($hairImage), imagesy($hairImage));
imagedestroy($hairImage);

$single = isset($_GET['single']);
$sizeToZoom = ['s' => 1, 'm' => 2, 'l' => 3, 'xl' => 4, '2xl' => 5, '3xl' => 6];
$sizeParam = isset($_GET['size']) ? $_GET['size'] : 's';
$zoom = isset($sizeToZoom[$sizeParam]) ? $sizeToZoom[$sizeParam] : 1;

if ($body === 0) {
    // Dimensions for the head section
    $headWidth = 16;
    $headHeight = 17;

    // Create a new image for the head section
    $headImage = imagecreatetruecolor($headWidth * $zoom, $headHeight * $zoom);

    // Set up transparency for the new image
    $transparent = imagecolorallocatealpha($headImage, 0, 0, 0, 127);
    imagefill($headImage, 0, 0, $transparent);
    imagesavealpha($headImage, true);

    // Copy and resize the head section of the original image to the new image
    imagecopyresampled($headImage, $skinImage, 0, 0, 0, 0, $headWidth * $zoom, $headHeight * $zoom, $headWidth, $headHeight);

    // Clean up the original image
    imagedestroy($skinImage);

    // Use the head image for final output
    $skinImage = $headImage;
} else if ($zoom > 1) {
    // For resizing the whole body image when &body!=0 and zoom is applied
    $originalWidth = imagesx($skinImage);
    $originalHeight = imagesy($skinImage);
    $outputWidth = $originalWidth * $zoom;
    $outputHeight = $originalHeight * $zoom;
    $outputImage = imagecreatetruecolor($outputWidth, $outputHeight);
    $transparent = imagecolorallocatealpha($outputImage, 0, 0, 0, 127);
    imagefill($outputImage, 0, 0, $transparent);
    imagesavealpha($outputImage, true);
    imagecopyresampled($outputImage, $skinImage, 0, 0, 0, 0, $outputWidth, $outputHeight, $originalWidth, $originalHeight);
    imagedestroy($skinImage);
    $skinImage = $outputImage;
}

header('Content-Type: image/png');
imagesavealpha($skinImage, true);
imagepng($skinImage);
imagedestroy($skinImage);
?>