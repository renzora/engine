<?php
if ($_SERVER['REQUEST_METHOD'] === 'POST') {
    // Directory where the tileset images are stored
    $tilesetDir = $_SERVER['DOCUMENT_ROOT'] . '/assets/img/tiles/';
    
    // Check if the directory exists, if not, create it
    if (!is_dir($tilesetDir)) {
        mkdir($tilesetDir, 0755, true);
    }

    // Check if the files were uploaded
    if (isset($_FILES['tilesetImage']) && isset($_POST['tilesetName'])) {
        $tilesetName = basename($_POST['tilesetName']);
        $targetFile = $tilesetDir . $tilesetName;

        // Check if the file is a valid image
        $check = getimagesize($_FILES['tilesetImage']['tmp_name']);
        if ($check !== false) {
            // Move the uploaded file to the target directory
            if (move_uploaded_file($_FILES['tilesetImage']['tmp_name'], $targetFile)) {
                echo json_encode(['success' => true, 'message' => 'Tileset image uploaded successfully.']);
            } else {
                echo json_encode(['success' => false, 'error' => 'Failed to upload tileset image.']);
            }
        } else {
            echo json_encode(['success' => false, 'error' => 'Uploaded file is not a valid image.']);
        }
    } else {
        echo json_encode(['success' => false, 'error' => 'No files were uploaded.']);
    }
} else {
    echo json_encode(['success' => false, 'error' => 'Invalid request method.']);
}
?>
