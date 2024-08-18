<?php
header('Content-type: application/json'); // Ensure JSON output
include $_SERVER['DOCUMENT_ROOT'] . '/config.php'; // Include configuration for DB and user authentication

// Enhanced error handling
try {
    if (!$auth) { // Check if the user is authenticated
        echo json_encode([
            'message' => 'Unauthorized',
            'error' => true
        ]);
        exit();
    }

    $userId = $user->id; // Ensure user ID is an integer

    // Retrieve the raw POST data
    $input = file_get_contents('php://input');
    $data = json_decode($input, true);

    // Log incoming data for debugging
    error_log("Received tab type: " . print_r($data, true));

    $tabType = $data['tabType'] ?? 'public'; // Default to 'public' tab

    $collection = $db->servers; // Connect to the servers collection

    // Filter servers based on the tab type
    $filter = [];
    switch ($tabType) {
        case 'public':
            $filter = ['public' => 1]; // Only show public servers
            break;
        case 'private':
            $filter = ['public' => 0]; // Show all private servers
            break;
        case 'events':
            $filter = ['events' => 1]; // Only show servers with events enabled
            break;
        case 'me':
            $filter = ['created_by' => (int)$userId]; // Convert user ID to integer
            break;
        default:
            throw new Exception("Invalid tab type specified.");
    }

    // Log the filter being used
    error_log("Using filter: " . print_r($filter, true));

    // Sort servers by 'created_at' in descending order
    $servers = $collection->find($filter, ['sort' => ['created_at' => -1]]);

    $serverList = [];
    foreach ($servers as $server) {
        $serverList[] = [
            'id' => (string)$server['_id'],
            'name' => $server['name'],
            'created_at' => $server['created_at'],
            'public' => $server['public'] // Include the public field in the response
        ];
    }

    echo json_encode([
        'message' => 'success',
        'servers' => $serverList
    ]);
} catch (Exception $e) {
    echo json_encode([
        'message' => 'Error fetching servers',
        'error' => $e->getMessage()
    ]);
    
    // Log the error for debugging
    error_log("Error fetching servers: " . $e->getMessage());
}
?>
