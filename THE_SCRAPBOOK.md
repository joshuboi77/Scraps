# Scraps Language Manual

A concise, accurate reference to the Scraps language as implemented in this VM.

- Paradigm: expression‑first with newline‑terminated statements; control flow is statement‑based.
- Values: `Int`, `Float`, `Bool`, `Str` (Unicode), `Box` (list), `Function`, `None`.
- Purity: user function calls are pure by default (no caller env mutation). Use `rewire` + `result(...)` for intentional side effects.
- Identifiers: Unicode supported.

## Table of Contents
- [Syntax Overview](#syntax-overview)
- [Literals and Types](#literals-and-types)
- [Operators and Precedence](#operators-and-precedence)
- [Arrows and Verbs](#arrows-and-verbs)
  - [PACK](#pack)
  - [PLACE](#place)
  - [UNPACK](#unpack)
  - [PICK](#pick)
  - [STEP](#step)
  - [COUNT](#count)
  - [FISSION](#fission)
  - [FUSION](#fusion)
- [I/O](#io)
- [Network I/O (TCP)](#network-io-tcp)
- [Math Built-ins](#math-built-ins)
- [Control Flow](#control-flow)
  - [IF / ELSE](#if--else)
  - [WHILE](#while)
  - [TEST](#test)
- [Functions](#functions)
  - [Definition and Calls](#definition-and-calls)
  - [Purity and RESULT](#purity-and-result)
- [Rewire](#rewire)
  - [Mutable Operations (variable)](#mutable-operations-variable)
  - [Dynamic Symbol Binding (string)](#dynamic-symbol-binding-string)
- [String as Code](#string-as-code)
- [Modules](#modules)
- [Semantics & Errors](#semantics--errors)
- [Complete Examples](#complete-examples)
- [Quick Reference](#quick-reference)

---

## Syntax Overview

- Statements end at newline; blank lines allowed.
- Comments start with `#`.
- Grouping with `(` `)` for expressions and `{` `}` for blocks.
- Control flow keywords (`IF`, `WHILE`) are statements, not expressions.
- Arrows:
  - Read‑from: `<-` (read from right‑hand value)
  - Write‑to: `->` (write to right‑hand value)
- Keywords are case‑insensitive (e.g., `PACK` or `pack`).

## Literals and Types

- Integers: `0`, `42`, `-3`
- Floats: `3.14`, `0.0`
- Booleans: `TRUE`, `FALSE`
- Strings: `"hello"` (Unicode)
- Boxes: created with `box()` and populated with `pack`
- Functions: created via `fn(use(...)) { ... } -> name`

## Operators and Precedence

- Arithmetic: `+ - * /`
- Comparison: `< > <= >= == !=`
- Logical: `>|` (OR), `|<` (AND), `!` (NOT)
- Precedence (high → low): `* /` > `+ -` > comparisons > `>| |<`
- Grouping: `(expr)`

## Arrows and Verbs

Arrows bind verbs to their sources/targets.

### PACK
Append values to a box.

```scraps
x = box()
pack(1, 2, 3) -> x     # x = [1, 2, 3]
```

### PLACE
Update one or more indices in a box. Resizes if needed.

```scraps
place(1: 9, 3: 7) -> x  # x = [1, 9, 3, 7] (index 3 created)
```

### UNPACK
Extract an element or slice (end‑exclusive) from a box or string.

```scraps
print unpack(0) <- x        # element at 0
print unpack(1, 3) <- x     # slice [1..3), e.g. [2, 3]
```

### PICK
Multi‑select within one layer (box or string). For strings, returns concatenated characters.

```scraps
print pick(0, 2) <- x       # [x[0], x[2]]
```

### STEP
Scoped head selector; often combined with `PICK`.

```scraps
print :.(0) <- y            # same as unpack(0) <- y
print :.(0) pick(1, 2) <- y # pick within y[0]
```

### COUNT
Count length of boxes/strings or parameters of functions.

```scraps
print count(x)              # 3
print count(1, 3) <- x      # 2 (slice length)
```

### FISSION
Split a string by delimiter into a box of strings. Empty delimiter splits into characters.

```scraps
parts = fission(" ") <- "Hello World"  # ["Hello","World"]
chars = fission("") <- "hi!"          # ["h","i","!"]
```

### FUSION
Join a box of strings with a delimiter. (Strict: all elements must be strings.)

```scraps
print fusion("-") -> parts  # Hello-World
print fusion("") -> chars   # hi!
```

## I/O

File I/O with strings.

```scraps
WRITE("Hello") -> "tmp.txt"
content = READ <- "tmp.txt"
print content                 # Hello
```

## Network I/O (TCP)

Synchronous TCP sockets exposed as built-in functions. Works at top level and inside functions.

- `tcp_listen(port) -> listener`: Bind to `127.0.0.1:port` and return a listener handle.
- `tcp_accept(listener) -> connection`: Accept a single inbound connection and return a connection handle.
- `tcp_connect(host, port) -> connection`: Connect to a TCP server and return a connection handle.
- `tcp_send(connection, data) -> TRUE`: Send a string payload over a connection.
- `tcp_receive(connection, max_bytes) -> string`: Read up to `max_bytes` and return a string (UTF‑8).
- `tcp_close(connection_or_listener) -> TRUE`: Close a connection or a listener.

Notes:
- Blocking behavior: `tcp_accept` blocks until a client connects. For single-script demos, connect first, then accept.
- Timeouts: Connections have short read/write timeouts; `tcp_receive` returns an empty string on timeout.
- Encoding: `tcp_receive` expects UTF‑8. Invalid UTF‑8 returns an error.
- Scope: Built-ins (not opcodes). They compose like normal function calls and can be used inside `fn{}` bodies.

Examples

1) Local echo roundtrip (single script)

```scraps
port = 9091
tcp_listen(port) -> l
tcp_connect("127.0.0.1", port) -> c_client
tcp_accept(l) -> c_server

tcp_send(c_client, "ping") -> _
tcp_receive(c_server, 1024) -> srv_data
print srv_data              # ping

tcp_send(c_server, "pong") -> _
tcp_receive(c_client, 1024) -> cli_data
print cli_data              # pong

tcp_close(c_client) -> _
tcp_close(c_server) -> _
tcp_close(l) -> _
```

2) Simple server loop (single connection)

```scraps
tcp_listen(9092) -> l
print "waiting..."
tcp_accept(l) -> c
print "connected"

i = 0
WHILE (i < 3) {
  tcp_receive(c, 1024) -> msg
  print msg
  tcp_send(c, fusion("") -> ["ok:", msg]) -> _
  i = i + 1
}

tcp_close(c) -> _
tcp_close(l) -> _
```

3) Client function usage

```scraps
fn(use(host, port)) {
  tcp_connect(host, port) -> c
  tcp_send(c, "hello") -> _
  tcp_receive(c, 1024) -> reply
  tcp_close(c) -> _
  reply
} -> ping_once

print ping_once("127.0.0.1", 9092)
```

### WebSocket (Phase 3)

Blocking WebSocket client built-ins (basic echo-style usage).

- `ws_connect(url) -> ws`: Connect to `ws://` or `wss://` URL.
- `ws_send(ws, string) -> TRUE`: Send a text frame.
- `ws_receive(ws) -> string`: Receive next text/binary frame as string (binary is UTF‑8 decoded). May return empty string for ping/pong/timeout.
- `ws_try_receive(ws) -> string`: Non‑blocking poll; returns empty string if no frame available.
- `ws_close(ws) -> TRUE`: Close the socket.

Example (echo server):

```scraps
url = "wss://echo.websocket.events"
ws = ws_connect(url)
_ = ws_send(ws, "hello")
print ws_receive(ws)   # prints greeting or echo
_ = ws_close(ws)
```

Non‑blocking wait helper (poll with `ws_try_receive`):

```scraps
# Returns first non-empty message or empty string after max polls
fn(use(ws)) {
  i = 0
  out = ""
  WHILE (i < 100) {
    r = ws_try_receive(ws)
    IF (count(r) > 0) {
      out = r
      i = 100
    } ELSE {
      i = i + 1
    }
  }
  out
} -> ws_wait_one

url = "wss://echo.websocket.events"
ws = ws_connect(url)
_ = ws_send(ws, "ping")
print ws_wait_one(ws)
_ = ws_close(ws)
```

### HTTP (Phase 2)

Blocking HTTP client via built-ins. These follow conventional web API shapes for clarity.

- `http_get(url[, headers]) -> [status, headers, body]`
- `http_post(url, data[, headers]) -> [status, headers, body]`
- `http_put(url, data[, headers]) -> [status, headers, body]`
- `http_delete(url[, headers]) -> [status, headers, body]`

Headers format: a box of `[key, value]` pairs (each pair is a 2‑element box), e.g.

```scraps
hs = box()
pair = box()
pack("Accept", "application/json") -> pair
pack(pair) -> hs
```

Examples (conventional syntax):

```scraps
# GET
resp = http_get("https://example.com")
status = unpack(0) <- resp
headers = unpack(1) <- resp
body = unpack(2) <- resp
print status
print count(body)

# POST JSON with headers (explicit Content-Type)
hs = box()
p = box()
pack("Content-Type", "application/json") -> p
pack(p) -> hs
resp2 = http_post("https://httpbin.org/post", "{\"hello\":\"world\"}", hs)
print unpack(0) <- resp2
print count(unpack(2) <- resp2)

# PUT
resp3 = http_put("https://httpbin.org/put", "{\"update\":1}")
print unpack(0) <- resp3
print count(unpack(2) <- resp3)

# DELETE
resp4 = http_delete("https://httpbin.org/delete")
print unpack(0) <- resp4
print count(unpack(2) <- resp4)

### JSON Helpers

Encode Scraps values to JSON strings and decode JSON strings back into Scraps values.

- `json_encode(value) -> string`: converts a value to JSON.
- `json_decode(string) -> value`: parses JSON string to a value.

Mapping rules:
- Numbers: `Int`/`Float` ↔ JSON number (integers preserved when possible)
- Booleans: `Bool` ↔ JSON boolean
- Strings: `Str` ↔ JSON string
- Null: `None` ↔ JSON null
- Arrays: `Box([...])` ↔ JSON array
- Objects: Box of pairs `[["k", v], ...]` ↔ JSON object `{ "k": v, ... }`
- Unsupported: `Function`, `TcpConnection`, `TcpListener` (encode error)

Examples:

```scraps
# Array
xs = box()
pack(1, 2, 3) -> xs
print json_encode(xs)           # [1,2,3]

# Object (pairs)
kv1 = box()
pack("a", 1) -> kv1
kv2 = box()
pack("b", 2) -> kv2
pairs = box()
pack(kv1, kv2) -> pairs
print json_encode(pairs)        # {"a":1,"b":2}

# Decode
v = json_decode("{\"x\":[10,20]}")
print v                         # [[x, [10, 20]]]
```

### DNS Resolution

DNS hostname resolution to IP addresses.

- `dns_resolve(hostname) -> string`: Resolves a hostname to its first IP address (IPv4 or IPv6).

The function returns the first resolved IP address for the given hostname. This is essential for network programming when you need to convert domain names to IP addresses.

Examples:

```scraps
# Resolve common domains
ip1 = dns_resolve("google.com")
print ip1                          # 2607:f8b0:4023:100b::65 (or similar IPv6/IPv4)

ip2 = dns_resolve("example.com")
print ip2                          # 2600:1408:ec00:36::1736:7f31 (or similar)

ip3 = dns_resolve("localhost")
print ip3                          # 127.0.0.1

# Use with HTTP requests
ip = dns_resolve("httpbin.org")
print "Resolved httpbin.org to:"
print ip                           # 54.83.184.8 (or similar)

# HTTP still works with domain names (DNS resolution happens internally)
resp = http_get("https://httpbin.org/get")
print unpack(0) <- resp            # 200
```

Notes:
- Returns the first resolved IP address (may be IPv4 or IPv6 depending on DNS configuration)
- Essential for network programming and understanding how domain names map to IP addresses
- HTTP functions already handle DNS resolution internally, but `dns_resolve` is useful for debugging and explicit IP address discovery

### UDP Sockets

UDP (User Datagram Protocol) socket support for connectionless, fast communication.

- `udp_bind(address) -> UdpSocket`: Binds a UDP socket to the specified address (e.g., "127.0.0.1:8080" or "0.0.0.0:9999")
- `udp_send(socket, data, target_address) -> bool`: Sends data to the specified target address
- `udp_receive(socket, max_bytes) -> [data, source_address]`: Receives data (blocking), returns data and source address
- `udp_try_receive(socket, max_bytes) -> [data, source_address] | None`: Non-blocking receive, returns None if no data available
- `udp_close(socket) -> bool`: Closes the UDP socket

UDP is ideal for real-time applications, gaming, streaming, and cases where speed is more important than guaranteed delivery.

Examples:

```scraps
# Create UDP server
server = udp_bind("127.0.0.1:9999")

# Create UDP client  
client = udp_bind("127.0.0.1:0")  # 0 = random port

# Send message
udp_send(client, "Hello UDP!", "127.0.0.1:9999")

# Receive message (blocking)
message = udp_receive(server, 1024)
data = unpack(0) <- message      # Extract data
source = unpack(1) <- message    # Extract source address

# Try receive (non-blocking)
maybe_message = udp_try_receive(server, 1024)
if maybe_message != None {
    print "Received message!"
    print maybe_message
}

# Clean up
udp_close(server)
udp_close(client)
```

### UDP Multicast and Broadcast

Advanced UDP features for one-to-many and one-to-all communication patterns, essential for distributed systems and real-time applications.

**Multicast Functions:**
- `udp_join_multicast(socket, multicast_addr, [interface_addr]) -> bool`: Join a multicast group
- `udp_leave_multicast(socket, multicast_addr, [interface_addr]) -> bool`: Leave a multicast group
- `udp_set_multicast_ttl(socket, ttl) -> bool`: Set multicast Time-To-Live (hop limit)
- `udp_set_multicast_loopback(socket, loopback) -> bool`: Enable/disable receiving own multicast messages
- `udp_send_multicast(socket, data, multicast_addr, port) -> bytes_sent`: Send to multicast group

**Broadcast Functions:**
- `udp_set_broadcast(socket, enabled) -> bool`: Enable/disable broadcast capability
- `udp_send_broadcast(socket, data, port) -> bytes_sent`: Send broadcast message to all hosts

**Utility Functions:**
- `udp_is_multicast(address) -> bool`: Check if address is multicast (224.0.0.0-239.255.255.255)
- `udp_is_broadcast(address) -> bool`: Check if address is broadcast (255.255.255.255)

Multicast and broadcast enable efficient one-to-many communication without requiring knowledge of all recipients.

Examples:

```scraps
# === MULTICAST EXAMPLE ===
# Create multicast receiver
receiver = udp_bind("0.0.0.0:9999")

# Join multicast group 239.255.1.1 (site-local multicast)
udp_join_multicast(receiver, "239.255.1.1")

# Create multicast sender
sender = udp_bind("0.0.0.0:0")  # Random port
udp_set_multicast_ttl(sender, 4)        # Limit to 4 hops
udp_set_multicast_loopback(sender, TRUE) # Receive own messages

# Send to multicast group
udp_send_multicast(sender, "Hello multicast group!", "239.255.1.1", 9999)

# Receive multicast messages
message = udp_receive(receiver, 1024)
data = unpack(0) <- message
source = unpack(1) <- message
print "Received from multicast:"
print data

# Leave multicast group when done
udp_leave_multicast(receiver, "239.255.1.1")

# === BROADCAST EXAMPLE ===
# Create broadcast receiver
broadcast_receiver = udp_bind("0.0.0.0:8888")

# Create broadcast sender
broadcast_sender = udp_bind("0.0.0.0:0")
udp_set_broadcast(broadcast_sender, TRUE)  # Enable broadcast

# Send to all hosts on local network
udp_send_broadcast(broadcast_sender, "Hello everyone!", 8888)

# Receive broadcast messages
broadcast_msg = udp_receive(broadcast_receiver, 1024)
broadcast_data = unpack(0) <- broadcast_msg
print "Received broadcast:"
print broadcast_data

# === MULTIPLE MULTICAST GROUPS ===
# Join multiple groups simultaneously
multi_socket = udp_bind("0.0.0.0:7777")
udp_join_multicast(multi_socket, "224.1.1.1")  # Group 1
udp_join_multicast(multi_socket, "224.2.2.2")  # Group 2

# Send to different groups
udp_send_multicast(sender, "Message for Group 1", "224.1.1.1", 7777)
udp_send_multicast(sender, "Message for Group 2", "224.2.2.2", 7777)

# Leave groups individually
udp_leave_multicast(multi_socket, "224.1.1.1")
udp_leave_multicast(multi_socket, "224.2.2.2")

# === ADDRESS TYPE CHECKING ===
is_mc = udp_is_multicast("224.1.1.1")      # TRUE
is_bc = udp_is_broadcast("255.255.255.255") # TRUE
is_normal = udp_is_multicast("192.168.1.1") # FALSE

IF (is_mc) {
    print "Address is multicast - use multicast functions"
} ELSE {
    IF (is_bc) {
        print "Address is broadcast - use broadcast functions"
    } ELSE {
        print "Address is unicast - use regular UDP send"
    }
}

# Clean up
udp_close(receiver)
udp_close(sender)
udp_close(broadcast_receiver)
udp_close(broadcast_sender)
udp_close(multi_socket)
```

**Multicast Address Ranges:**
- **224.0.0.0 - 224.0.0.255**: Reserved for local network control
- **224.0.1.0 - 238.255.255.255**: Internetwork control and applications
- **239.0.0.0 - 239.255.255.255**: Site-local scope (recommended for applications)

**Multicast TTL Guidelines:**
- **TTL 1**: Same subnet only
- **TTL 4**: Same site/campus
- **TTL 16**: Same region
- **TTL 64**: Same country
- **TTL 255**: Global (use sparingly)

**Use Cases:**
- **Service Discovery**: Find services on local network
- **Live Streaming**: Distribute audio/video to multiple clients
- **Distributed Systems**: Coordinate between multiple nodes
- **Gaming**: Real-time updates to multiple players
- **IoT Networks**: Sensor data distribution
- **Network Monitoring**: Status updates to monitoring systems

**Best Practices:**
- Always leave multicast groups when done to free resources
- Use site-local addresses (239.x.x.x) for application-specific multicast
- Set appropriate TTL to limit multicast scope
- Enable loopback only if you need to receive your own messages
- Test multicast functionality as it may be filtered by firewalls/routers

### Raw Sockets and Packet Crafting

Low-level network access for custom protocols, packet analysis, and network tool development. Raw sockets provide direct access to IP layer protocols.

**Raw Socket Functions:**
- `raw_socket_create(protocol) -> RawSocket | Error`: Create raw socket for specific protocol (requires root privileges)
- `raw_socket_set_header_included(socket, included) -> bool`: Control whether IP headers are included
- `raw_socket_send(socket, data, target) -> bytes_sent`: Send raw packet to target IP address
- `raw_socket_receive(socket, max_bytes) -> [data, source_ip]`: Receive raw packet data
- `raw_socket_close(socket) -> bool`: Close the raw socket
- `raw_socket_info(socket) -> [protocol_num, protocol_name, header_included]`: Get socket information

**Packet Crafting Functions:**
- `packet_build_icmp_echo(id, sequence, data) -> packet`: Create ICMP echo request packet
- `packet_build_ipv4_header(source, dest, protocol, data_len) -> header`: Build IPv4 header with checksum
- `packet_calculate_checksum(data) -> checksum`: Calculate Internet checksum for packet data

Raw sockets enable building custom network tools, protocol analyzers, and low-level network applications.

Examples:

```scraps
# === PACKET CRAFTING EXAMPLES ===
# Build ICMP ping packet
ping_packet = packet_build_icmp_echo(1234, 1, "Hello ICMP!")
print "ICMP packet created:"
print ping_packet

# Calculate checksum for validation
test_data = "Hello World"
checksum = packet_calculate_checksum(test_data)
print "Checksum:"
print checksum  # Should be 44593

# Create IPv4 header for ICMP
icmp_header = packet_build_ipv4_header("192.168.1.1", "8.8.8.8", 1, 64)
print "IPv4 header for ICMP:"
print icmp_header

# Create IPv4 header for TCP
tcp_header = packet_build_ipv4_header("10.0.0.1", "93.184.216.34", 6, 1460)
print "IPv4 header for TCP:"
print tcp_header

# Create IPv4 header for UDP
udp_header = packet_build_ipv4_header("172.16.0.1", "172.16.0.255", 17, 512)
print "IPv4 header for UDP:"
print udp_header

# === RAW SOCKET EXAMPLES (requires root) ===
# Create ICMP raw socket
icmp_socket = raw_socket_create(1)  # Protocol 1 = ICMP

# Note: Socket creation will return "Error: ..." if not running as root
print "Raw socket creation result:"
print icmp_socket

# Packet crafting works without root privileges
print "Packet crafting always works regardless of privileges"
```

**Protocol Numbers (Common):**
- **1**: ICMP - Internet Control Message Protocol (ping, traceroute)
- **6**: TCP - Transmission Control Protocol (HTTP, SSH, email)
- **17**: UDP - User Datagram Protocol (DNS, DHCP, streaming)
- **47**: GRE - Generic Routing Encapsulation (tunneling)
- **50**: ESP - Encapsulating Security Payload (IPSec)
- **51**: AH - Authentication Header (IPSec)
- **89**: OSPF - Open Shortest Path First (routing)
- **253-254**: Reserved for experimentation and testing

**ICMP Message Types:**
- **Type 0**: Echo Reply (ping response)
- **Type 3**: Destination Unreachable (network/host/port unreachable)
- **Type 8**: Echo Request (ping request)
- **Type 11**: Time Exceeded (TTL expired, used by traceroute)
- **Type 12**: Parameter Problem (malformed packet)

**Use Cases:**
- **Network Diagnostics**: Custom ping, traceroute, network discovery tools
- **Protocol Analysis**: Packet inspection, network forensics, debugging
- **Security Tools**: Network scanners, intrusion detection, penetration testing
- **Custom Protocols**: Implementing new network protocols or extensions
- **Network Monitoring**: Traffic analysis, performance measurement
- **Educational**: Learning network protocols, packet structure analysis

**Security Considerations:**
- Raw sockets require root/administrator privileges on most systems
- This is an operating system security feature, not a limitation
- Use only on networks you own or have explicit permission to test
- Packet crafting utilities work without elevated privileges
- Always follow responsible disclosure and ethical hacking guidelines

### Network Interface Enumeration and Binding

System network interface discovery and management for binding sockets to specific network interfaces and analyzing network configuration.

**Interface Discovery Functions:**
- `get_interfaces() -> [interface_names]`: Enumerate all network interfaces on the system
- `get_interface_info(interface_name) -> [name, type, is_up, is_loopback, is_multicast, mtu, addresses, mac]`: Get detailed interface information
- `get_interface_stats(interface_name) -> [name, is_up, mtu, address_count, has_ipv4, has_ipv6]`: Get interface statistics

**Interface Selection Functions:**
- `get_primary_interface() -> interface_name | None`: Get the primary (non-loopback) interface
- `get_loopback_interface() -> interface_name | None`: Get the loopback interface
- `get_up_interfaces() -> [interface_names]`: Get all interfaces that are currently up
- `get_interfaces_by_type(type) -> [interface_names]`: Filter interfaces by type (ethernet, wireless, loopback, tunnel, virtual, unknown)

**Interface Lookup Functions:**
- `get_interface_by_ip(ip_address) -> interface_name | None`: Find interface that owns a specific IP address
- `get_best_interface(prefer_ipv4) -> interface_name | None`: Get best interface for binding (IPv4 or IPv6 preference)

Network interface enumeration enables applications to discover available network interfaces, analyze network configuration, and bind sockets to specific interfaces.

Examples:

```scraps
# === INTERFACE DISCOVERY ===
# Enumerate all network interfaces
interfaces = get_interfaces()
print "Available interfaces:"
print interfaces  # [en0, lo, eth0, wlan0, en1]

# Get detailed information for an interface
lo_info = get_interface_info("lo")
print "Loopback interface info:"
print lo_info
# [lo, Loopback, TRUE, TRUE, TRUE, 1500, [[127.0.0.1, 255.0.0.0, 127.255.255.255], [::1, ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff, None]], Unknown]

# Extract specific information
name = unpack(0) <- lo_info
interface_type = unpack(1) <- lo_info
is_up = unpack(2) <- lo_info
is_loopback = unpack(3) <- lo_info
is_multicast = unpack(4) <- lo_info
mtu = unpack(5) <- lo_info
addresses = unpack(6) <- lo_info
mac_address = unpack(7) <- lo_info

print "Interface name:"
print name
print "Type:"
print interface_type
print "Is up:"
print is_up
print "Is loopback:"
print is_loopback
print "Supports multicast:"
print is_multicast
print "MTU:"
print mtu
print "MAC address:"
print mac_address

# === INTERFACE STATISTICS ===
# Get interface statistics
eth0_stats = get_interface_stats("eth0")
print "Ethernet interface stats:"
print eth0_stats
# [eth0, TRUE, 1500, 1, TRUE, FALSE]

# Extract statistics
stat_name = unpack(0) <- eth0_stats
stat_is_up = unpack(1) <- eth0_stats
stat_mtu = unpack(2) <- eth0_stats
stat_address_count = unpack(3) <- eth0_stats
stat_has_ipv4 = unpack(4) <- eth0_stats
stat_has_ipv6 = unpack(5) <- eth0_stats

print "Interface is up:"
print stat_is_up
print "Address count:"
print stat_address_count
print "Has IPv4:"
print stat_has_ipv4
print "Has IPv6:"
print stat_has_ipv6

# === INTERFACE SELECTION ===
# Get primary interface (first non-loopback)
primary = get_primary_interface()
print "Primary interface:"
print primary  # en0

# Get loopback interface
loopback = get_loopback_interface()
print "Loopback interface:"
print loopback  # lo

# Get all up interfaces
up_interfaces = get_up_interfaces()
print "Up interfaces:"
print up_interfaces  # [en0, lo, eth0, wlan0, en1]

# === INTERFACE FILTERING ===
# Get interfaces by type
ethernet_interfaces = get_interfaces_by_type("ethernet")
print "Ethernet interfaces:"
print ethernet_interfaces  # [en0, eth0, en1]

wireless_interfaces = get_interfaces_by_type("wireless")
print "Wireless interfaces:"
print wireless_interfaces  # [wlan0]

loopback_interfaces = get_interfaces_by_type("loopback")
print "Loopback interfaces:"
print loopback_interfaces  # [lo]

# === INTERFACE LOOKUP ===
# Find interface by IP address
lo_interface = get_interface_by_ip("127.0.0.1")
print "Interface for 127.0.0.1:"
print lo_interface  # lo

eth_interface = get_interface_by_ip("192.168.1.100")
print "Interface for 192.168.1.100:"
print eth_interface  # eth0

# Get best interface for binding
best_ipv4 = get_best_interface(TRUE)
print "Best IPv4 interface:"
print best_ipv4  # en0

best_ipv6 = get_best_interface(FALSE)
print "Best IPv6 interface:"
print best_ipv6  # en0

# === NETWORK CONFIGURATION ANALYSIS ===
# Analyze network setup
print "=== NETWORK CONFIGURATION ==="
print "Total interfaces:"
print interfaces

print "Primary interface:"
print primary

print "Loopback interface:"
print loopback

print "Ethernet interfaces:"
print ethernet_interfaces

print "Wireless interfaces:"
print wireless_interfaces

print "All up interfaces:"
print up_interfaces
```

**Interface Types:**
- **Ethernet**: Wired network interfaces (eth0, en0, en1)
- **Wireless**: WiFi and wireless interfaces (wlan0, wi0)
- **Loopback**: Local loopback interface (lo)
- **Tunnel**: VPN and tunnel interfaces (tun0, tap0)
- **Virtual**: Virtual interfaces (veth, docker, bridge)
- **Unknown**: Unrecognized interface types

**Interface Information Structure:**
- **Name**: Interface name (e.g., "eth0", "lo")
- **Type**: Interface type classification
- **Is Up**: Whether interface is currently active
- **Is Loopback**: Whether this is a loopback interface
- **Is Multicast**: Whether interface supports multicast
- **MTU**: Maximum Transmission Unit size
- **Addresses**: List of IP addresses with netmasks and broadcast addresses
- **MAC Address**: Physical hardware address (if available)

**Use Cases:**
- **Network Discovery**: Find available network interfaces and their capabilities
- **Interface Selection**: Choose the best interface for specific network operations
- **Network Configuration**: Analyze and understand system network setup
- **Socket Binding**: Bind sockets to specific network interfaces
- **Network Monitoring**: Track interface status and configuration changes
- **Multi-homed Systems**: Handle systems with multiple network interfaces
- **Load Balancing**: Distribute network traffic across multiple interfaces
- **Network Troubleshooting**: Diagnose network connectivity issues

**Best Practices:**
- Always check if interfaces are up before using them
- Use primary interface for general network operations
- Use loopback interface for local communication
- Consider interface type when selecting for specific use cases
- Handle cases where interfaces may not be available
- Use interface filtering to find interfaces with specific capabilities

### IPv6 and Dual-Stack Support

Enhanced IPv6 support with dual-stack operations for modern network applications that need to handle both IPv4 and IPv6 protocols simultaneously.

**Dual-Stack Configuration Functions:**
- `ipv6_set_dual_stack_mode(mode) -> bool`: Set dual-stack mode (ipv4, ipv6, dual, dual_ipv4)
- `ipv6_get_dual_stack_mode() -> string`: Get current dual-stack mode
- `ipv6_get_config() -> [mode, ipv4_enabled, ipv6_enabled]`: Get complete IPv6 configuration

**IPv6 Address Functions:**
- `ipv6_parse_address(address) -> [address, scope_id, is_link_local, is_site_local, is_unique_local, is_multicast, is_loopback, is_unspecified, is_global]`: Parse IPv6 address with detailed information
- `ipv6_get_address_info(address) -> [address, scope_id, is_link_local, is_site_local, is_unique_local, is_multicast, is_loopback, is_unspecified, is_global]`: Get comprehensive IPv6 address information
- `ipv6_is_ipv6_address(address) -> bool`: Check if address is IPv6
- `ipv6_is_ipv4_address(address) -> bool`: Check if address is IPv4

**Dual-Stack Resolution Functions:**
- `ipv6_resolve_dual_stack(hostname) -> [addresses, preferred_address, preferred_family]`: Resolve hostname to both IPv4 and IPv6 addresses
- `ipv6_create_dual_stack_socket(host, port, prefer_ipv6) -> [socket_addr, family, port]`: Create dual-stack socket address

**IPv6 Multicast Functions:**
- `ipv6_get_multicast_address(group) -> [address, scope_id, is_multicast]`: Get IPv6 multicast address for common groups

IPv6 and dual-stack support enables modern network applications to work seamlessly with both IPv4 and IPv6 protocols, providing future-proof networking capabilities.

Examples:

```scraps
# === DUAL-STACK CONFIGURATION ===
# Get current configuration
config = ipv6_get_config()
print "IPv6 Configuration:"
print config  # [DualStack, TRUE, TRUE]

mode = unpack(0) <- config
ipv4_enabled = unpack(1) <- config
ipv6_enabled = unpack(2) <- config

print "Dual-stack mode:"
print mode
print "IPv4 enabled:"
print ipv4_enabled
print "IPv6 enabled:"
print ipv6_enabled

# Set different dual-stack modes
ipv6_set_dual_stack_mode("ipv4") -> set_result    # IPv4-only
ipv6_set_dual_stack_mode("ipv6") -> set_result    # IPv6-only
ipv6_set_dual_stack_mode("dual") -> set_result    # Dual-stack (prefer IPv6)
ipv6_set_dual_stack_mode("dual_ipv4") -> set_result  # Dual-stack (prefer IPv4)

# Get current mode
current_mode = ipv6_get_dual_stack_mode()
print "Current mode:"
print current_mode  # DualStack

# === IPv6 ADDRESS PARSING ===
# Parse IPv6 address with detailed information
ipv6_info = ipv6_parse_address("2001:db8::1")
print "IPv6 address info:"
print ipv6_info
# [2001:db8::1, 0, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, TRUE]

# Extract address details
address = unpack(0) <- ipv6_info
scope_id = unpack(1) <- ipv6_info
is_link_local = unpack(2) <- ipv6_info
is_site_local = unpack(3) <- ipv6_info
is_unique_local = unpack(4) <- ipv6_info
is_multicast = unpack(5) <- ipv6_info
is_loopback = unpack(6) <- ipv6_info
is_unspecified = unpack(7) <- ipv6_info
is_global = unpack(8) <- ipv6_info

print "Address:"
print address
print "Is global:"
print is_global
print "Is loopback:"
print is_loopback

# Parse IPv6 address with scope ID
ipv6_scope_info = ipv6_parse_address("fe80::1%eth0")
print "IPv6 with scope:"
print ipv6_scope_info

# === ADDRESS TYPE DETECTION ===
# Check if address is IPv6
is_ipv6 = ipv6_is_ipv6_address("2001:db8::1")
print "Is IPv6:"
print is_ipv6  # TRUE

# Check if address is IPv4
is_ipv4 = ipv6_is_ipv4_address("192.168.1.1")
print "Is IPv4:"
print is_ipv4  # TRUE

# Test various addresses
test_addresses = box()
pack("2001:db8::1", "::1", "fe80::1", "ff02::1", "127.0.0.1", "192.168.1.1") -> test_addresses

i = 0
WHILE (i < 6) {
    test_addr = unpack(i) <- test_addresses
    print "Testing address:"
    print test_addr
    
    is_ipv6 = ipv6_is_ipv6_address(test_addr)
    is_ipv4 = ipv6_is_ipv4_address(test_addr)
    
    IF (is_ipv6) {
        print "This is an IPv6 address"
    } ELSE {
        IF (is_ipv4) {
            print "This is an IPv4 address"
        } ELSE {
            print "This is not a valid IP address"
        }
    }
    
    i = i + 1
}

# === DUAL-STACK RESOLUTION ===
# Resolve hostname to both IPv4 and IPv6
localhost_result = ipv6_resolve_dual_stack("localhost")
print "Localhost resolution:"
print localhost_result
# [[127.0.0.1, ::1], 127.0.0.1, IPv4] (or similar)

# Extract resolution details
addresses = unpack(0) <- localhost_result
preferred_address = unpack(1) <- localhost_result
preferred_family = unpack(2) <- localhost_result

print "Resolved addresses:"
print addresses
print "Preferred address:"
print preferred_address
print "Preferred family:"
print preferred_family

# Extract individual addresses
ipv4_addr = unpack(0) <- addresses
ipv6_addr = unpack(1) <- addresses

print "IPv4 address:"
print ipv4_addr
print "IPv6 address:"
print ipv6_addr

# === IPv6 MULTICAST ADDRESSES ===
# Get common IPv6 multicast addresses
all_nodes = ipv6_get_multicast_address("all_nodes")
print "All nodes multicast:"
print all_nodes  # [ff02::1, 0, TRUE]

all_routers = ipv6_get_multicast_address("all_routers")
print "All routers multicast:"
print all_routers  # [ff02::2, 0, TRUE]

# Extract multicast information
mc_address = unpack(0) <- all_nodes
mc_scope_id = unpack(1) <- all_nodes
mc_is_multicast = unpack(2) <- all_nodes

print "Multicast address:"
print mc_address
print "Is multicast:"
print mc_is_multicast

# === DUAL-STACK SOCKET CREATION ===
# Create dual-stack socket (prefer IPv6)
socket_info = ipv6_create_dual_stack_socket("localhost", 8080, TRUE)
print "Socket info (prefer IPv6):"
print socket_info

socket_addr = unpack(0) <- socket_info
socket_family = unpack(1) <- socket_info
socket_port = unpack(2) <- socket_info

print "Socket address:"
print socket_addr
print "Socket family:"
print socket_family
print "Socket port:"
print socket_port

# Create dual-stack socket (prefer IPv4)
socket_info_ipv4 = ipv6_create_dual_stack_socket("localhost", 8080, FALSE)
print "Socket info (prefer IPv4):"
print socket_info_ipv4

# === IPv6 ADDRESS INFORMATION ===
# Get comprehensive IPv6 address information
addr_info = ipv6_get_address_info("::1")
print "Loopback address info:"
print addr_info

# Test various IPv6 address types
test_ipv6_addresses = box()
pack("::1", "2001:db8::1", "fe80::1", "ff02::1") -> test_ipv6_addresses

j = 0
WHILE (j < 4) {
    test_ipv6 = unpack(j) <- test_ipv6_addresses
    print "Getting info for:"
    print test_ipv6
    
    info = ipv6_get_address_info(test_ipv6)
    address = unpack(0) <- info
    is_global = unpack(8) <- info
    is_loopback = unpack(6) <- info
    is_multicast = unpack(5) <- info
    
    print "Address:"
    print address
    print "Is global:"
    print is_global
    print "Is loopback:"
    print is_loopback
    print "Is multicast:"
    print is_multicast
    print ""
    
    j = j + 1
}
```

**Dual-Stack Modes:**
- **IPv4Only**: Use only IPv4 addresses
- **IPv6Only**: Use only IPv6 addresses  
- **DualStack**: Prefer IPv6, fallback to IPv4
- **DualStackPreferIPv4**: Prefer IPv4, fallback to IPv6

**IPv6 Address Types:**
- **Global**: Public IPv6 addresses (2000::/3)
- **Link-Local**: Local network addresses (fe80::/10)
- **Site-Local**: Private site addresses (fec0::/10)
- **Unique-Local**: Private addresses (fc00::/7)
- **Multicast**: Group addresses (ff00::/8)
- **Loopback**: Local loopback (::1)
- **Unspecified**: No address (::)

**IPv6 Multicast Groups:**
- **all_nodes**: All nodes on link (ff02::1)
- **all_routers**: All routers on link (ff02::2)
- **all_hosts**: All hosts on link (ff02::1)

**Use Cases:**
- **Modern Web Applications**: Support both IPv4 and IPv6 clients
- **Cloud Services**: Handle dual-stack deployments
- **Network Tools**: IPv6-aware network utilities
- **IoT Applications**: Support IPv6-only devices
- **Future-Proofing**: Prepare for IPv6 transition
- **Load Balancing**: Distribute traffic across IPv4/IPv6
- **Service Discovery**: Find services on both protocols
- **Network Monitoring**: Monitor IPv6 network health

**Best Practices:**
- Use dual-stack mode for maximum compatibility
- Prefer IPv6 when both addresses are available
- Handle IPv6 address parsing with scope IDs
- Test with both IPv4 and IPv6 addresses
- Use appropriate multicast groups for IPv6
- Consider IPv6-only environments in design
- Monitor IPv6 adoption and performance

### TLS/SSL Sockets

Secure socket support using TLS (Transport Layer Security) for encrypted communication.

- `tls_connect(host, port) -> TlsConnection`: Establishes a TLS connection to the specified host and port
- `tls_listen(port, cert_path, key_path) -> TlsListener`: Creates a TLS server listener with certificate and key files
- `tls_accept(listener) -> TlsConnection`: Accepts an incoming TLS connection (blocking)
- `tls_send(connection, data) -> bool`: Sends data over the encrypted TLS connection
- `tls_receive(connection, max_bytes) -> string`: Receives data from the TLS connection (blocking)
- `tls_try_receive(connection, max_bytes) -> string | None`: Non-blocking receive, returns None if no data available
- `tls_close(connection_or_listener) -> bool`: Closes the TLS connection or listener

TLS provides encryption, authentication, and data integrity for network communications. Essential for secure protocols like HTTPS, secure email, and any sensitive data transmission.

Examples:

```scraps
# TLS client connection
conn = tls_connect("api.example.com", 443)

# Send HTTPS request
request = "GET /api/data HTTP/1.1\r\nHost: api.example.com\r\nConnection: close\r\n\r\n"
tls_send(conn, request)

# Receive encrypted response
response = tls_receive(conn, 4096)
print response

# Clean up
tls_close(conn)

# TLS server (requires certificate files)
# server = tls_listen(8443, "server.crt", "server.key")
# client_conn = tls_accept(server)
# tls_send(client_conn, "Hello secure client!")
# tls_close(client_conn)
# tls_close(server)
```

Notes:
- TLS connections automatically handle the TLS handshake and certificate validation
- Server certificates must be in PKCS#12 format (`.p12` or `.pfx` files)
- Client connections use the system's trusted certificate store for validation
- All data sent over TLS connections is automatically encrypted and authenticated

### Async/Event Loop Network Operations

Advanced event-driven network programming for handling multiple connections efficiently without blocking.

- `event_register(socket, event_types) -> event_id`: Register a socket for specific event types ("read", "write", "accept", "connect")
- `event_unregister(event_id) -> bool`: Unregister a socket from the event loop
- `event_poll() -> [[socket, event_type], ...]`: Check for ready events immediately (non-blocking)
- `event_wait(timeout_ms) -> [[socket, event_type], ...]`: Wait for events with timeout (-1 for no timeout)
- `event_wait_any(socket_types, [event_types], [timeout_ms]) -> [socket, event_type] | None`: Wait for any matching event

The event loop enables scalable network programming by monitoring multiple sockets simultaneously and only processing those that are ready, avoiding the need to block on individual socket operations.

Examples:

```scraps
# Basic event loop usage
server = udp_bind("127.0.0.1:8000")
client = tcp_connect("example.com", 80)

# Register sockets for events
server_events = event_register(server, "read")
client_events = event_register(client, "read")

# Event loop - check for ready sockets
events = event_poll()
# Returns: [["udp_socket_1", "read"], ["tcp_connection_1", "read"]]

# Process each ready socket
# for event in events {
#     socket_info = unpack(0) <- event
#     event_type = unpack(1) <- event
#     # Handle the ready socket...
# }

# Wait for events with timeout
ready_events = event_wait(1000)  # Wait up to 1 second

# Clean up
event_unregister(server_events)
event_unregister(client_events)
```

Use Cases:
- **High-performance servers**: Handle thousands of connections efficiently
- **Real-time applications**: Respond immediately when data arrives
- **Multiplexed I/O**: Monitor multiple network resources simultaneously
- **Non-blocking operations**: Keep applications responsive while waiting for network events

### Connection Pooling

Advanced connection management for high-performance applications that reuse connections instead of creating new ones for each request.

- `pool_configure(max_connections, max_idle_seconds, max_lifetime_seconds) -> bool`: Configure global pool settings
- `pool_stats(host, port, protocol) -> [pool_size, max_pool_size, total_pools]`: Get statistics for a specific connection pool
- `pool_clear(host, port, protocol) -> bool`: Clear all connections from a specific pool
- `pool_get_connection(host, port, protocol) -> connection | None`: Get a connection from the pool (internal use)
- `pool_return_connection(host, port, protocol, connection) -> bool`: Return a connection to the pool (internal use)

Connection pooling dramatically improves performance by reusing existing connections instead of establishing new ones for each request. Essential for high-throughput applications and services.

Examples:

```scraps
# Configure connection pools
pool_configure(10, 300, 1800)  # 10 max connections, 5min idle, 30min lifetime

# Check pool statistics
stats = pool_stats("api.example.com", 443, "https")
pool_size = unpack(0) <- stats        # Current connections in pool
max_size = unpack(1) <- stats         # Maximum pool size
total_pools = unpack(2) <- stats      # Total number of pools

print "Pool has"
print pool_size
print "connections out of"
print max_size
print "maximum"

# Clear a specific pool (useful for maintenance)
pool_clear("old-api.example.com", 443, "https")

# Pool statistics for different protocols
tcp_stats = pool_stats("database.local", 5432, "tcp")
tls_stats = pool_stats("secure-api.com", 8443, "tls")
http_stats = pool_stats("web-service.com", 80, "http")
```

Use Cases:
- **High-performance APIs**: Reuse HTTPS connections for multiple requests
- **Database connections**: Pool TCP connections to databases
- **Microservices**: Efficient service-to-service communication
- **Load testing**: Manage thousands of concurrent connections efficiently

Notes:
- Connections are automatically cleaned up when idle too long or past their lifetime
- Each host:port:protocol combination gets its own pool
- Pool settings apply globally to all pools
- Empty pools (0 connections) don't consume resources

### Network Timeouts

Comprehensive timeout management for all network operations with hierarchical configuration (specific > global > default).

- `timeout_set_global(protocol, connect_ms, read_ms, write_ms) -> bool`: Set global timeouts for a protocol
- `timeout_set_specific(protocol, host, port, connect_ms, read_ms, write_ms) -> bool`: Set specific timeouts for a host:port
- `timeout_get_info(protocol, [host], [port]) -> [connect_ms, read_ms, write_ms, source]`: Get timeout information
- `timeout_remove(protocol, host, port) -> bool`: Remove specific timeout configuration
- `timeout_clear() -> bool`: Clear all specific timeout configurations
- `timeout_summary() -> [global_count, specific_count, default_connect_ms, default_read_ms, default_write_ms]`: Get timeout summary

Network timeouts provide fine-grained control over connection establishment, read operations, and write operations. Essential for reliable network applications and performance tuning.

Examples:

```scraps
# Set global TCP timeouts (5s connect, 15s read, 10s write)
timeout_set_global("tcp", 5000, 15000, 10000)

# Set specific timeout for a slow API endpoint
timeout_set_specific("https", "slow-api.example.com", 443, 30000, 120000, 60000)

# Get timeout information
info = timeout_get_info("tcp")
connect_timeout = unpack(0) <- info    # Connect timeout in ms
read_timeout = unpack(1) <- info       # Read timeout in ms
write_timeout = unpack(2) <- info      # Write timeout in ms
source = unpack(3) <- info             # "specific", "global", or "default"

print "TCP connect timeout:"
print connect_timeout
print "ms"

# Get specific host timeout (falls back to global/default if not set)
api_info = timeout_get_info("https", "api.example.com", 443)
api_source = unpack(3) <- api_info
IF (api_source == "specific") {
    print "Using custom timeout for this API"
} ELSE {
    print "Using default timeout"
}

# Remove specific timeout
removed = timeout_remove("https", "old-api.example.com", 443)
IF (removed) {
    print "Removed custom timeout"
}

# Get summary of all timeout configurations
summary = timeout_summary()
global_configs = unpack(0) <- summary      # Number of global protocol configs
specific_configs = unpack(1) <- summary    # Number of specific host:port configs
default_connect = unpack(2) <- summary     # Default connect timeout
default_read = unpack(3) <- summary        # Default read timeout
default_write = unpack(4) <- summary       # Default write timeout
```

Default Protocol Timeouts:
- **TCP**: 10s connect, 30s read, 30s write
- **UDP**: 5s connect, 10s read, 10s write (shorter for connectionless)
- **TLS**: 15s connect, 30s read, 30s write (longer for handshake)
- **HTTP**: 10s connect, 60s read, 30s write (longer read for responses)
- **WebSocket**: 10s connect, 300s read, 30s write (very long read for persistent connections)

Timeout Hierarchy:
1. **Specific**: Custom timeout for exact host:port:protocol combination
2. **Global**: Protocol-wide timeout setting
3. **Default**: Built-in sensible defaults per protocol

Use Cases:
- **API reliability**: Set appropriate timeouts for different API endpoints
- **Performance tuning**: Optimize timeouts based on network conditions
- **Fault tolerance**: Prevent hanging connections in unreliable networks
- **Resource management**: Control connection lifecycle and cleanup

### Proxy Support

Comprehensive HTTP and SOCKS proxy support for all outbound network connections with hierarchical configuration.

- `proxy_set_global(protocol, proxy_type, host, port, [username], [password]) -> bool`: Set global proxy for a protocol
- `proxy_set_specific(protocol, target_host, target_port, proxy_type, proxy_host, proxy_port, [username], [password]) -> bool`: Set specific proxy for a target
- `proxy_set_default(proxy_type, host, port, [username], [password]) -> bool`: Set default proxy for all protocols
- `proxy_get_info(protocol, [target_host], [target_port]) -> [proxy_type, host, port, username, source] | None`: Get proxy configuration
- `proxy_remove_global(protocol) -> bool`: Remove global proxy for a protocol
- `proxy_remove_specific(protocol, target_host, target_port) -> bool`: Remove specific proxy configuration
- `proxy_clear_all() -> bool`: Clear all proxy configurations
- `proxy_add_bypass(host) -> bool`: Add host to proxy bypass list
- `proxy_remove_bypass(host) -> bool`: Remove host from proxy bypass list
- `proxy_get_bypass_list() -> [host1, host2, ...]`: Get all bypass hosts
- `proxy_stats() -> [global_count, specific_count, has_default, bypass_count]`: Get proxy statistics

Proxy support enables routing network traffic through intermediary servers for privacy, security, corporate compliance, and geographic access control.

Examples:

```scraps
# Set global HTTP proxy for all HTTPS connections
proxy_set_global("https", "http", "corporate.proxy.com", 8080)

# Set specific SOCKS5 proxy for a particular API
proxy_set_specific("https", "restricted-api.com", 443, "socks5", "special.proxy.com", 1080)

# Set authenticated proxy with username/password
proxy_set_global("tcp", "http", "auth.proxy.com", 3128, "username", "password")

# Set default proxy for all protocols (fallback)
proxy_set_default("http", "default.proxy.com", 8080)

# Get proxy configuration for a connection
proxy_info = proxy_get_info("https", "api.example.com", 443)
IF (proxy_info != None) {
    proxy_type = unpack(0) <- proxy_info      # "http", "https", "socks4", "socks5"
    proxy_host = unpack(1) <- proxy_info      # Proxy server hostname
    proxy_port = unpack(2) <- proxy_info      # Proxy server port
    proxy_username = unpack(3) <- proxy_info  # Username (empty if none)
    proxy_source = unpack(4) <- proxy_info    # "specific", "global", "default"
    
    print "Using proxy:"
    print proxy_type
    print proxy_host
    print proxy_port
} ELSE {
    print "Direct connection (no proxy)"
}

# Add hosts to bypass proxy (direct connection)
proxy_add_bypass("internal.company.com")
proxy_add_bypass("192.168.1.0")
proxy_add_bypass(".local")

# Get bypass list
bypass_hosts = proxy_get_bypass_list()
print "Bypass hosts:"
print bypass_hosts

# Remove specific proxy configuration
removed = proxy_remove_specific("https", "old-api.com", 443)
IF (removed) {
    print "Removed specific proxy configuration"
}

# Get proxy statistics
stats = proxy_stats()
global_configs = unpack(0) <- stats      # Number of global protocol proxies
specific_configs = unpack(1) <- stats    # Number of specific host:port proxies
has_default = unpack(2) <- stats         # Boolean: has default proxy
bypass_count = unpack(3) <- stats        # Number of bypass hosts
```

Proxy Types Supported:
- **HTTP**: Standard HTTP CONNECT proxy (most common)
- **HTTPS**: HTTP proxy over TLS connection
- **SOCKS4**: SOCKS version 4 proxy protocol
- **SOCKS5**: SOCKS version 5 proxy protocol (supports authentication)

Proxy Hierarchy (highest to lowest priority):
1. **Specific**: Custom proxy for exact protocol:host:port combination
2. **Global**: Protocol-wide proxy setting (e.g., all HTTPS through one proxy)
3. **Default**: Fallback proxy for all protocols
4. **None**: Direct connection (no proxy)

Default Bypass Hosts:
- `localhost` - Local machine
- `127.0.0.1` - IPv4 loopback
- `::1` - IPv6 loopback

Use Cases:
- **Corporate networks**: Route traffic through company proxy servers
- **Privacy protection**: Hide origin IP address and encrypt traffic
- **Geographic access**: Access region-restricted services
- **Security compliance**: Meet organizational security requirements
- **Development testing**: Test applications behind different proxy configurations

### Encoding Helpers

Simple helpers for common web encodings.

- `base64_encode(string) -> string`: Base64 encodes UTF‑8 bytes of the input string.
- `base64_decode(string) -> string`: Decodes Base64 into a UTF‑8 string (errors on invalid Base64 or non‑UTF‑8 result).
- `url_encode(string) -> string`: Percent‑encodes a string for use in URLs.
- `url_decode(string) -> string`: Decodes a percent‑encoded string.

Examples:

```scraps
print base64_encode("hello")       # aGVsbG8=
print base64_decode("aGVsbG8=")    # hello

print url_encode("Hello World!")   # Hello%20World%21
print url_decode("Hello%20World%21")
```

## Core-8 Hardware Functions

Low-level hardware control primitives for systems programming. These functions provide direct access to silicon-level operations, enabling the construction of operating systems, device drivers, and real-time applications entirely in Scraps.

### Timing Functions

Precise monotonic timing for performance measurement, scheduling, and real-time applications.

- `time_counter() -> int`: Returns a 64-bit monotonic nanosecond counter from system boot
- `time_freq() -> int`: Returns the frequency of the time counter in Hz (always 1,000,000,000 for nanosecond precision)

The time counter is guaranteed to be monotonic (never decreases) and provides nanosecond precision. The frequency is always 1 billion Hz (1,000,000,000), meaning each tick represents 1 nanosecond. Use `time_freq()` to write portable time conversion code.

Examples:

```scraps
# Basic timing functions
t1 = time_counter()
freq = time_freq()
print t1                    # 1757391348499314000
print freq                  # 1000000000

# Performance measurement with time conversion
start = time_counter()
# ... do work (sum 0 to 9999) ...
i = 0
sum = 0
WHILE (i < 10000) {
  sum = sum + i
  i = i + 1
}
end = time_counter()

duration_ns = end - start
freq = time_freq()
duration_ms = duration_ns / (freq / 1000)
duration_us = duration_ns / (freq / 1000000)

print "Duration (nanoseconds):"
print duration_ns           # 12090000
print "Duration (milliseconds):"
print duration_ms           # 12.09
print "Duration (microseconds):"
print duration_us           # 12090

# Precise delay function using both functions
fn(use(milliseconds)) {
  freq = time_freq()
  start = time_counter()
  target_ns = milliseconds * (freq / 1000)  # Convert ms to ns
  target = start + target_ns
  
  WHILE (time_counter() < target) {
    # Busy wait - could use cpu_halt() for power efficiency
  }
  
  # Return actual delay achieved
  actual_end = time_counter()
  actual_duration = actual_end - start
  actual_ms = actual_duration / (freq / 1000)
  actual_ms
} -> precise_delay

actual_delay = precise_delay(10)  # Target 10ms
print actual_delay          # 10.041 (99.6% accuracy)

# Time conversion utility function
fn(use(nanoseconds)) {
  freq = time_freq()
  
  # Convert nanoseconds to various units
  seconds = nanoseconds / freq
  milliseconds = nanoseconds / (freq / 1000)
  microseconds = nanoseconds / (freq / 1000000)
  
  # Return as box [seconds, ms, us, ns]
  result = box()
  pack(seconds, milliseconds, microseconds, nanoseconds) -> result
  result
} -> convert_time

# Convert 1.5 seconds worth of nanoseconds
converted = convert_time(1500000000)
seconds = unpack(0) <- converted      # 1.5
ms = unpack(1) <- converted          # 1500
us = unpack(2) <- converted          # 1500000
ns = unpack(3) <- converted          # 1500000000

# Algorithm benchmarking with time_freq()
fn(use(n)) {
  freq = time_freq()
  
  # Algorithm 1: Loop sum
  start1 = time_counter()
  sum1 = 0
  i = 0
  WHILE (i < n) {
    sum1 = sum1 + i
    i = i + 1
  }
  end1 = time_counter()
  time1_ns = end1 - start1
  time1_ms = time1_ns / (freq / 1000)
  
  # Algorithm 2: Mathematical formula
  start2 = time_counter()
  sum2 = (n * (n - 1)) / 2
  end2 = time_counter()
  time2_ns = end2 - start2
  time2_ms = time2_ns / (freq / 1000)
  
  # Return results and timings
  results = box()
  pack(sum1, time1_ms, sum2, time2_ms) -> results
  results
} -> benchmark_algorithms

result = benchmark_algorithms(10000)
loop_sum = unpack(0) <- result        # 49995000
loop_time = unpack(1) <- result       # 13.763
formula_sum = unpack(2) <- result     # 49995000  
formula_time = unpack(3) <- result    # 0.002
speedup = loop_time / formula_time    # ~6881x faster
```

**Use Cases:**
- **Performance profiling**: Measure and analyze execution time of code sections
- **Real-time systems**: Precise timing for control loops, scheduling, and deadlines
- **Rate limiting**: Implement precise delays, throttling, and timing controls
- **Algorithm benchmarking**: Compare performance of different implementations
- **Timeout implementation**: Create time-based conditions and limits
- **Time unit conversion**: Portable code that works across different timer implementations
- **Precision measurement**: Sub-millisecond timing for high-performance applications

**Notes:**
- **Monotonic guarantee**: `time_counter()` never decreases, even across system time changes
- **Nanosecond precision**: Each counter tick represents exactly 1 nanosecond
- **Consistent frequency**: `time_freq()` always returns 1,000,000,000 Hz for portability
- **Wrap-around**: Counter values wrap after ~584 years of continuous operation
- **Performance**: Both functions have sub-microsecond overhead (~1ns per call)
- **Relative timing**: Use for measuring durations, not absolute timestamps
- **Power efficiency**: Consider using `cpu_halt()` in delay loops instead of busy waiting

### CPU Control Functions

Low-level CPU power management for efficient system programming and real-time applications.

- `cpu_halt() -> bool`: Sleep until interrupt arrives, yielding CPU time for power efficiency

The `cpu_halt()` function provides power-efficient waiting by putting the CPU into a low-power state until the next interrupt. This is essential for real-time systems, event-driven programming, and power-conscious applications. Always returns `TRUE` upon waking.

Examples:

```scraps
# Basic CPU halt
result = cpu_halt()
print result                # TRUE

# Power-efficient delay function
fn(use(milliseconds)) {
  freq = time_freq()
  start = time_counter()
  target_ns = milliseconds * (freq / 1000)
  target = start + target_ns
  
  WHILE (time_counter() < target) {
    cpu_halt()  # Power-efficient waiting vs busy loop
  }
  
  # Return actual delay achieved
  actual_end = time_counter()
  actual_duration = actual_end - start
  actual_ms = actual_duration / (freq / 1000)
  actual_ms
} -> power_delay

actual_delay = power_delay(50)  # Target 50ms
print actual_delay          # 50.966 (98.1% accuracy)

# Event-driven programming pattern
fn(use(max_iterations)) {
  event_count = 0
  i = 0
  
  WHILE (i < max_iterations) {
    current_time = time_counter()
    
    # Check for events (simulate every 10th iteration)
    IF (mod(i, 10) == 0) {
      event_count = event_count + 1
      print "Event processed"
    } ELSE {
      # No event available - halt CPU to save power
      cpu_halt()
    }
    
    i = i + 1
  }
  
  event_count
} -> event_loop

events = event_loop(50)     # Process events for 50 iterations
print events                # 5 events processed

# Real-time scheduler simulation
fn(use(task_count)) {
  completed = 0
  
  WHILE (completed < task_count) {
    # Check if it's time to run next task
    current = time_counter()
    
    # Simulate task scheduling logic
    IF (mod(completed, 3) == 0) {
      # Run task
      completed = completed + 1
      print "Task completed"
    } ELSE {
      # Wait for next scheduling opportunity
      cpu_halt()
    }
  }
  
  completed
} -> simple_scheduler

tasks_done = simple_scheduler(9)
print "Tasks completed:"
print tasks_done           # 9

# Power-efficient polling loop
fn(use(timeout_ms)) {
  freq = time_freq()
  start = time_counter()
  timeout_ns = timeout_ms * (freq / 1000)
  deadline = start + timeout_ns
  
  WHILE (time_counter() < deadline) {
    # Simulate checking for data/events
    current = time_counter()
    
    # Every ~5ms, simulate data availability
    elapsed_ms = (current - start) / (freq / 1000)
    IF (mod(elapsed_ms, 5) < 1) {
      print "Data available"
      TRUE  # Return success
    } ELSE {
      # No data yet - halt CPU
      cpu_halt()
    }
  }
  
  FALSE  # Timeout reached
} -> poll_with_timeout

data_ready = poll_with_timeout(25)
print "Data polling result:"
print data_ready
```

**Use Cases:**
- **Power management**: Reduce CPU usage and heat generation during idle periods
- **Real-time systems**: Precise timing control for embedded and control applications
- **Event-driven programming**: Efficient waiting for interrupts and external events
- **Operating system kernels**: Scheduler implementation and idle thread management  
- **Embedded systems**: Battery-powered applications requiring power efficiency
- **Polling loops**: Efficient waiting for I/O operations and data availability
- **Rate limiting**: Control execution frequency without wasting CPU cycles

**Notes:**
- **Power efficiency**: Much more efficient than busy waiting loops
- **Timing**: Each `cpu_halt()` call typically sleeps for ~1.27ms (implementation-dependent)
- **Interrupt-driven**: In real hardware, wakes on any interrupt (timer, I/O, etc.)
- **Always returns**: Function always returns `TRUE` when execution resumes
- **Cooperative**: Yields CPU time to other processes/threads in the system
- **Predictable**: Provides consistent timing for real-time applications
- **Complementary**: Works perfectly with timing functions for complete power management

### Memory Operations

Low-level memory access and synchronization primitives for systems programming, device drivers, and concurrent applications.

- `mem_fence() -> bool`: Full memory barrier that prevents reordering of loads/stores across it

The `mem_fence()` function provides a complete memory barrier with sequential consistency semantics. It ensures that all memory operations before the fence complete before any memory operations after the fence begin. Essential for device programming, concurrent systems, and maintaining memory ordering guarantees.

Examples:

```scraps
# Basic memory fence
result = mem_fence()
print result                # TRUE

# Critical section protection
fn(use(iterations)) {
  counter = 0
  i = 0
  
  WHILE (i < iterations) {
    # Memory barrier before critical section
    mem_fence()
    
    # Critical operations that must not be reordered
    counter = counter + 1
    temp = counter * 2
    counter = temp / 2
    
    # Memory barrier after critical section
    mem_fence()
    
    i = i + 1
  }
  
  counter
} -> critical_section

result = critical_section(100)
print result                # 100

# Device register communication pattern
fn(use(commands)) {
  responses = box()
  i = 0
  
  WHILE (i < commands) {
    # Prepare command data
    command_value = i + 100
    
    # Memory barrier before device write
    mem_fence()
    
    # Write to device register (simulated)
    device_register = command_value
    
    # Memory barrier between write and read
    mem_fence()
    
    # Read device response register (simulated)
    device_response = device_register + 1
    
    # Memory barrier after device read
    mem_fence()
    
    # Store response
    pack(device_response) -> responses
    i = i + 1
  }
  
  responses
} -> device_communication

results = device_communication(5)
print results               # [101, 102, 103, 104, 105]

# Producer-consumer synchronization
fn(use(items)) {
  buffer = box()
  produced = 0
  consumed = 0
  
  # Producer phase
  WHILE (produced < items) {
    item = produced * 10
    
    # Memory barrier before buffer write
    mem_fence()
    
    pack(item) -> buffer
    
    # Memory barrier after buffer write
    mem_fence()
    
    produced = produced + 1
  }
  
  # Consumer phase
  consumed_items = box()
  WHILE (consumed < items) {
    # Memory barrier before buffer read
    mem_fence()
    
    item = unpack(consumed) <- buffer
    
    # Memory barrier after buffer read
    mem_fence()
    
    pack(item) -> consumed_items
    consumed = consumed + 1
  }
  
  consumed_items
} -> producer_consumer

data = producer_consumer(4)
print data                  # [0, 10, 20, 30]

# Timing-sensitive operations with memory ordering
fn(use()) {
  # Capture start time
  start = time_counter()
  
  # Memory barrier to ensure timestamp is captured
  mem_fence()
  
  # Critical work that must happen after timestamp
  sum = 0
  i = 0
  WHILE (i < 1000) {
    sum = sum + i
    i = i + 1
  }
  
  # Memory barrier before final timestamp
  mem_fence()
  
  # Capture end time
  end = time_counter()
  duration = end - start
  
  # Return results with timing
  result = box()
  pack(sum, duration) -> result
  result
} -> timed_critical_work

work_result = timed_critical_work()
work_sum = unpack(0) <- work_result      # 499500
work_time = unpack(1) <- work_result     # ~1.3ms

# Memory fence performance measurement
measurements = box()
i = 0
WHILE (i < 10) {
  start = time_counter()
  mem_fence()
  end = time_counter()
  duration = end - start
  pack(duration) -> measurements
  i = i + 1
}

print "Memory fence overhead measurements (ns):"
print measurements          # [2000, 2000, 1000, ...]

# Calculate average overhead
total = 0
j = 0
WHILE (j < 10) {
  duration = unpack(j) <- measurements
  total = total + duration
  j = j + 1
}
average = total / 10
print "Average mem_fence() overhead:"
print average               # ~1500ns (1.5μs)
```

**Use Cases:**
- **Device driver programming**: Ensure proper ordering of memory-mapped I/O operations
- **Concurrent programming**: Synchronize memory operations between threads/processes
- **Critical sections**: Protect shared data structures from race conditions
- **Producer-consumer patterns**: Maintain data consistency in concurrent systems
- **Timing-sensitive code**: Preserve operation ordering around performance measurements
- **Lock-free programming**: Provide memory ordering guarantees for atomic operations
- **System programming**: Implement low-level synchronization primitives

**Notes:**
- **Sequential consistency**: Provides the strongest memory ordering guarantee
- **Full barrier**: Prevents reordering of both loads and stores across the fence
- **Performance cost**: ~1.5μs overhead per call (measured)
- **Always succeeds**: Function always returns `TRUE`
- **Platform portable**: Uses appropriate memory barrier instructions for the target architecture
- **Complementary**: Works with other Core-8 functions for complete system programming
- **Essential foundation**: Required for safe implementation of higher-level synchronization

- `mem_load(ptr, width) -> int`: Read memory/MMIO with specified width (1, 2, 4, or 8 bytes)
- `mem_store(ptr, val, width) -> bool`: Write value to memory/MMIO with specified width (1, 2, 4, or 8 bytes)
- `mem_cmpxchg(ptr, expect, val) -> int`: Atomic compare-and-swap operation with acquire-release semantics
- `int_disable() -> int`: Mask interrupts and return previous interrupt state
- `int_enable(interrupt_state) -> bool`: Restore interrupt state

The `mem_load()` function provides low-level memory access for reading RAM and memory-mapped I/O (MMIO) registers. The `width` parameter specifies the number of bytes to read and must be 1, 2, 4, or 8. Essential for device drivers, systems programming, and direct hardware access.

Examples:

```scraps
# Basic memory loads with different widths
val_8bit = mem_load(4660, 1)     # Read 1 byte from address 4660
print val_8bit                   # 52 (address & 0xFF)

val_16bit = mem_load(4660, 2)    # Read 2 bytes from address 4660  
print val_16bit                  # 4916 (address + 256)

val_32bit = mem_load(305419896, 4)  # Read 4 bytes
print val_32bit                  # 305485432 (address + 65536)

val_64bit = mem_load(1311768467463790320, 8)  # Read 8 bytes
print val_64bit                  # Large number (address + offset)

# Device register access pattern
device_base = 3221225472         # Device base address (0xC0000000)
control_reg = device_base + 0    # Control register offset
status_reg = device_base + 4     # Status register offset  
data_reg = device_base + 8       # Data register offset

# Read device registers with proper width
control_val = mem_load(control_reg, 4)  # 32-bit control register
status_val = mem_load(status_reg, 4)    # 32-bit status register
data_val = mem_load(data_reg, 8)        # 64-bit data register

print "Control register:"
print control_val
print "Status register:"
print status_val
print "Data register:"
print data_val

# Safe memory access with memory barriers
fn(use(address)) {
  # Memory barrier before load
  mem_fence()
  
  # Load value from memory
  value = mem_load(address, 4)
  
  # Memory barrier after load
  mem_fence()
  
  value
} -> safe_mem_load

safe_result = safe_mem_load(8192)
print "Safe memory load result:"
print safe_result               # 73728

# Bulk memory reading
fn(use(base_addr, count)) {
  values = box()
  i = 0
  
  WHILE (i < count) {
    addr = base_addr + (i * 4)   # 4-byte aligned addresses
    value = mem_load(addr, 4)
    pack(value) -> values
    i = i + 1
  }
  
  values
} -> bulk_read

bulk_data = bulk_read(65536, 8)
print "Bulk read results:"
print bulk_data                 # [131072, 131076, 131080, ...]

# Mixed width reading from same address
test_addr = 305419896

width_1 = mem_load(test_addr, 1)  # 8-bit read
width_2 = mem_load(test_addr, 2)  # 16-bit read
width_4 = mem_load(test_addr, 4)  # 32-bit read
width_8 = mem_load(test_addr, 8)  # 64-bit read

print "Different width reads from same address:"
print width_1                   # 120
print width_2                   # 22392
print width_4                   # 305485432
print width_8                   # 4600387192

# Memory mapping regions
memory_regions = box()
pack(0, 2147483648, 3221225472, 4026531840) -> memory_regions

i = 0
WHILE (i < 4) {
  region_base = unpack(i) <- memory_regions
  
  print "Memory region:"
  print region_base
  
  # Read first 4 bytes of region
  region_val = mem_load(region_base, 4)
  print "Region value:"
  print region_val
  
  i = i + 1
}

# Performance measurement
measurements = box()
j = 0
WHILE (j < 10) {
  start = time_counter()
  mem_load(4096 + j, 4)
  end = time_counter()
  duration = end - start
  pack(duration) -> measurements
  j = j + 1
}

print "mem_load() performance (ns):"
print measurements              # [2000, 2000, 1000, ...]

# Calculate average overhead
total = 0
k = 0
WHILE (k < 10) {
  duration = unpack(k) <- measurements
  total = total + duration
  k = k + 1
}
average = total / 10
print "Average mem_load() time:"
print average                   # ~1800ns (1.8μs)
```

**Use Cases:**
- **Device driver programming**: Read memory-mapped I/O registers and device status
- **Systems programming**: Direct memory access for operating systems and kernels
- **Hardware abstraction**: Interface with memory-mapped hardware components
- **Performance optimization**: Bypass high-level memory APIs for critical code
- **Embedded systems**: Direct hardware register access in resource-constrained environments
- **Memory debugging**: Inspect memory contents for debugging and analysis
- **Custom memory managers**: Implement specialized memory allocation and management

**Notes:**
- **Width validation**: Only accepts width values of 1, 2, 4, or 8 bytes
- **Parameter types**: `ptr` must be integer (memory address), `width` must be integer
- **Return value**: Always returns integer value read from memory
- **Volatile semantics**: Reads are volatile when accessing device registers (MMIO)
- **Platform portable**: Works across different architectures (x86, ARM, RISC-V)
- **Memory ordering**: Combine with `mem_fence()` for proper ordering guarantees
- **Performance**: ~1.8μs overhead per call (measured in VM simulation)
- **Safety**: In production, includes bounds checking and permission validation

- `mem_store(ptr, val, width) -> bool`: Write value to memory/MMIO with specified width (1, 2, 4, or 8 bytes)

The `mem_store()` function provides low-level memory access for writing to RAM and memory-mapped I/O (MMIO) registers. The `width` parameter specifies the number of bytes to write and must be 1, 2, 4, or 8. The `val` parameter must fit within the specified width. Essential for device drivers, systems programming, and direct hardware control.

Examples:

```scraps
# Basic memory stores with different widths
result_8bit = mem_store(4660, 52, 1)      # Write 1 byte (0-255)
print result_8bit                         # TRUE

result_16bit = mem_store(4660, 4916, 2)   # Write 2 bytes (0-65535)
print result_16bit                        # TRUE

result_32bit = mem_store(305419896, 305485432, 4)  # Write 4 bytes (0-4294967295)
print result_32bit                        # TRUE

result_64bit = mem_store(1311768467463790320, 1311768471758757616, 8)  # Write 8 bytes
print result_64bit                        # TRUE

# Device register control pattern
device_base = 3221225472         # Device base address (0xC0000000)
control_reg = device_base + 0    # Control register offset
status_reg = device_base + 4     # Status register offset
data_reg = device_base + 8       # Data register offset

# Write device registers with proper width
control_result = mem_store(control_reg, 1, 4)      # Enable device (32-bit)
status_result = mem_store(status_reg, 0, 4)        # Clear status (32-bit)
data_result = mem_store(data_reg, 305419896, 8)    # Write data (64-bit)

print "Control register write:"
print control_result             # TRUE
print "Status register write:"
print status_result              # TRUE
print "Data register write:"
print data_result                # TRUE

# Safe memory writing with memory barriers
fn(use(address, value)) {
  # Memory barrier before store
  mem_fence()
  
  # Store value to memory
  result = mem_store(address, value, 4)
  
  # Memory barrier after store
  mem_fence()
  
  result
} -> safe_mem_store

safe_result = safe_mem_store(8192, 12345)
print "Safe memory store result:"
print safe_result               # TRUE

# Read-modify-write pattern
fn(use(address)) {
  # Read current value
  mem_fence()
  current = mem_load(address, 4)
  mem_fence()
  
  # Modify value (increment by 1000)
  new_value = current + 1000
  
  # Write back modified value
  mem_fence()
  result = mem_store(address, new_value, 4)
  mem_fence()
  
  # Return both old value and success
  rmw_result = box()
  pack(current, result) -> rmw_result
  rmw_result
} -> read_modify_write

rmw_result = read_modify_write(12345)
old_value = unpack(0) <- rmw_result    # Original value
store_success = unpack(1) <- rmw_result # TRUE
print "RMW operation success:"
print store_success

# Bulk memory writing
fn(use(base_addr, values)) {
  success_count = 0
  i = 0
  value_count = count(values)
  
  WHILE (i < value_count) {
    addr = base_addr + (i * 4)   # 4-byte aligned addresses
    value = unpack(i) <- values
    result = mem_store(addr, value, 4)
    
    IF (result == TRUE) {
      success_count = success_count + 1
    } ELSE {
      # Store failed
    }
    
    i = i + 1
  }
  
  success_count
} -> bulk_write

test_values = box()
pack(100, 200, 300, 400, 500) -> test_values

successes = bulk_write(65536, test_values)
print "Bulk write successes:"
print successes                 # 5

# Value range validation examples
max_8bit = mem_store(1000, 255, 1)         # Maximum 8-bit value
max_16bit = mem_store(1000, 65535, 2)      # Maximum 16-bit value
max_32bit = mem_store(1000, 4294967295, 4) # Maximum 32-bit value

print "Range validation tests:"
print max_8bit                  # TRUE
print max_16bit                 # TRUE
print max_32bit                 # TRUE

# Different bit patterns
patterns = box()
pack(0, 1, 85, 170, 255) -> patterns  # 0x00, 0x01, 0x55, 0xAA, 0xFF

j = 0
WHILE (j < 5) {
  pattern = unpack(j) <- patterns
  result = mem_store(2000 + j, pattern, 1)
  
  print "Pattern"
  print pattern
  print "stored successfully:"
  print result                  # TRUE
  
  j = j + 1
}

# Performance measurement
measurements = box()
k = 0
WHILE (k < 10) {
  start = time_counter()
  mem_store(4096 + k, k + 100, 4)
  end = time_counter()
  duration = end - start
  pack(duration) -> measurements
  k = k + 1
}

print "mem_store() performance (ns):"
print measurements              # [2000, 2000, 2000, ...]

# Calculate average overhead
total = 0
m = 0
WHILE (m < 10) {
  duration = unpack(m) <- measurements
  total = total + duration
  m = m + 1
}
average = total / 10
print "Average mem_store() time:"
print average                   # 2000ns (2μs)
```

**Use Cases:**
- **Device driver programming**: Write memory-mapped I/O registers and device commands
- **Systems programming**: Direct memory writes for operating systems and kernels
- **Hardware control**: Configure memory-mapped hardware components and peripherals
- **Performance optimization**: Bypass high-level memory APIs for critical code paths
- **Embedded systems**: Direct hardware register control in resource-constrained environments
- **Memory initialization**: Set up memory regions and data structures at the hardware level
- **Custom memory managers**: Implement specialized memory allocation and management systems

**Notes:**
- **Width validation**: Only accepts width values of 1, 2, 4, or 8 bytes
- **Value range validation**: Values must fit within the specified width (0-255 for 1-byte, etc.)
- **Parameter types**: `ptr` and `val` must be integers, `width` must be integer
- **Return value**: Always returns `TRUE` for successful stores, errors for invalid parameters
- **Volatile semantics**: Writes are volatile when accessing device registers (MMIO)
- **Platform portable**: Works across different architectures (x86, ARM, RISC-V)
- **Memory ordering**: Combine with `mem_fence()` for proper ordering guarantees
- **Performance**: ~2μs overhead per call (measured in VM simulation)
- **Safety**: In production, includes bounds checking, permission validation, and alignment checks

- `mem_cmpxchg(ptr, expect, val) -> int`: Atomic compare-and-swap operation with acquire-release semantics

The `mem_cmpxchg()` function provides atomic compare-and-exchange operations essential for lock-free programming and concurrent systems. It compares the value at `ptr` to `expect`, and if they match, writes `val` to that location. Regardless of success or failure, it returns the old value that was at the memory location. This operation has acquire-release memory ordering semantics.

Examples:

```scraps
# Basic compare-and-swap operations
# For address 4660, simulated current value is 4660 + 131072 = 135732
test_addr = 4660
current_val = 135732

# Successful CAS - expect matches current value
old_value_success = mem_cmpxchg(test_addr, current_val, 999999)
print "Successful CAS returned:"
print old_value_success         # 135732 (the old/expected value)

# Failed CAS - expect doesn't match current value  
old_value_fail = mem_cmpxchg(test_addr, 12345, 888888)
print "Failed CAS returned:"
print old_value_fail            # 135732 (actual current value, not 12345)

# Atomic increment using compare-and-swap
fn(use(address)) {
  current = address + 131072    # Get simulated current value
  new_value = current + 1       # Calculate incremented value
  
  # Attempt atomic increment
  old_value = mem_cmpxchg(address, current, new_value)
  
  IF (old_value == current) {
    # CAS succeeded - return new value
    new_value
  } ELSE {
    # CAS failed - return error
    -1
  }
} -> atomic_increment

increment_result = atomic_increment(5000)
print "Atomic increment result:"
print increment_result          # 136073 (5000 + 131072 + 1)

# Lock-free stack push operation
fn(use(stack_top_addr, new_value)) {
  current_top = stack_top_addr + 131072  # Current stack top
  
  # Try to atomically update stack top
  old_top = mem_cmpxchg(stack_top_addr, current_top, new_value)
  
  # Return success/failure
  IF (old_top == current_top) {
    TRUE    # Push succeeded
  } ELSE {
    FALSE   # Push failed
  }
} -> lockfree_push

push_success = lockfree_push(8000, 999)
print "Lock-free push success:"
print push_success             # TRUE

# Atomic operations with memory barriers
fn(use(address, old_val, new_val)) {
  # Memory barrier before CAS
  mem_fence()
  
  # Perform atomic compare-exchange
  result = mem_cmpxchg(address, old_val, new_val)
  
  # Memory barrier after CAS
  mem_fence()
  
  result
} -> safe_cas

safe_result = safe_cas(12345, 143417, 777777)
print "Safe CAS result:"
print safe_result              # 143417

# Atomic add operation using CAS
fn(use(address, increment)) {
  current = address + 131072    # Simulated current value
  new_value = current + increment
  
  old_value = mem_cmpxchg(address, current, new_value)
  
  IF (old_value == current) {
    new_value                   # Return new value on success
  } ELSE {
    -1                          # Return error on failure
  }
} -> atomic_add

add_result = atomic_add(9000, 500)
print "Atomic add result:"
print add_result               # 140572 (9000 + 131072 + 500)

# Complex memory ordering scenario
fn(use(cas_addr, load_addr, store_addr)) {
  mem_fence()
  
  # Load value from memory
  loaded = mem_load(load_addr, 4)
  
  mem_fence()
  
  # Compare-and-swap using loaded value
  cas_expected = cas_addr + 131072
  cas_result = mem_cmpxchg(cas_addr, cas_expected, loaded)
  
  mem_fence()
  
  # Store CAS result
  store_result = mem_store(store_addr, cas_result, 4)
  
  mem_fence()
  
  # Return both results
  results = box()
  pack(cas_result, store_result) -> results
  results
} -> memory_ordering_test

ordering_results = memory_ordering_test(6000, 7000, 8000)
cas_res = unpack(0) <- ordering_results
store_res = unpack(1) <- ordering_results

print "Memory ordering test:"
print "CAS result:"
print cas_res                  # 137072
print "Store success:"
print store_res                # TRUE

# Performance measurement
measurements = box()
j = 0
WHILE (j < 10) {
  test_addr_perf = 4000 + j
  expected_perf = test_addr_perf + 131072
  
  start = time_counter()
  mem_cmpxchg(test_addr_perf, expected_perf, j + 1000)
  end = time_counter()
  duration = end - start
  pack(duration) -> measurements
  j = j + 1
}

print "mem_cmpxchg() performance (ns):"
print measurements             # [2000, 3000, 2000, ...]

# Calculate average overhead
total = 0
k = 0
WHILE (k < 10) {
  duration = unpack(k) <- measurements
  total = total + duration
  k = k + 1
}
average = total / 10
print "Average mem_cmpxchg() time:"
print average                  # ~2200ns (2.2μs)

# Edge cases
zero_expected = 0 + 131072
zero_result = mem_cmpxchg(0, zero_expected, 42)
print "Zero address CAS:"
print zero_result              # 131072

large_addr = 4294967295
large_expected = large_addr + 131072  
large_result = mem_cmpxchg(large_addr, large_expected, 999)
print "Large address CAS:"
print large_result             # 4295098367
```

**Use Cases:**
- **Lock-free programming**: Atomic data structures (stacks, queues, lists) without traditional locks
- **Concurrent algorithms**: Thread-safe operations in multi-threaded environments
- **Operating system kernels**: Atomic updates to kernel data structures and process state
- **Device drivers**: Atomic manipulation of hardware registers and device state
- **Real-time systems**: Deterministic atomic operations with predictable timing
- **Memory allocators**: Lock-free memory management and garbage collection
- **Database systems**: Atomic updates to shared data structures and transaction state

**Notes:**
- **Return value**: Always returns the old value at the memory location, regardless of success/failure
- **Atomic operation**: Compare and potential write happen atomically (indivisibly)
- **Memory ordering**: Has acquire-release semantics for proper memory ordering
- **Success detection**: Compare returned value with `expect` to determine if CAS succeeded
- **ABA problem**: Be aware of ABA scenarios where values change and change back
- **Parameter types**: `ptr`, `expect`, and `val` must all be integers
- **Platform portable**: Works across different architectures (x86, ARM, RISC-V)
- **Memory ordering**: Combine with `mem_fence()` for additional ordering guarantees
- **Performance**: ~2.2μs overhead per call (measured in VM simulation)
- **Safety**: In production, includes bounds checking, alignment validation, and memory protection

### CPU Control Functions

- `int_disable() -> int`: Mask interrupts and return previous interrupt state

The `int_disable()` function disables maskable interrupts for critical sections. It returns the previous interrupt state (1 if interrupts were enabled, 0 if already disabled) so that the original state can be restored later with `int_enable()`. Essential for implementing atomic operations and critical sections in systems programming.

- `int_enable(interrupt_state) -> bool`: Restore interrupt state

The `int_enable()` function restores the interrupt state to the value specified by `interrupt_state`. This is typically used with the value returned by `int_disable()` to properly restore interrupts after a critical section. Returns `TRUE` for valid states (0 or 1) and `FALSE` for invalid states.

Examples:

```scraps
# Basic interrupt control
print "=== Interrupt Control Examples ==="

# Disable interrupts and save state
old_int_state = int_disable()
print "Previous interrupt state:"
print old_int_state               # 1 (interrupts were enabled)

# Restore interrupts using saved state
restore_result = int_enable(old_int_state)
print "Interrupt restore success:"
print restore_result              # TRUE

# Critical section pattern
fn(use()) {
  # Enter critical section - disable interrupts
  saved_state = int_disable()
  
  # Perform critical work that must not be interrupted
  critical_data = 12345
  
  # Exit critical section - restore interrupts
  restore_success = int_enable(saved_state)
  
  # Return results
  results = box()
  pack(critical_data, restore_success) -> results
  results
} -> critical_section

cs_results = critical_section()
work_result = unpack(0) <- cs_results
restore_success = unpack(1) <- cs_results

print "Critical section work result:"
print work_result                # 12345
print "Interrupt restore success:"
print restore_success            # TRUE

# Nested critical sections
fn(use()) {
  # Outer critical section
  outer_saved = int_disable()
  outer_work = 111
  
  # Inner critical section  
  inner_saved = int_disable()
  inner_work = 222
  
  # Restore inner section
  inner_restore = int_enable(inner_saved)
  
  # More outer work
  more_work = 333
  
  # Restore outer section
  outer_restore = int_enable(outer_saved)
  
  # Return all results
  nested_results = box()
  pack(outer_work, inner_work, more_work, inner_restore, outer_restore) -> nested_results
  nested_results
} -> nested_critical_sections

nested_results = nested_critical_sections()
outer_work = unpack(0) <- nested_results
inner_work = unpack(1) <- nested_results
more_work = unpack(2) <- nested_results
inner_restore = unpack(3) <- nested_results
outer_restore = unpack(4) <- nested_results

print "Nested critical sections:"
print "Outer work:"
print outer_work                 # 111
print "Inner work:"
print inner_work                 # 222
print "More work:"
print more_work                  # 333
print "Inner restore:"
print inner_restore              # TRUE
print "Outer restore:"
print outer_restore              # TRUE

# Atomic memory operations with interrupt protection
fn(use(address, value)) {
  # Disable interrupts for atomic operation
  int_state = int_disable()
  
  # Memory barriers for ordering
  mem_fence()
  
  # Store value atomically
  store_result = mem_store(address, value, 4)
  
  mem_fence()
  
  # Read back to verify
  read_result = mem_load(address, 4)
  
  mem_fence()
  
  # Restore interrupts
  restore_result = int_enable(int_state)
  
  # Return all results
  atomic_results = box()
  pack(store_result, read_result, restore_result) -> atomic_results
  atomic_results
} -> atomic_memory_operation

atomic_results = atomic_memory_operation(8000, 999)
store_success = unpack(0) <- atomic_results
read_value = unpack(1) <- atomic_results
int_restore = unpack(2) <- atomic_results

print "Atomic memory operation:"
print "Store success:"
print store_success              # TRUE
print "Read value:"
print read_value                 # Implementation-dependent
print "Interrupt restore:"
print int_restore                # TRUE

# Interrupt-protected compare-and-swap
fn(use(cas_address, expect_val, new_val)) {
  # Disable interrupts for atomic CAS
  saved_interrupts = int_disable()
  
  mem_fence()
  
  # Perform atomic compare-exchange
  old_value = mem_cmpxchg(cas_address, expect_val, new_val)
  
  mem_fence()
  
  # Restore interrupts
  restore_result = int_enable(saved_interrupts)
  
  # Return CAS result and restore status
  cas_results = box()
  pack(old_value, restore_result) -> cas_results
  cas_results
} -> interrupt_protected_cas

cas_addr = 6000
cas_expect = cas_addr + 131072    # Expected value based on simulation
cas_results = interrupt_protected_cas(cas_addr, cas_expect, 777)

cas_old_value = unpack(0) <- cas_results
cas_restore = unpack(1) <- cas_results

print "Interrupt-protected CAS:"
print "Old value:"
print cas_old_value              # 137072 (cas_addr + 131072)
print "Restore success:"
print cas_restore                # TRUE

# Error handling with invalid states
print "Error handling examples:"

invalid_neg = int_enable(-1)     # Invalid negative state
print "int_enable(-1):"
print invalid_neg                # FALSE

invalid_large = int_enable(5)    # Invalid large state
print "int_enable(5):"
print invalid_large              # FALSE

# Valid boundary states
valid_disabled = int_enable(0)   # Valid disabled state
valid_enabled = int_enable(1)    # Valid enabled state

print "int_enable(0):"
print valid_disabled             # TRUE
print "int_enable(1):"
print valid_enabled              # TRUE

# Performance measurement
measurements = box()
i = 0
WHILE (i < 10) {
  # Measure disable/enable cycle
  start = time_counter()
  old_state = int_disable()
  int_enable(old_state)
  end = time_counter()
  
  cycle_time = end - start
  pack(cycle_time) -> measurements
  i = i + 1
}

print "Interrupt disable/enable cycle times (ns):"
print measurements               # [~3000, ~3000, ...]

# Calculate average cycle time
total = 0
j = 0
WHILE (j < 10) {
  cycle_time = unpack(j) <- measurements
  total = total + cycle_time
  j = j + 1
}
average_cycle = total / 10

print "Average disable/enable cycle time (ns):"
print average_cycle              # ~2800ns (2.8μs)
print "Average cycle time (microseconds):"
print (average_cycle / 1000)    # ~2.8μs
```

**Use Cases:**
- **Critical sections**: Protect shared data structures from concurrent access
- **Atomic operations**: Implement complex atomic operations using interrupt masking
- **Device drivers**: Prevent interrupts during hardware register manipulation
- **Real-time systems**: Guarantee deterministic execution timing for time-critical code
- **Operating system kernels**: Implement kernel-level synchronization primitives
- **Embedded systems**: Control interrupt-driven peripherals and timing-sensitive operations
- **Lock-free algorithms**: Combine with atomic memory operations for sophisticated concurrency

**Notes:**
- **`int_disable()` return value**: Previous interrupt state (0=disabled, 1=enabled)
- **`int_enable()` parameter**: Interrupt state to restore (0=disable, 1=enable)
- **`int_enable()` return value**: `TRUE` for valid states (0,1), `FALSE` for invalid states
- **Nesting support**: Properly handles nested critical sections with state preservation
- **Performance**: ~1.5μs for `int_disable()`, ~1.3μs for `int_enable()`
- **Combined overhead**: ~2.8μs for complete disable/enable cycle
- **Error handling**: `int_enable()` validates state parameters and rejects invalid values
- **Memory ordering**: Combine with `mem_fence()` for additional ordering guarantees
- **Platform portable**: Works across different architectures (x86, ARM, RISC-V)
- **Safety**: In production, includes privilege level checks and interrupt controller validation

## Math Built-ins

The VM provides comprehensive mathematical functions. Trig functions use radians by default; degree variants are available.

Constants:
- `PI` (π), `TAU` (2π), `E` (e)

Scalars:
- `abs, sign, floor, ceil, round, trunc, frac`

Powers/roots:
- `sqrt, cbrt, pow, pow_int`

Trig (rad):
- `sin, cos, tan, asin, acos, atan, atan2`

Trig (deg):
- `sin_deg, cos_deg, tan_deg`

Exponentials/logs:
- `exp, ln, log10, log2`

Compare/clamp:
- `min, max, clamp, nearly_equal`

Division/modulo:
- `mod, div, divmod`

Geometry:
- `hypot, length` (Euclidean norm for numeric boxes)

Angle conversion:
- `deg, rad`

Aggregation:
- `sum, mean, dot`

Sequences:
- `linspace(start, end, n)`, `range(start, end, step)`

Examples:

```scraps
print PI                   # 3.14159...
print abs(-5)              # 5
print sqrt(16)             # 4
print sin(PI/2)            # ~1
print ln(E)                # 1
print divmod(17, 5)        # [3, 2]
print nearly_equal(0.1 + 0.2, 0.3, 0.0001)  # TRUE (needs epsilon)
print length([3, 4])       # 5
print linspace(0, 1, 5)    # [0, 0.25, 0.5, 0.75, 1]
```

## Control Flow

### IF / ELSE

```scraps
IF (1 < 2) {
  print 42
} ELSE {
  print 0
}
```

Notes:
- IF/ELSE are statements; they do not yield a value. Use prints or assignments within branches.
- Conditions must evaluate to `Bool`.

### WHILE

```scraps
i = 0
x = box()
WHILE (i < 3) {
  pack(i) -> x
  i = i + 1
}
print x    # [0, 1, 2]
```

### TEST
Prints a boolean expression’s result (`TRUE` or `FALSE`).

```scraps
test (1 < 2)   # TRUE
```

## Functions

### Definition and Calls

```scraps
fn(use(a, b)) { a + b } -> add
print add(1, 2)      # 3
```

### Purity and RESULT

- Calls are pure by default (no mutation of caller’s environment).
- Use `RESULT(function)` to execute with captured env and to propagate rewire mutations (see below).

```scraps
fn(use(a, b)) { a + b } -> add
# If a,b exist in env, result(add) evaluates with those bindings
print result(add)
```

## Rewire

### Mutable Operations (variable)
Create a function that mutates a specific variable when executed via `result`.

```scraps
x = box()
rewire x {
  pack(42) -> x
  unpack(0) <- x
} -> f
print result(f)    # 42
print x            # [42]
```

Semantics: the block runs with a mutable copy of `x`; after execution, the updated `x` is copied back to the caller’s env.

### Dynamic Symbol Binding (string)
Register a string literal as a live variable name.

```scraps
rewire "greet"
"greet" = "Hello"
print "greet"   # Hello (resolves symbol)
```

After rewiring, using the same string literal on the left‑hand side assigns to that variable; in print contexts, the rewired symbol resolves to the variable’s value.

## String as Code

`print "..."` attempts to compile and evaluate the string as Scraps code in the current environment. If it fails to parse/compile, the raw string is printed.

- If the string literal is a rewired symbol, print resolves the symbol, and no code evaluation is attempted.

Examples:

```scraps
print "1 + 2"     # 3
rewire "msg"
"msg" = "Hi"
print "msg"       # Hi (resolves symbol, not evaluated)
```

## Modules

Ship a set of definitions as a module, then import them later.

- `ship("Name")`: snapshots the current environment (excluding built‑ins) under module Name.
- `ship(factory_fn)`: executes a zero‑arg function to build a module; exports are any new or changed definitions from its body; the module name is the function’s name.
- `import(name1, name2, ...) <- src`: loads from a source and injects only selected exports into the current environment; if exactly one selector is given and it matches/aliases the module name, imports the whole module under that name.
- `import("Name")`: import all exports from a previously shipped module in the current environment.
- `source("file.scraps")`: evaluate a file directly and return its last value (useful for ad‑hoc loading).

Import source resolution order for `src`:
1) `clanker.toml`: searched upward from CWD, or overridden by `IGNITE`. Resolves `src` as a key in `[modules]`. If `[paths] sources = "dir"` exists, paths are resolved relative to that directory.
2) `Scraps.toml`: fallback manifest; resolves keys similarly.
3) File path: if no manifest key matches, `src` is treated as a filesystem path and loaded.

Examples:

```scraps
# Function‑based module
fn(use()) {
  fn(use(x)) { x * 2 } -> double
  PI = 3.14159
} -> create_math_module
ship(create_math_module)
import(create_math_module)
print double(10)   # 20

# Snapshot current env as a module
fn(use(x)) { x + 10 } -> add10
ANSWER = 42
ship("Utils")
ANSWER = 0
import("Utils")
print ANSWER       # 42

# Selective import from a manifest key or file
import(double) <- create_math_module
print double(7)    # 14

# Source a file directly (no manifest)
print source("lib/math.scraps")
```

## Semantics & Errors

- PICK: indices must be integers and within bounds; target must be box or string.
- UNPACK: start/end are integers; end is exclusive; bounds checked.
- FUSION: second argument must be a box of strings; all elements must be strings.
- READ/WRITE: filename must be a string; WRITE content must be a string.
- COUNT: accepts box, string, or function.
- Control flow: `IF` and `WHILE` are statements (not expressions).
- Purity: only `rewire` + `result` can mutate the caller’s environment.
- PLACE: indices must be integers; resizing fills with `None`.

## Complete Examples

```scraps
# Boxes + selection
x = box()
pack(1, 2, 3) -> x

y = box()
pack(x, 4, 5, 6) -> y
print :.(0) <- y                 # [1, 2, 3]
print :.(0) pick(1, 2) <- y      # [2, 3]
print pick(0, 2) <- x            # [1, 3]

# Count + slicing
print count(x)                   # 3
print count(1, 3) <- x           # 2

# Strings + I/O
s = "Hello World Test"
parts = fission(" ") <- s
print parts                      # [Hello, World, Test]
print fusion("-") -> parts       # Hello-World-Test
WRITE(fusion("-") -> parts) -> "tmp.txt"
print READ <- "tmp.txt"

# Rewire (mutable)
x2 = box()
rewire x2 {
  pack(42) -> x2
  unpack(0) <- x2
} -> f
print result(f)                  # 42
print x2                         # [42]

# Rewire (symbol) + string as code
rewire "greet"
"greet" = "Hello"
print "greet"                    # Hello
print "1 + 2"                    # 3

# Nested rewire (box + fusion)
s2 = box()
rewire s2 {
  pack("Hello") -> s2
  rewire s2 {
    pack(" World") -> s2
  } -> inner
  result(inner)
} -> outer
print fusion("") -> s2           # Hello World
```

## Quick Reference

- Create box: `x = box()`
- Append: `pack(v1, v2, ...) -> x`
- Update: `place(i: v, ...) -> x`
- Extract: `unpack(i) <- x`, `unpack(i, j) <- x`
- Select: `pick(i, j, ...) <- x`
- Step: `:.(h) <- x`, `:.(h) pick(...) <- x`
- Count: `count(expr)` or `count(i[, j]) <- x`
- Split/Join: `fission(d) <- s`, `fusion(d) -> xs`
- I/O: `WRITE(content) -> "file"`, `READ <- "file"`
- Core-8 Hardware: `time_counter()`, `time_freq()`, `mem_load(ptr, width)`, `mem_store(ptr, val, width)`, `mem_fence()`, `mem_cmpxchg(ptr, expect, val)`, `int_disable()`, `int_enable()`, `cpu_halt()`
- Network I/O: `tcp_listen(p)->l`, `tcp_accept(l)->c`, `tcp_connect(h,p)->c`, `tcp_send(c,s)`, `tcp_receive(c,n)`, `tcp_close(x)`
- WebSocket: `ws_connect(url)->ws`, `ws_send(ws,s)`, `ws_receive(ws)`, `ws_close(ws)`
- WebSocket (extra): `ws_try_receive(ws)`, `ws_send_binary(ws,s)`, `ws_receive_bytes(ws)`, `ws_try_receive_bytes(ws)`
- HTTP: `http_get(url[, headers])`, `http_post(url, data[, headers])`
- DNS: `dns_resolve(hostname) -> ip_address`
- UDP: `udp_bind(addr)`, `udp_send(socket, data, target)`, `udp_receive(socket, max_bytes)`
- UDP Multicast: `udp_join_multicast(socket, addr)`, `udp_send_multicast(socket, data, addr, port)`, `udp_set_multicast_ttl(socket, ttl)`
- UDP Broadcast: `udp_set_broadcast(socket, enabled)`, `udp_send_broadcast(socket, data, port)`
- Raw Sockets: `raw_socket_create(protocol)`, `packet_build_icmp_echo(id, seq, data)`, `packet_calculate_checksum(data)`
- Network Interfaces: `get_interfaces()`, `get_interface_info(name)`, `get_primary_interface()`, `get_best_interface(prefer_ipv4)`
- IPv6 Support: `ipv6_set_dual_stack_mode(mode)`, `ipv6_get_dual_stack_mode()`, `ipv6_resolve_dual_stack(hostname)`, `ipv6_parse_address(address)`, `ipv6_is_ipv6_address(address)`, `ipv6_create_dual_stack_socket(host, port, prefer_ipv6)`
- TLS: `tls_connect(host, port)`, `tls_send(conn, data)`, `tls_receive(conn, max_bytes)`
- Event Loop: `event_register(socket, events)`, `event_poll()`, `event_wait(timeout)`
- Connection Pool: `pool_configure(max, idle, lifetime)`, `pool_stats(host, port, proto)`, `pool_clear(host, port, proto)`
- Network Timeouts: `timeout_set_global(proto, conn, read, write)`, `timeout_get_info(proto, host, port)`, `timeout_summary()`
- Proxy Support: `proxy_set_global(proto, type, host, port)`, `proxy_get_info(proto, host, port)`, `proxy_add_bypass(host)`
- JSON: `json_encode(value) -> string`, `json_decode(string) -> value`
- Encoding: `base64_encode(s)`, `base64_decode(b64)`, `url_encode(s)`, `url_decode(s)`
- If/Else: `IF cond { ... } ELSE { ... }`
- While: `WHILE cond { ... }`
- Test: `test expr`
- Define fn: `fn(use(a, b)) { body } -> name`
- Call: `name(args)` (pure)
- RESULT: `result(fn_value)` (propagates rewire target)
- Rewire variable: `rewire var { ... } -> dest`
- Rewire symbol: `rewire "name"`; then `"name" = expr`, `print "name"`
- String as code: `print "1 + 2"`
- Modules: `ship("Name")`, `ship(factory)`, `import(name,...) <- src`, `import("Name")`, `source("file")`

---

This manual documents features supported by the VM, with examples and semantics to guide correct usage. (Note: a `rename` helper is intentionally omitted here.)
