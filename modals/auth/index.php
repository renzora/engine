<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';

if(!$auth) {
?>
<div data-window='auth_window' class='window login_window' style='width: 400px;background: #1d335b;'>

  <!-- Content -->
  <div data-part='handle' class='window_title' style='background-image: radial-gradient(#384c6f 1px, transparent 0) !important;'>
    <div data-part='title' class='title_bg' style='background: #1d335b; color: #ede8d6;'>Renzora Server</div>
  </div>

  <div class='clearfix'></div>

  <div class='window_body'>

    <!-- Tabs -->
    <div class="flex space-x-2 m-3 mb-0">
      <button id="loginTab" class="py-2 px-4 text-lg text-white font-semibold rounded focus:outline-none">Login</button>
      <button id="registerTab" class="py-2 px-4 text-lg text-white font-semibold rounded focus:outline-none">Register</button>
      <button id="forgotTab" class="py-2 px-4 text-lg text-white font-semibold rounded focus:outline-none">Forgot</button>
    </div>
    <div class='p-3' style='font-size: 13px;'>

      <!-- Login Form -->
      <div id="loginContent">
        <input type="text" id='login_username' class='light_input shadow appearance-none text-2xl border rounded w-full p-3 mb-2 text-gray-700 leading-tight focus:outline-none focus:shadow-outline' placeholder="Username or Email" required="required" autocomplete="off" />
        <input type="password" id='login_password' class='light_input shadow appearance-none text-2xl border rounded w-full p-3 text-gray-700 leading-tight focus:outline-none focus:shadow-outline' placeholder="Password" required="required" autocomplete="off" />
        <div class='clearfix'></div>
        <button id='login_connect' onclick="auth_window.login()" class="green_button text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md" style="font-size: 16px;"><i class="fas fa-lock-open"></i> Connect to Server</button>
      </div>

      <!-- Register Form -->
      <div id="registerContent" class="hidden">
      <input
        type="text"
        id="register_username" class='light_input shadow appearance-none border rounded text-2xl w-full p-3 text-gray-700 leading-tight focus:outline-none focus:shadow-outline mb-2'
        placeholder="Username"
        required="required"
        autocomplete="off"
      />
      <div class="clearfix"></div>
      <input
        type="password"
        id="register_password" class='light_input shadow appearance-none border rounded text-2xl w-full p-3 text-gray-700 leading-tight focus:outline-none focus:shadow-outline mb-2'
        placeholder="Choose a Password"
        required="required"
        autocomplete="off"
      />
      <div class="clearfix"></div>
      <input
        type="email"
        id="register_email" class='light_input shadow appearance-none border rounded text-2xl w-full p-3 text-gray-700 leading-tight focus:outline-none focus:shadow-outline'
        placeholder="Email Address"
        required="required"
        autocomplete="off"
      />
      <div class="clearfix"></div>

      <button id='register_connect' onclick="auth_window.register();" class="green_button text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md">Create Character</button>

      </div>

      <!-- Forgot Password Form -->
      <div id="forgotContent" class="hidden">
        <!-- Forgot password form fields go here -->
      </div>

    </div>
  </div>

  <style>
    /* Add your gradient animation styles here */
  </style>

  <script>
    var auth_window = {
      login: function() {
        var login_username = document.getElementById('login_username').value;
        var login_password = document.getElementById('login_password').value;

        if (login_username == '' || login_password == '') {
          modal.load('auth/error.php?code=1', 'auth_error_window');
        } else {

          ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'modals/auth/ajax/login_ajax.php',
            data: 'login_username=' + login_username + '&login_password=' + login_password,
            success: function(data) {
              console.log(data);
              if(data.message == 'login_complete') {
                modal.load('ui');
                ui.notif("You are now signed in as " + login_username, 'bottom-center');
                modal.close("auth_window");
              } else {
                modal.load('auth/error.php?code=' + data.message, 'auth_error_window');
              }
            },
            error: function(data) {
              console.log(data);
            }
          });
        }
      },
      register: function() {
        var register_username = document.getElementById('register_username').value;
        var register_password = document.getElementById('register_password').value;
        var register_email = document.getElementById('register_email').value;

          if(register_username == '' || register_password == '' || register_email == '') {
            modal.load('auth/error.php?code=1', 'auth_error_window');
          } else {
            ui.ajax({
              method: 'POST',
              url: 'modals/auth/ajax/register_ajax.php',
              data: 'register_username=' + register_username + '&register_password=' + register_password + '&register_email=' + register_email,
              success: function(data) {
                console.log(data);
                if(data == 'registration_complete') {
                  ui.notif("Welcome to Renzora, " + register_username + "!");
                  modal.close("auth_window");
                  modal.load('ui');
                } else {
                  modal.load('auth/error.php?code=' + data.message, 'auth_error_window');
                }
            }
          });
        }
        },
      unmount: function() {
      },
      // Function to toggle content based on tab clicked
      toggleContent: function(tabId) {
        const tabs = ["loginTab", "registerTab", "forgotTab"];
        tabs.forEach((tab) => {
          document.getElementById(tab).classList.remove("bg-green-600");
          document.getElementById(tab).classList.add("bg-none");
        });
        document.getElementById(tabId).classList.remove("bg-none");
        document.getElementById(tabId).classList.add("bg-green-600");

        // Hide/show content based on the selected tab
        if (tabId === "loginTab") {
          document.getElementById("loginContent").style.display = "block";
          document.getElementById("registerContent").style.display = "none";
          document.getElementById("forgotContent").style.display = "none";
        } else if (tabId === "registerTab") {
          document.getElementById("loginContent").style.display = "none";
          document.getElementById("registerContent").style.display = "block";
          document.getElementById("forgotContent").style.display = "none";
        } else if (tabId === "forgotTab") {
          document.getElementById("loginContent").style.display = "none";
          document.getElementById("registerContent").style.display = "none";
          document.getElementById("forgotContent").style.display = "block";
        }
      },
      // Initialize with the Login tab active
      initTabs: function() {
        auth_window.toggleContent("loginTab");

        // Event listeners for tab clicks
        document.getElementById("loginTab").addEventListener("click", function() {
          auth_window.toggleContent("loginTab");
        });
        document.getElementById("registerTab").addEventListener("click", function() {
          auth_window.toggleContent("registerTab");
        });
        document.getElementById("forgotTab").addEventListener("click", function() {
          auth_window.toggleContent("forgotTab");
        });
      }
    };
    
    auth_window.initTabs(); // Initialize tabs
  </script>

</div>

<?php 
}
?>