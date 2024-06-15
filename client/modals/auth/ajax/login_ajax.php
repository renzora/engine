<?php

header('Content-type: application/json');
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
include $_SERVER['DOCUMENT_ROOT'] . '/ajax/helpers/inputCheck.php';

use Firebase\JWT\JWT;

if (!$auth) {

  $login_username = clean($_POST['login_username']);
  $login_password = clean($_POST['login_password']);

  if ($login_username == '' or $login_password == '') {
    $json = array("message" => "error_1");
    echo json_encode($json);
  } else {

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
    } else {
      if(password_verify($login_password, $findUser['password'])) { 
    
        $payload = [
          'id' => (string)$findUser['_id'],
          'username' => $findUser['username']
        ];

        $jwt = JWT::encode($payload, $_ENV['JWT_KEY'], 'HS256');
        setcookie("renaccount", $jwt, 2147483647, '/');

        $json = array("message" => "login_complete");
        echo json_encode($json);

      } else {
        $json = array("message" => "incorrect_info");
        echo json_encode($json);
      }
    }
  }
}
?>