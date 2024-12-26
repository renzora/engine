<?php
header('Content-Type: application/json'); // Ensure the response is JSON
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';

if (!$auth) {
    echo json_encode([
        'message' => 'Unauthorized',
        'error' => true
    ]);
    exit();
}

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
    exit();
}

// Log the received data for debugging
error_log("Received data: " . $data);

try {
    // Check if sceneid and roomData are set in the received data
    if (!isset($decodedData['sceneid']) || !isset($decodedData['roomData'])) {
        throw new Exception('sceneid or roomData not provided in the request data');
    }

    // Get the scene ID from the received data
    $sceneId = new MongoDB\BSON\ObjectId($decodedData['sceneid']); // Ensure you are passing the correct scene ID

    // Prepare the roomData for updating
    $roomData = $decodedData['roomData'];

    // Log the sceneId and roomData for debugging
    error_log("Scene ID: " . $sceneId);
    error_log("Room Data: " . json_encode($roomData));

    // Update the scene document with the new roomData
    $updateResult = $db->scenes->updateOne(
        ['_id' => $sceneId],
        ['$set' => ['roomData' => $roomData]]
    );

    // Log the update result for debugging
    error_log("Update Result: " . json_encode($updateResult));

    if ($updateResult->getMatchedCount() > 0) {
        http_response_code(200);
        echo json_encode([
            'message' => 'Room data saved successfully'
        ]);
    } else {
        http_response_code(404);
        echo json_encode([
            'message' => 'Scene not found',
            'error' => 'No matching scene found for the provided ID'
        ]);
    }
} catch (MongoDB\Driver\Exception\Exception $e) {
    error_log("MongoDB Exception: " . $e->getMessage());
    http_response_code(500);
    echo json_encode([
        'message' => 'Error saving room data',
        'error' => $e->getMessage()
    ]);
} catch (Exception $e) {
    error_log("General Exception: " . $e->getMessage());
    http_response_code(500);
    echo json_encode([
        'message' => 'Error saving room data',
        'error' => $e->getMessage()
    ]);
}
?>