<?php 
header('Content-Type: application/json');
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';

if($auth) {
    $storymodeChaptersCollection = $db->storymodeChapters;
    $storymodeProgressCollection = $db->storymodeProgress;

    $id = $_GET['id'];
    $story = $storymodeChaptersCollection->findOne(['id' => $id]);

    if(!$story) {
        echo json_encode(['error' => 'not_found']);
    } else {
        $progress = $storymodeProgressCollection->findOne(['chapter_id' => $story->id, 'uid' => $user->id]);

        if(!$progress) {
            $storymodeProgressCollection->insertOne([
                'uid' => $user->id, 
                'chapter_id' => $story->id, 
                'progress' => 'objective1'
            ]);
            $objective = 'objective1';
        } else {
            $objective = $progress->progress;
        }

        echo json_encode([
            "status" => "success",
            "id" => $story->id,
            "storyData" => $story->config,
            "progress" => $objective
        ]);
    }
}
?>