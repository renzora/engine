<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';

if($auth) {
  if(isset($_COOKIE['renaccount'])) {
    unset($_COOKIE['renaccount']);
    setcookie('renaccount', '', time() - 3600, '/');
  }
}
