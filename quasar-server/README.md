Quasar is a way for multiple clients to connect to a single channel. Quasar by itself does not provide much application functionality, but serves as a simple way for persistent client connections to be made.

A simple websocket server that multiple clients can connect to. The protocol works like this:
- A user can establish a websocket one of three ways:
    - `/ws/new`: Create a new channel. This returns a channel ID that is a UUID. The UUID functions as the channel secret for the duration of the session. As part of the control protocol the user can request a "channel code" which temporarily provide another user the ability to connect to the given channel using a human readable shortcode.
    - `/ws/connect?id={channel_uuid}`: Connect to a channel at the given UUID.
    - `/ws/connect?code={channel_code}`: Connect to the channel at a given code.

A channel is closed immediately after all connected users leave. No data is persisted, all Quasar does is provide a mechanism for clients to broadcast changes among each other.

The wire format for the message protocol for the websocket is just JSON. There is some future work here to allow binary data to be sent without paying the price of base64 encoding/decoding.

The control protocol is as follows:
- `{ type: 'generate_code' }`: Generate a code. Quasar will respond with `{ type: 'generated_code', code: string }`.
- `{ type: 'data', content: Blob }`: Broadcast a data message to all clients with the same shape.

The generated code is a human readable code generated as {[0-100]}-{[word in wordlist]}-{[word in wordlist]} - similar to the connection string used for magic-wormhole. The security model is as follows:
- At any given time, only one code for a given channel number (0-100) is "pending".
- Quasar maintains the mapping of `pending_connects = { channel_number: (channel_uuid, channel_code) }`.
- When a client connects at `client_code = {channel_number_input}-{word1}-{word2}`. We check to see if `pending_connects[channel_number_input][1] == client_code`.
    - If the code is correct, we connect the channel to the event loop for the correct channel, and remove the pending channel from the map, making the channel number available for use again.
    - If the code is incorrect, we remove the pending channel number from the map. The client that generated the code has to regenerate another one.


Claude took a first pass at this, but I don't like Claude's abstractions. What are the invariants of the program?
- There is a global state manager of all the channels. When a new channel is created, we create an entry in `HashMap<uuid, Channel>`. We then do the same thing as connect: `channel.add(client)`.
- A channel manages it's own pending 
- Every channel id 
