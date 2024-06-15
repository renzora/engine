<?php
header('Content-Type: application/json');

// Path to the JSON file
$filePath = $_SERVER['DOCUMENT_ROOT'] . '/assets/json/objectData.json';

// Read the incoming JSON data
$input = file_get_contents('php://input');
$data = json_decode($input, true);

if (json_last_error() !== JSON_ERROR_NONE) {
    echo json_encode(['error' => 'Invalid JSON data']);
    exit;
}

// Read the existing data from the file
$existingData = json_decode(file_get_contents($filePath), true);

if (json_last_error() !== JSON_ERROR_NONE) {
    echo json_encode(['error' => 'Failed to read existing data']);
    exit;
}

// Merge the new data with the existing data
$existingData = array_merge($existingData, $data);

// Save the updated data back to the file
if (file_put_contents($filePath, json_encode($existingData))) {
    echo json_encode(['success' => true]);
} else {
    echo json_encode(['error' => 'Failed to save data']);
}
?>
