<p align="left">
  <img src="wanshitong-web/static/wst_logo.png" alt="The Owl" width="200">
</p>

# Wan Shi Tong (万事通)

The world's best media librarian. Free for noncommercial use.

> *"If you're going to lie to an all-knowing spirit being, you should at least put some effort into it."* - Wan Shi Tong

## TL;DR

You can self-host WST and point it at your mess of a media collection to produce
a knowledge graph. Then export that as a Jellyfin or Plex-compatible directory
of symlinks to the original media files. WST never mutates or renames your media
files or directories, and if it makes a labeling mistake, you can audit *why* it
made that decision and override it.

WST catalogs movies and TV series.
It doesn't handle music or books yet, but if there is interest, support can be added.

**Status:** beta.
WST has only been released for a short time, and the user community is small.

WST is **NOT** a media server, player, or transcoder.
It wants to play nice with your existing media server, slotting into a cozy
position between your erratic heap of files and a pristine Jellyfin dashboard.

## The Wan Shi Tong ethos

Popular media servers (Jellyfin, Plex, etc.) are responsible for much more than playback.
They scan filesystems, interpret directory structures, parse filenames, fetch metadata,
and construct an internal understanding of your library.

This coupling creates several problems:

* Each application must independently infer what your media “means”
* File layout and naming must conform to the expectations of the consumer
* Different tools require different structures for the same files
* Torrent seeding and archival layouts are fragile or incompatible
* Metadata errors are hard to audit, explain, or correct
* Re-scanning is opaque and often destructive

**WST addresses this by decoupling the representation of media knowledge from
the applications that consume it, exposing a canonical, reusable understanding
of your library instead of embedding that logic inside each media server.**

## How it works

![How Wan Shi Tong labels a media library](docs/how-it-works.svg)

## Getting started

WST is a single binary, `wst`. Run `wst serve` somewhere that can read your
media library and open the web UI on port 6765. It will walk you through the rest.

Whichever way you run it, the one thing to get right is paths. A view is a
directory of symlinks to your media by absolute path, so your media server
must see the library at the same paths WST does. If either runs in a
container, mount the library and the view directory into both at the same
paths, as the Compose example below does.

### Docker Compose

[`deploy/docker-compose.yml`](deploy/docker-compose.yml) runs WST beside
Jellyfin, with the library at `/srv/media` and views at `/srv/views` in both.
Copy it, set your paths, then:

```sh
mkdir -p wanshitong/config wanshitong/data wanshitong/cache jellyfin /srv/views
docker compose up -d
```

The image is `github.com/nanosnack/WanShiTong`, for x86_64 and aarch64,
with ffprobe included. It serves on every address, keeps its config, data and
cache at `/config`, `/data` and `/cache`, and reports its own health, so
`docker ps` shows whether it's up. The CLI runs inside the container:
`docker exec wanshitong wst unidentified`.

### NixOS

Add this repository as a flake input, import its module
(`wanshitong.nixosModules.default`), and enable the service:

```nix
services.wanshitong = {
  enable = true;
  settings.host = "0.0.0.0";      # listen beyond localhost
  openFirewall = true;
  libraryPaths = [ "/mnt/media" ]; # mounted read-only for the service
  viewPaths = [ "/srv/views" ];    # where views may be written
  viewGroup = "media";             # a group that can write to viewPaths
};
```

### A binary and systemd

