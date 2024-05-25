<?php
header('Content-Type: application/json'); // Ensure the response is JSON

// Read the input data
$data = file_get_contents("php://input");

// Decode the JSON data
$decodedData = json_decode($data, true);

// Check for JSON errors
if (json_last_error() !== JSON_ERROR_NONE) {
    // Log the error
    error_log("JSON decode error: " . json_last_error_msg());
    http_response_code(400);
    echo json_encode([
        'message' => 'Invalid JSON data',
        'error' => json_last_error_msg()
    ]);
    exit;
}

// Log the received data for debugging
error_log("Received data: " . $data);

// Define the file path
$filePath = $_SERVER['DOCUMENT_ROOT'] . '/assets/json/roomData.json';

// Log the file path for debugging
error_log("File path: " . $filePath);

// Save the JSON data in compact form
$compactJsonData = json_encode($decodedData);

// Attempt to save the file
if (file_put_contents($filePath, $compactJsonData)) {
    echo json_encode([
        'message' => 'Room data saved successfully'
    ]);
} else {
    $errorMessage = error_get_last();
    error_log("File write error: " . json_encode($errorMessage));
    http_response_code(500);
    echo json_encode([
        'message' => 'Error saving room data',
        'error' => 'Unable to write to file'
    ]);
}
?>
