<?php 
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';


$roomsCollection = $db->rooms;
$marketCollection = $db->market;
$usersCollection = $db->users;

$query = isset($_GET['query']) ? $_GET['query'] : '';

try {
    $groupedResults = [];
    $regex = new MongoDB\BSON\Regex($query, 'i');

    // Rooms
    $rooms = $roomsCollection->find(['$or' => [['name' => $regex], ['info' => $regex]]], ['limit' => 5])->toArray();
    if ($rooms) {
        $groupedResults['Rooms'] = array_map(function ($doc) {
            return (object)[
                'result_type' => 'Rooms',
                'title' => $doc->name,
                'description' => $doc->info ?? '',
                'extra_info' => $doc->id
            ];
        }, $rooms);
    }

    // Market Items
    $marketItems = $marketCollection->find(['$or' => [['item_name' => $regex], ['item_info' => $regex]]], ['limit' => 5])->toArray();
    if ($marketItems) {
        $groupedResults['Market Items'] = array_map(function ($doc) {
            return (object)[
                'result_type' => 'Market Items',
                'title' => $doc->item_name,
                'description' => $doc->item_info ?? '',
                'extra_info' => $doc->item_id
            ];
        }, $marketItems);
    }

    // Users
    $users = $usersCollection->find(['username' => $regex], ['limit' => 5])->toArray();
    if ($users) {
        $groupedResults['Users'] = array_map(function ($doc) {
            return (object)[
                'result_type' => 'Users',
                'title' => $doc->username,
                'description' => '',
                'extra_info' => $doc->avatar ?? ''
            ];
        }, $users);
    }

    // Display results, similar HTML rendering as your original code
    if (!empty($groupedResults)) {
        // Your HTML rendering logic here, similar to the original
    } else {
        echo "<p class='text-white text-center'>No results found for " . htmlspecialchars($query) . "</p>";
    }

} catch (Exception $e) {
    echo "Database error: " . $e->getMessage();
}
?>