# Troubleshooting

## "the site API refused this address (HTTP 403)"

The room lookup is behind Cloudflare and some addresses (datacentres, some VPN
exits) are refused. Set a proxy in Settings, for example a local Tor:

```sh
brew install tor && brew services start tor
# then in Settings: socks5h://127.0.0.1:9050
```

Media segments always go direct, so the proxy only carries the small lookups.

## "the playlist advertises no MOUFLON key we hold"

The CDN rotated its pkey. Add a fresh `pkey:pdkey` pair in Settings, or set
`PD_KEY_<n>=<pkey>:<pdkey>` in the environment before launching the app.

## ffmpeg missing

Health shows `ffmpeg missing` and recordings are not remuxed. Install ffmpeg or
point Settings at the binary. Everything already recorded still plays.

## A recording ended early

If the room went offline, that is expected. If it stops while the room is live,
check the job message in the queue: it names the failure. Interrupted files are
kept, never deleted.

## The output file is a strange length

A `.recording.mp4` that was killed mid-write keeps correct content but may
report odd duration metadata. Remux it (`convert --preset copy`) and the
duration is right.

## Nothing is recorded but the room is live

Check that the encryption keys row in Health is green; a stale pair
records nothing because no segment URI decodes.
