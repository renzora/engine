<?php

header('Content-type: application/json');
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
include $_SERVER['DOCUMENT_ROOT'] . '/helpers/inputCheck.php';

use Firebase\JWT\JWT;
use Firebase\JWT\Key;

if (!$auth) {

    $login_username = clean($_POST['login_username'] ?? '');
    $login_password = clean($_POST['login_password'] ?? '');

    if ($login_username === '' || $login_password === '') {
        $json = array("message" => "error_1");
        echo json_encode($json);
        exit;
    }

    $usersCollection = $db->selectCollection('users');

    $findUser = $usersCollection->findOne([
        '$or' => [
            ['username' => $login_username],
            ['email' => $login_username]
        ]
    ]);

    if (!$findUser) {
        $json = array("message" => "user_not_found");
        echo json_encode($json);
        exit;
    }

    if (password_verify($login_password, $findUser['password'])) { 

        // Create JWT payload
        $payload = [
            'id' => (string)$findUser['_id'],
            'username' => $findUser['username'],
            'iat' => time(), // Issued at
            'exp' => time() + (60 * 60 * 24 * 7) // Expiration (1 week)
        ];

        // Encode JWT
        $jwt = JWT::encode($payload, $_ENV['JWT_KEY'], 'HS256');

        // Set the cookie with the JWT
        setcookie("renaccount", $jwt, time() + (10 * 365 * 24 * 60 * 60), '/');

        // Respond with success
        $json = array("message" => "login_complete", "token" => $jwt);
        echo json_encode($json);
        exit;

    } else {
        $json = array("message" => "incorrect_info");
        echo json_encode($json);
        exit;
    }
}
