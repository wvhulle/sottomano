# Network

## Network: WireGuard tunnels

Every WireGuard interface on this computer, one row each, with a switch that connects or disconnects the tunnel. A server tunnel counts its connected clients in a badge. Its card shows the listen port, how many clients are connected and, per peer, the endpoint, the time since the last handshake and the data received and sent. Peer statistics need `CAP_NET_ADMIN`; without it the card says so.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/wireguard_tunnels.webp" width="1001" alt="Opening the WireGuard group from the Network menu and hovering a server tunnel">

## Network: Tailscale access

The Tailscale group has a switch for Tailscale itself, this machine's tags, the exit nodes to route through and a switch for reaching the LAN while exiting. The "Reachable from" row reads the Tailscale packet filter: its card lists the machines that may reach this one and on which ports, and the machines allowed to route through it.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/tailnet_access.webp" width="1198" alt="Opening the Tailscale group from the Network menu and hovering who can reach this machine">
