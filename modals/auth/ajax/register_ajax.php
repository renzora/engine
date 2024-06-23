<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
include $_SERVER['DOCUMENT_ROOT'] . '/ajax/helpers/inputCheck.php';

use Firebase\JWT\JWT;

if (!$auth) {

  $register_username = clean($_POST['register_username']);
  $register_password = clean($_POST['register_password']);
  $register_email = clean($_POST['register_email']);

  if ($register_username == '' or $register_password == '' or $register_email == '') {
    echo 'error_1';
  } else {

    $usersCollection = $db->selectCollection('users');
    $usernameExists = $usersCollection->findOne(['username' => $register_username]); 

    if ($usernameExists) {
      echo 'username_exists';
    } else if (!preg_match('/^[a-zA-Z0-9._]+$/', $register_username)) {
      echo 'username_invalid';
    } else if (strlen($register_username) > 20) {
      echo 'username_too_long';
    } else if (strlen($register_username) < 3) {
      echo 'username_too_short';
    } else {
      $options = ['cost' => 8];
      $password_hash = password_hash($register_password, PASSWORD_BCRYPT, $options);

      $newUser = [
        'username' => $register_username,
        'password' => $password_hash,
        'email' => $register_email,
        'ugroup' => 1,
        'created' => time()
      ];

      $insertResult = $usersCollection->insertOne($newUser);
      $new_id = $insertResult->getInsertedId();
      $payload = [
        'id' => (string)$new_id,
        'username' => $register_username
      ];

      $jwt = JWT::encode($payload, $_ENV['JWT_KEY'], 'HS256');
      setcookie("renaccount", $jwt, 2147483647, '/'); 

      $notesCollection = $db->selectCollection('notes'); 
      $notesCollection->insertOne([
        'profile_uid' => $new_id,
        'note' => 'Registered Account',
        'author' => 2,
        'time' => time()
      ]);

      echo 'registration_complete';
    }
  }
}