Each [release](https://github.com/nanosnack/WanShiTong/releases) has a
static `wst` for x86_64 and aarch64 Linux, which runs on any distribution. It
needs `ffprobe` (part of FFmpeg) on its `PATH`, or `ffprobe_path` set in
`config.toml`.

```sh
tar -xzf wst-*-x86_64-unknown-linux-musl.tar.gz
./wst serve
```

That keeps everything under your home directory: `config.toml` in
`~/.config/wst`, the database in `~/.local/share/wst` and the cache in
`~/.cache/wst`, each also settable by a flag or `WST_CONFIG_DIR`, `WST_DATA_DIR`
and `WST_CACHE_DIR`. To run it as a service,
[`deploy/wanshitong.service`](deploy/wanshitong.service) is a hardened systemd
unit with the same layout as the NixOS module: settings in `/etc/wanshitong`,
the database in `/var/lib/wanshitong`. Its comments say how to install it.

### From source

You need [Rust](https://rustup.rs), TypeScript (`npm install -g typescript`)
to build, and `ffprobe` to run.

```sh
cargo install --locked --git https://github.com/nanosnack/WanShiTong wst
wst serve
```

`docker build -t wanshitong .` builds the container image instead.

### Settings, backups and upgrades

Settings live in `config.toml` in the config directory, which is written with
the defaults on first run. Any of them can be overridden by environment
variables named `WST__<KEY>`. WST listens on `127.0.0.1` by default, so set
`host = "0.0.0.0"` to reach it from another machine. The web UI's Config page
shows the settings in effect.

Everything WST knows is in one SQLite database, `db.sqlite` in the data
directory; back that up, while WST is stopped or with
`sqlite3 db.sqlite ".backup db-backup.sqlite"`. The cache directory only holds
what it fetched from TMDB and can be deleted. To upgrade, replace the binary or image and restart:
the database migrates itself on start.

Most of the database can be made again by ingesting the files and looking
works up afresh. The corrections people and agents have made can't, so they
have a backup of their own: the Assertions page's Export, or
`wst assertions export -o assertions.json`, writes the assertions in effect,
and its Import, or `wst assertions import assertions.json`, makes them again
in another instance, matching their data sources by name. Set up the new
instance's data sources with the same names first.

## REST API and CLI

While humans will likely prefer to use the UI, the CLI is designed to make it
easy for both humans and LLMs to find unidentified or misidentified media, audit
why automatic identification failed, and make corrections.

```sh
wst unidentified                   # video files that weren't identified
wst marks                          # files people marked as wrong in the web UI
wst file <source-uuid> <path>      # the evidence for a file
wst work <uuid>                    # how a movie or series was identified, file by file
wst changeset new --author <name>  # start a batch of corrections, reviewed before they apply
wst assertions export -o <file>    # back up the corrections in effect, to import elsewhere
```

The REST API itself is documented at `/api/docs` on any running server, which
the web UI links to as "API".

You can mark any media in the web UI if it needs correction,
and that puts it into a correction queue discoverable via the UI, API, and CLI.

WST does not use LLMs. But if WST makes any mistakes, you can tell your LLM:

```
Use the `wst` CLI to detect any unidentified or misidentified media,
and make corrections until the library is completely identified.
```

## Why not just use an LLM for the entire job?

WST's heuristics can interpret your library orders of magnitude faster than an LLM,
and it only requires access to TMDB.
WST aims to be highly usable entirely without an LLM if clankers aren't your thing.
WST is accurate enough on its own that corrections should be rare.
Don't take our word for it: WST never touches your files, so pointing it at
your library is risk-free. If it gets something wrong, [tell us](#contributions).

## Contributions

Please [open an issue](https://github.com/nanosnack/WanShiTong/issues) if WST
struggles with any parts of your media library!
We will continue improving the interpretation engine with new heuristics and better tuning.

The most useful bug report is the evidence for a file it got wrong, which has
everything the heuristics saw, and the version that saw it:

```sh
wst file <data-source-uuid> "Some Movie (2001)/some.movie.2001.mkv" --json > evidence.json
curl -s http://localhost:6765/api/v1/health
```

The evidence names your files by their paths below the data source, so read
it over before posting it.

PRs are welcome if you actually understand how the code works and aren't just
dropping a slop grenade on us.

## License

Free for noncommercial use under the PolyForm Noncommercial License 1.0.0.
Commercial use requires a commercial license. See [LICENSE.md](LICENSE.md).

WST comes with a TMDB API token for noncommercial use. Commercial users must
set their own, as `tmdb_token` in `config.toml` or `WST__TMDB_TOKEN`, under
TMDB's [API terms](https://www.themoviedb.org/api-terms-of-use).
