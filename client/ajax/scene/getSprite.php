<?php 
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';

$usersCollection = $db->users;

header('Content-Type: application/json');

$userDocument = $usersCollection->findOne(['id' => $user->id]);

if ($userDocument) {
    echo json_encode([
        'avatar' => $userDocument->avatar
    ]);
} else {
    echo json_encode(['error' => 'not_found']);
}