<?php
header('Content-type: application/json');
include $_SERVER['DOCUMENT_ROOT'] . '/config/db.php';

if (!$auth) {
    echo json_encode([
        'message' => 'Unauthorized',
        'error' => true
    ]);
    exit();
}

// Retrieve the raw POST data
$input = file_get_contents('php://input');
$data = json_decode($input, true);

$name = $data['name'] ?? 'default server';

try {
    $playerId = (int) $user->id; // Assuming playerId is stored in the JWT payload
    $collection = $db->servers;
    
    $newServer = [
        'name' => $name,
        'created_by' => $playerId,
        'created_at' => time(),
        'public' => 0 // Set the default value to 0 for new servers
    ];
    
    $insertResult = $collection->insertOne($newServer);
    $newServerId = (string) $insertResult->getInsertedId();

    echo json_encode([
        'message' => 'success',
        'server' => [
            'id' => $newServerId,
            'name' => $name,
            'created_by' => $playerId,
            'created_at' => time(),
            'public' => 0 // Include the 'public' field in the response
        ]
    ]);
} catch (Exception $e) {
    echo json_encode([
        'message' => 'Error creating server',
        'error' => $e->getMessage()
    ]);
}
?>
