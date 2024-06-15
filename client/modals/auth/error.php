<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';

if (!$auth) {
?>

  <div data-window='auth_error_window' class='window window_bg'>

    <div data-part='handle' class='window_title window_border'>
      <div data-part='title' class='title_bg window_border'>ERROR SIGNING IN</div>
    </div>

    <div class='clearfix'></div>

    <div class='window_body'>
      <div class='p-3 text-center text-white' style='font-size: 13px;'>

        <?php
        if ($_GET['code'] == 1) {
          echo ' Please fill in the whole form';
        } else if ($_GET['code'] == 'user_not_found') {
          echo 'User not found';
        } else if ($_GET['code'] == 'incorrect_info') {
          echo 'Username or password incorrect';
        }
        ?>

        <div class='clearfix mb-3'></div>
        <button type="button" class="bg-blue-500 hover:bg-blue-600 text-white font-bold py-2 px-4 rounded mx-auto block login_error_window_close" data-close>Okay</button>

      </div>
    </div>

  </div>

<?php
}
?>
