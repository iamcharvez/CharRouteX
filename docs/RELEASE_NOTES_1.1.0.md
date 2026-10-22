# CharRouteX 1.1.0

## Network configuration

- Configure one primary and up to 15 additional static IPv4 addresses per adapter.
- Enter networks as CIDR prefix lengths or dotted-decimal subnet masks.
- Configure up to four ordered DNS servers and the interface metric.
- Validate address syntax, usable host ranges, duplicates, subnet overlap, gateway reachability, current local assignments, and live ARP responses before applying.
- Verify the actual Windows address, gateway, DNS, DHCP, and metric state before accepting a protected change.

## Background operation

- Keep CharRouteX running in the Windows notification area after its window closes.
- Open, refresh, pause or resume monitoring, and exit from the tray menu.
- Reopening CharRouteX activates the existing instance instead of starting a duplicate.
- Choose close-to-tray or exit-on-close in Settings.

## Monitoring

- Select traffic refresh rates from 0.5 to 10 seconds.
- Select health-probe intervals from 3 to 60 seconds.
- Pause monitoring without exiting the application.

The application, helper, MSI, and NSIS installer in this local release are unsigned development artifacts.
