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