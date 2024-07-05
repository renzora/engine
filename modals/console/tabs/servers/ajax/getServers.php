<?php
header('Content-type: application/json');
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';

if (!$auth) {
    echo json_encode([
        'message' => 'Unauthorized',
        'error' => true
    ]);
    exit();
}

$userId = $user->id;

try {
    $collection = $db->servers;
    $servers = $collection->find(['created_by' => (int)$userId]);

    $serverList = [];
    foreach ($servers as $server) {
        $serverList[] = [
            'id' => (string) $server['_id'],
            'name' => $server['name'],
            'created_at' => $server['created_at']
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
}
?>
