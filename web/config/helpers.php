<?php
function isPublicIP($ip) {
    // Validate the IP address (IPv4 or IPv6)
    if (!filter_var($ip, FILTER_VALIDATE_IP, FILTER_FLAG_IPV4 | FILTER_FLAG_IPV6)) {
        return false; // Not a valid IP address
    }

    // Reject private and reserved IP ranges for IPv4
    if (filter_var($ip, FILTER_VALIDATE_IP, FILTER_FLAG_IPV4) && (
        $ip === '127.0.0.1' || // localhost
        $ip === '::1' || // IPv6 localhost
        ip2long($ip) >= ip2long('10.0.0.0') && ip2long($ip) <= ip2long('10.255.255.255') || // 10.0.0.0/8
        ip2long($ip) >= ip2long('172.16.0.0') && ip2long($ip) <= ip2long('172.31.255.255') || // 172.16/12
        ip2long($ip) >= ip2long('192.168.0.0') && ip2long($ip) <= ip2long('192.168.255.255') || // 192.168/16
        ip2long($ip) >= ip2long('169.254.0.0') && ip2long($ip) <= ip2long('169.254.255.255') // 169.254/16 (Link-local)
    )) {
        return false; // IP is private or reserved
    }

    return true; // IP is public
}

function generateServerKey($length = 32) { // 256 bits = 32 bytes
    return bin2hex(random_bytes($length));
}

function generateServerCode($length = 8) {
    $characters = '0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ';
    $charactersLength = strlen($characters);
    $randomCode = '';
    for ($i = 0; $i < $length; $i++) {
        $randomCode .= $characters[rand(0, $charactersLength - 1)];
    }
    return $randomCode;
}

function clean($input) {
    $sanitized = htmlspecialchars(strip_tags($input), ENT_QUOTES, 'UTF-8');
    return $sanitized;
}