# Network

<div class="beside">
<div class="beside-text">

WireGuard tunnels with a switch to connect or disconnect them, and who may reach this machine over Tailscale.

</div>

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/tour_network.webp" width="1167" alt="Opening the Network menu, hovering who can reach this machine and an exit node, then a WireGuard tunnel">

</div>

## Network: WireGuard tunnels

Every WireGuard interface on this computer, one row each, with a switch that connects or disconnects the tunnel. A server tunnel counts its connected clients in a badge. Its card shows the listen port, how many clients are connected and, per peer, the endpoint, the time since the last handshake and the data received and sent. Peer statistics need `CAP_NET_ADMIN`; without it the card says so.

## Network: Tailscale access

The Tailscale group has a switch for Tailscale itself, this machine's tags, the exit nodes to route through and a switch for reaching the LAN while exiting. The "Reachable from" row reads the Tailscale packet filter: its card lists the machines that may reach this one and on which ports, and the machines allowed to route through it.
