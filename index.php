<!DOCTYPE html>
<html lang="en" dir="ltr">

<head>
  <meta charset="utf-8">
  <title>Renzora</title>
  <link rel="stylesheet" href="assets/css/output.css">
  <script src='assets/js/engine/network.js'></script>
  <script src='assets/js/engine/assets.js'></script>
  <script src='assets/js/engine/effects.js'></script>
  <script src='assets/js/engine/game.js'></script>
  <script src='assets/js/engine/sprite.js'></script>
  <script src='assets/js/engine/astar.js'></script>
  <script src='assets/js/engine/input.js'></script>
  <script src='assets/js/engine/ui.js'></script>
  <script src='assets/js/engine/modal.js'></script>
  <script src='assets/js/engine/weather.js'></script>
  <script src='assets/js/engine/audio.js'></script>
  <script src='assets/js/engine/actions.js'></script>
  <script src='assets/js/engine/editor.js'></script>
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">

</head>

<div id="loadingBarContainer" class="fixed top-5 left-1/2 transform -translate-x-1/2 w-60 p-2 hidden z-50 bg-black bg-opacity-75 rounded">
  <div id="loadingBarWrapper" class="w-full">
    <div id="loadingBar" class="h-5 w-0 bg-green-500 rounded"></div>
  </div>
  <div id="loadingPercentage" class="text-center text-white text-xs mt-1 w-full"></div>
</div>

<body class="flex justify-center items-center"></body>

</html>