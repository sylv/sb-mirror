# sb-mirror

> Experimental/alpha, syncing may not even work properly. You should use an existing mirror instead of running your own because each mirror puts more strain on the main server, which will just make things worse.

> Be warned that this code is... not fantastic. Just wait until you see the horrors on page 36.

Mirrors [SponsorBlock](https://sponsor.ajay.app/) to reduce load on the main server and smooth over downtime. Only implements the lookup part of the API, anything unmatched will be proxied to the main server.

The mirror updates automatically from the CSV file specified with `SPONSOR_TIMES_CSV_URL` using partial HTTP requests (`Range` headers). `User-Agent` will be set to `sb-mirror (...)` if you need to block the mirror, for example `sb-mirror (mirror-0.1.0, https://github.com/sylv/sb-mirror)` for the sync requests and `sb-mirror (proxy, https://github.com/sylv/sb-mirror)` for the proxy requests.

## environment variables

| Name            | Description                         | Default                                         |
| --------------- | ----------------------------------- | ----------------------------------------------- |
| `CSV_URL`       | URL of the CSV file to mirror       | `https://mirror.sb.mchang.xyz/sponsorTimes.csv` |
| `SYNC_INTERVAL` | Interval between syncs (in seconds) | `300`                                           |
| `DATA_PATH`     | Where to store the SQLite+CSV files | `/data`                                         |

## usage

This is not ready for an actual deployment, just use one of the other mirrors. If you're confident you want to make a mistake, `docker.io/sylver/sb-mirror` is the image, listening on port `4100`. Make sure you mount `/data` or your `DATA_PATH` to a persistent volume, like a host mount. Initial startup will take a while to download and import the CSV file, but after that it will keep itself up to date without downtime.

## todo

- Add an option to serve `sponsorTimes.csv`
- Change the name because [its already taken](https://github.com/mchangrh/sb-mirror) :(
- Add an option to serve a torrent of `sponsorTimes.csv`, would be possible with [web seed](https://getright.com/seedtorrent.html) where it just points to the HTTP url to kickstart the swarm.
- Have an rsync option for the CSV file, should also make it possible to use zstd-compressed CSV files that some mirrors provide.
