<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';

if($auth) {
?>

  <div data-window='signout_window' class='window window_bg'>

    <div data-part='handle' class='window_title window_border'>
      <div class='float-right'>
        <i data-part='close' class="fa fa-window-close fa-2x pb-3 ps-2 window_border" data-close></i>
      </div>
      <div data-part='title' class='window_bg window_border'>Sign Out confirmation</div>
    </div>

    <div class='clearfix'></div>

    <div class='window_body'>
      <div class='p-3 text-center text-white'>
        Are you sure you want to sign out?
        <div class='clearfix'></div>

        <button class="bg-green-500 hover:bg-green-600 text-white font-bold py-2 px-4 rounded-full w-full mt-3" onclick='signout_window.confirm();'>
          Yes
        </button>

        <button class="bg-blue-500 hover:bg-blue-600 text-white font-bold py-2 px-4 rounded-full w-full mt-3" onclick='signout_window.cancel();'>
          Cancel
        </button>

      </div>
    </div>

    <script>
      var signout_window = {
        signout: function() {

          ui.ajax({
              method: 'POST',
              url: 'modals/auth/ajax/signout_ajax.php',
              success: function(data) {
                ui.notif("You are now signed out. Please come back again soon :)", 'bottom-center');
                modal.load('ui');
                modal.load('auth');
                modal.close('signout_window');
              }
            });

        },
        confirm: function() {
          signout_window.signout();
          modal.closeAll();
        },
        cancel: function() {
          modal.close('signout_window');
          modal.load('settings');
        }
      }
    </script>

  </div>

<?php
}
?>
