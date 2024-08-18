<?php
error_reporting(E_ALL);
ini_set('display_errors', 1);

include $_SERVER['DOCUMENT_ROOT'] . '/config.php';

header('Content-Type: application/json');

if ($auth) {
    // Get the raw POST data
    $jsonData = file_get_contents('php://input');
    $data = json_decode($jsonData, true);

    // Check if json_decode failed
    if (json_last_error() !== JSON_ERROR_NONE) {
        echo json_encode(['success' => false, 'message' => 'Invalid JSON format', 'received_data' => $jsonData]);
        exit;
    }

    // Save objectData, including the name and script data
    $objectDataPath = $_SERVER['DOCUMENT_ROOT'] . '/assets/json/objectData.json';
    $originalPermissionsObjectData = fileperms($objectDataPath);
    chmod($objectDataPath, 0777);

    // Save objectData, including the new polygon format for walkable data
    $objectDataSaved = file_put_contents($objectDataPath, json_encode($data, JSON_UNESCAPED_UNICODE));
    chmod($objectDataPath, $originalPermissionsObjectData);

    if ($objectDataSaved) {
        echo json_encode(['success' => true, 'received_data' => $data]);
    } else {
        echo json_encode(['success' => false, 'message' => 'Failed to save object data', 'received_data' => $data]);
    }
    
} else {
    echo json_encode(['success' => false, 'message' => 'Unauthorized', 'received_data' => null]);
}
