<?php
function clean($input) {
    $sanitized = htmlspecialchars(strip_tags($input), ENT_QUOTES, 'UTF-8');
    return $sanitized;
}
?>