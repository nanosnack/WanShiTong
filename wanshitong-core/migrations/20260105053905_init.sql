-- SYSTEM STATE
-- 
-- Single row global state.
-- 
CREATE TABLE system_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    latest_config_snapshot_id INTEGER NOT NULL,
    canonical_interpretation_id INTEGER,

    -- The first-run guide: whether it's shown, and how many of its steps are
    -- done. What the steps are is up to the application.
    onboarding_enabled INTEGER NOT NULL DEFAULT 1
        CHECK (onboarding_enabled IN (0, 1)),
    onboarding_steps_done INTEGER NOT NULL DEFAULT 0
        CHECK (onboarding_steps_done >= 0),

    FOREIGN KEY (latest_config_snapshot_id)
        REFERENCES config_snapshots(id),
    FOREIGN KEY (canonical_interpretation_id)
        REFERENCES interpretations(id)
) STRICT;

-- CONFIG SNAPSHOTS
--
-- SQL is the source of truth for config used by operations like ingest.
-- This way an ingest can safely say, "I used config snapshot #42", by loading
-- that config within a transaction.
-- 
CREATE TABLE config_snapshots (
    id INTEGER PRIMARY KEY,
    created_at INTEGER NOT NULL, -- UNIX timestamp (seconds)

    -- The config itself is a versioned JSON format.
    config TEXT NOT NULL CHECK (json_valid(config))
) STRICT;

-- DATA SOURCES
-- 
-- Where measurements are taken.
-- 
-- Two data sources may not overlap in namespace. Enforced outside of SQL.
--
-- Deleting a data source only tombstones it: it takes no part in new ingests,
-- assertions or interpretations, but past ones still refer to it, and it can be
-- restored. Only undeleted data sources need unique names.
--
CREATE TABLE data_sources (
    id INTEGER PRIMARY KEY,
    uuid BLOB NOT NULL UNIQUE,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    created_at INTEGER NOT NULL, -- UNIX timestamp
    deleted_at INTEGER -- UNIX timestamp
) STRICT;
CREATE UNIQUE INDEX idx_data_sources_undeleted_name
    ON data_sources(name) WHERE deleted_at IS NULL;
CREATE TABLE fs_data_sources (
    id INTEGER PRIMARY KEY,
    root_path TEXT NOT NULL,
    follow_symlinks INTEGER NOT NULL DEFAULT 1 CHECK (follow_symlinks IN (0, 1)),
    FOREIGN KEY (id)
        REFERENCES data_sources(id) ON DELETE CASCADE
) STRICT;

-- Help with queries based on stable UUID.
CREATE INDEX idx_data_sources_by_uuid
    ON data_sources(uuid);

-- INGESTS
--
-- An ingest measures one data source's files. Measurements are ordered by
-- which ingest they came from.
--
-- An ingest covers exactly one data source: an interpretation combines the
-- latest successful ingest of each source, so ingesting them together would
-- only let one unreadable source block cataloging the rest.
--
CREATE TABLE ingests (
    id INTEGER PRIMARY KEY,
    uuid BLOB NOT NULL UNIQUE,
    data_source_id INTEGER NOT NULL,
    config_id INTEGER NOT NULL,
    started_at INTEGER NOT NULL, -- UNIX timestamp
    files_seen INTEGER NOT NULL DEFAULT 0,
    files_probed INTEGER NOT NULL DEFAULT 0,
    files_recorded INTEGER NOT NULL DEFAULT 0,
    media_files_recorded INTEGER NOT NULL DEFAULT 0,
    -- Files it saw but couldn't measure (see `ingest_file_failures`). One
    -- file's trouble doesn't fail the ingest.
    files_failed INTEGER NOT NULL DEFAULT 0,
    stopped_at INTEGER, -- UNIX timestamp
    success INTEGER CHECK (success IN (0, 1)),

    FOREIGN KEY (data_source_id)
        REFERENCES data_sources(id) ON DELETE CASCADE,
    FOREIGN KEY (config_id)
        REFERENCES config_snapshots(id)
) STRICT;

-- Help with queries based on stable UUID.
CREATE INDEX idx_ingests_by_uuid
    ON ingests(uuid);
-- A data source's ingests, latest last.
CREATE INDEX idx_ingests_by_data_source
    ON ingests(data_source_id, id);

-- A file an ingest saw but couldn't measure: a directory it couldn't read, or
-- a file it couldn't probe. The ingest has no measurement of it.
CREATE TABLE ingest_file_failures (
    id INTEGER PRIMARY KEY,
    ingest_id INTEGER NOT NULL,
    -- Within the ingest's data source.
    rel_path TEXT NOT NULL,
    -- 'walk' if the path couldn't be listed or stat'ed, 'probe' if its
    -- contents couldn't be read.
    stage TEXT NOT NULL CHECK (stage IN ('walk', 'probe')),
    error TEXT NOT NULL,
    FOREIGN KEY (ingest_id)
        REFERENCES ingests(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX ingest_file_failures_by_ingest
    ON ingest_file_failures (ingest_id, id);

-- INTERPRETATIONS
--
-- Deriving an interpretation's catalog runs in the background: stopped_at is
-- NULL until it finishes, and error is set if it failed.
--
CREATE TABLE interpretations (
    id INTEGER PRIMARY KEY,
    uuid BLOB NOT NULL UNIQUE,
    -- Chosen by the user, to remember what sets this interpretation apart.
    name TEXT NOT NULL,
    started_at INTEGER NOT NULL, -- UNIX timestamp
    stopped_at INTEGER, -- UNIX timestamp
    error TEXT,

    -- The open changeset whose pending changes the interpretation includes,
    -- to show what accepting it would change. NULL for an interpretation of
    -- only the accepted assertions.
    changeset_id INTEGER,

    -- The build of WST that made the interpretation: a hash of its
    -- executable. An open changeset's interpretation is reused, e.g. to
    -- accept it, only by the same build, as another, e.g. after a deploy,
    -- would make another catalog of the same ingests and assertions.
    build TEXT,

    FOREIGN KEY (changeset_id)
        REFERENCES changesets(id) ON DELETE SET NULL
) STRICT;
CREATE TABLE interpretation_ingests (
    interpretation_id INTEGER NOT NULL,
    ingest_id INTEGER NOT NULL,
    -- The ingest's data source, repeated so the UNIQUE below can hold.
    data_source_id INTEGER NOT NULL,

    FOREIGN KEY (interpretation_id)
        REFERENCES interpretations(id) ON DELETE CASCADE,
    FOREIGN KEY (ingest_id)
        REFERENCES ingests(id) ON DELETE CASCADE,
    FOREIGN KEY (data_source_id)
        REFERENCES data_sources(id) ON DELETE CASCADE,

    -- Each interpretation selects exactly one ingest per data source
    UNIQUE(interpretation_id, data_source_id)
) STRICT;

-- Something an interpretation couldn't do, which it went on without: e.g. a
-- knowledge base search for a file, or a lookup of a work's details. Its
-- catalog is complete but for these.
CREATE TABLE interpretation_errors (
    id INTEGER PRIMARY KEY,
    interpretation_id INTEGER NOT NULL,
    -- The file it was about, if it was about one.
    file_id INTEGER,
    -- What was being done, e.g. "Searching the knowledge base".
    subject TEXT NOT NULL,
    error TEXT NOT NULL,
    FOREIGN KEY (interpretation_id)
        REFERENCES interpretations(id) ON DELETE CASCADE,
    FOREIGN KEY (file_id)
        REFERENCES file_measurements(id) ON DELETE SET NULL
) STRICT;

CREATE INDEX interpretation_errors_by_interpretation
    ON interpretation_errors (interpretation_id, id);

-- MEASUREMENTS
--
-- Measurements of a data source.
-- These are append-only events from which we derive our knowledge graph.
--
-- For now, measurements are only derived from data sources that have
-- filesystem-like namespaces. We assume roughly this hierarchy:
--
--     data source <-- directory <-- file <-- file media
--
-- Media containers and streams are only discovered via the use of probing tools. 
-- We persist raw probe output and attach probe tool configuration to each
-- ingest for provenance.

CREATE TABLE file_measurements (
    id INTEGER PRIMARY KEY,
    ingest_id INTEGER NOT NULL,
    -- The ingest's data source, repeated for lookups by path.
    data_source_id INTEGER NOT NULL,
    rel_path TEXT NOT NULL,

    device INTEGER, -- UNIX device number
    inode INTEGER, -- UNIX inode number

    size INTEGER NOT NULL, -- bytes
    mtime INTEGER NOT NULL, -- UNIX timestamp
    ctime INTEGER, -- UNIX timestamp
    hardlink_count INTEGER, -- stat.nlink
    is_symlink INTEGER NOT NULL CHECK (is_symlink IN (0, 1)),

    -- The media measurement to trust for this file measurement.
    -- The ingest process may determine that a file's content has not changed
    -- since the previous ingest of that data source, and in that case it may
    -- reuse the old media measurement as an optimization.
    --
    -- Because of reuse, a media measurement may be shared by file measurements
    -- from many ingests. Nothing deletes a media measurement once none of them
    -- reference it. That's harmless while nothing deletes ingests, but ingest
    -- deletion should come with a way to clean them up (e.g. an AFTER DELETE
    -- trigger on file_measurements).
    media_measurement_id INTEGER,

    UNIQUE (ingest_id, data_source_id, rel_path),
    FOREIGN KEY (ingest_id)
        REFERENCES ingests(id) ON DELETE CASCADE,
    FOREIGN KEY (data_source_id)
        REFERENCES data_sources(id) ON DELETE CASCADE,
    FOREIGN KEY (media_measurement_id)
        REFERENCES file_media_measurements(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX idx_file_measurements_by_ingest_and_source
    ON file_measurements(ingest_id, data_source_id);
CREATE INDEX idx_file_measurements_by_path
    ON file_measurements(data_source_id, rel_path);

-- Metadata about media contained in a file, extracted with ffprobe.
-- 
CREATE TABLE file_media_measurements (
    id INTEGER PRIMARY KEY,
    -- The file measurement that spawned this media measurement, or NULL once
    -- that file measurement is deleted.
    file_measurement_id INTEGER,
    mime_type TEXT,
    data TEXT NOT NULL, -- versioned JSON "FileMediaMeasurements"
    ffprobe_json BLOB, -- zstd-compressed JSON

    -- NOT ON DELETE CASCADE: newer ingests may reuse this media measurement
    -- (see file_measurements.media_measurement_id), and their file
    -- measurements cascade from it. Cascading here would mean deleting an
    -- ingest also deletes newer ingests' file measurements for unchanged files.
    FOREIGN KEY (file_measurement_id)
        REFERENCES file_measurements(id) ON DELETE SET NULL
) STRICT;

-- Discs: disc structures among an ingest's files, e.g. a ripped Blu-ray or
-- DVD, detected from its file listing when it finishes. A disc is its index
-- file (`index.bdmv` or `VIDEO_TS.IFO`), which stands for it from then on,
-- and its other files. See *Disc* in `spec/domain-model.md`.
CREATE TABLE discs (
    id INTEGER PRIMARY KEY,
    index_file_id INTEGER NOT NULL UNIQUE,
    kind TEXT NOT NULL CHECK (kind IN ('bluray', 'dvd')),
    -- The folder the disc structure is in: empty at the data source's root.
    root_path TEXT NOT NULL,
    -- All of its files', in bytes.
    size INTEGER NOT NULL,
    -- Its longest title's, if its files' durations were measured.
    duration_ms INTEGER,

    FOREIGN KEY (index_file_id)
        REFERENCES file_measurements(id) ON DELETE CASCADE
) STRICT;

-- A disc's files other than its index file, which only it stands for.
CREATE TABLE disc_files (
    disc_id INTEGER NOT NULL,
    file_id INTEGER NOT NULL UNIQUE,

    PRIMARY KEY (disc_id, file_id),
    FOREIGN KEY (disc_id)
        REFERENCES discs(id) ON DELETE CASCADE,
    FOREIGN KEY (file_id)
        REFERENCES file_measurements(id) ON DELETE CASCADE
) STRICT;

-- Helpful view that presents all measurements for a file.
-- 
CREATE VIEW complete_file_measurements AS
SELECT
    fm.*,
    ds.uuid AS data_source_uuid,
    ds.name AS data_source_name,
    fmm.mime_type,
    fmm.data as media_measurements,
    fmm.ffprobe_json,
    fmm.file_measurement_id AS media_original_file_measurement
FROM file_measurements fm
JOIN data_sources ds
  ON ds.id = fm.data_source_id
LEFT JOIN file_media_measurements fmm
  ON fmm.id = fm.media_measurement_id;

-- CATALOG
--
-- The fully-enriched view of all discovered media works based on some interpretation.
-- 

CREATE TABLE movie_files (
    id INTEGER PRIMARY KEY,
    movie_id INTEGER NOT NULL,
    file_id INTEGER NOT NULL,
    -- 'main', 'extra' or 'disc'. NULL when nothing says what the file's role
    -- is, e.g. a video that is the movie but might not be its main file. A
    -- 'disc' file is part of the movie's disc (see `discs`), e.g. a stream or
    -- a playlist, whose index file is the movie's main file.
    role TEXT CHECK (role IN ('main', 'extra', 'disc')),

    -- The expression a main file's manifestation realizes (see
    -- `movie_expressions`). NULL for the movie's other files, e.g. its
    -- extras, which belong to the movie, not to one cut of it.
    expression_id INTEGER,

    -- Which manifestation of its movie a main file is, and which part of
    -- that manifestation, are recorded with the catalog rather than derived
    -- from the file's root (see *Manifestation* in spec/domain-model.md). A
    -- main file is a manifestation of its own unless its name numbers it as
    -- a part of one, e.g. `CD1`, beside another part of it.
    --
    -- The main file's manifestation, numbered from 0 in the order of the
    -- manifestations' first files. NULL for the movie's other files.
    manifestation INTEGER,

    -- Which part of its manifestation the main file is, if its name numbers
    -- it as one, e.g. 1 for `CD1`. NULL for a manifestation of one file, and
    -- for other files.
    part INTEGER,

    FOREIGN KEY (movie_id)
        REFERENCES movies(id) ON DELETE CASCADE,
    FOREIGN KEY (file_id)
        REFERENCES file_measurements(id) ON DELETE CASCADE,
    FOREIGN KEY (expression_id)
        REFERENCES movie_expressions(id) ON DELETE CASCADE
) STRICT;

-- Every interpretation keeps its catalog, so these tables grow with each one.
-- Without indexes by work and by file, a work's files and a file's works were
-- found by scanning every interpretation's rows, as were the foreign-key
-- cascades from movies, movie_expressions and file_measurements.
CREATE INDEX movie_files_by_movie
    ON movie_files (movie_id);
CREATE INDEX movie_files_by_file
    ON movie_files (file_id);
CREATE INDEX movie_files_by_expression
    ON movie_files (expression_id);

-- A file can hold several episodes, e.g. a double episode, and an episode can
-- be in several files, e.g. copies of it.
CREATE TABLE tv_episode_files (
    id INTEGER PRIMARY KEY,
    episode_id INTEGER NOT NULL,
    file_id INTEGER NOT NULL,

    FOREIGN KEY (episode_id)
        REFERENCES tv_episodes(id) ON DELETE CASCADE,
    FOREIGN KEY (file_id)
        REFERENCES file_measurements(id) ON DELETE CASCADE,
    UNIQUE (episode_id, file_id)
) STRICT;

-- The UNIQUE constraint leads with the episode, so it only serves lookups
-- from that side.
CREATE INDEX tv_episode_files_by_file
    ON tv_episode_files (file_id);

CREATE TABLE movies (
    id INTEGER PRIMARY KEY,
    uuid BLOB NOT NULL UNIQUE,
    interpretation_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    tvdb_id INTEGER,
    tmdb_id INTEGER,
    imdb_id TEXT,

    original_title TEXT,
    original_language TEXT,
    tagline TEXT,
    overview TEXT,
    release_year INTEGER,
    release_month INTEGER,
    release_day INTEGER,
    runtime_seconds INTEGER, -- Not necessarily as accurate as the video file

    tmdb_poster_path TEXT,

    -- Movies are listed in order of their titles without a leading English
    -- article, so "The Matrix" sorts among the Ms and "A Quiet Place" among
    -- the Qs. The key is recorded with the catalog rather than generated from
    -- the title, since SQLite can't fold case beyond ASCII, drop accents or
    -- order numbers by their values (see sort_key.rs).
    sort_title TEXT NOT NULL,

    FOREIGN KEY (interpretation_id)
        REFERENCES interpretations(id) ON DELETE CASCADE,
    UNIQUE (interpretation_id, tvdb_id),
    UNIQUE (interpretation_id, tmdb_id),
    UNIQUE (interpretation_id, imdb_id)
) STRICT;

CREATE INDEX movies_interpretation_sort_title
    ON movies (interpretation_id, sort_title, id);

-- A movie's expressions: the cuts its manifestations realize (see
-- *Expression* in spec/domain-model.md). Each is named by its edition, e.g.
-- 'Director''s Cut', or NULL for the unspecified expression, which is no
-- particular cut.
CREATE TABLE movie_expressions (
    id INTEGER PRIMARY KEY,
    movie_id INTEGER NOT NULL,
    edition TEXT CHECK (edition <> ''),

    FOREIGN KEY (movie_id)
        REFERENCES movies(id) ON DELETE CASCADE
) STRICT;

-- One expression per edition, and one unspecified expression, per movie.
CREATE UNIQUE INDEX movie_expressions_by_edition
    ON movie_expressions (movie_id, COALESCE(edition, ''));

CREATE TABLE tv_series (
    id INTEGER PRIMARY KEY,
    uuid BLOB NOT NULL UNIQUE,
    interpretation_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    original_name TEXT,
    tvdb_id INTEGER,
    tmdb_id INTEGER,
    imdb_id TEXT,

    original_language TEXT,
    tagline TEXT,
    overview TEXT,
    first_air_year INTEGER,
    first_air_month INTEGER,
    first_air_day INTEGER,

    -- Nothing fetches series posters yet.
    tmdb_poster_path TEXT,

    -- Sorted like movies' titles: recorded with the catalog, without a
    -- leading English article (see sort_key.rs).
    sort_name TEXT NOT NULL,

    FOREIGN KEY (interpretation_id)
        REFERENCES interpretations(id) ON DELETE CASCADE,
    UNIQUE (interpretation_id, tvdb_id),
    UNIQUE (interpretation_id, tmdb_id),
    UNIQUE (interpretation_id, imdb_id)
) STRICT;

CREATE INDEX tv_series_interpretation_sort_name
    ON tv_series (interpretation_id, sort_name, id);

-- A series' season, or with a NULL number, its episodes that nothing says the
-- season of. Those are numbered through the whole series, e.g. One Piece's
-- episode 92, rather than within a season.
CREATE TABLE tv_seasons (
    id INTEGER PRIMARY KEY,
    series_id INTEGER NOT NULL,
    number INTEGER,
    name TEXT,

    FOREIGN KEY (series_id)
        REFERENCES tv_series(id) ON DELETE CASCADE,
    UNIQUE (series_id, number)
) STRICT;

-- UNIQUE treats NULLs as distinct, so this keeps a series' episodes of an
-- unknown season together.
CREATE UNIQUE INDEX tv_seasons_unknown
    ON tv_seasons (series_id) WHERE number IS NULL;

CREATE TABLE tv_episodes (
    id INTEGER PRIMARY KEY,
    season_id INTEGER NOT NULL,
    number INTEGER NOT NULL,
    title TEXT,

    FOREIGN KEY (season_id)
        REFERENCES tv_seasons(id) ON DELETE CASCADE,
    UNIQUE (season_id, number)
) STRICT;

-- Files that belong to a series without being one of its episodes, e.g.
-- creditless openings and endings, or specials without episode numbers.
--
-- TODO: an extra is a work, like an episode, so files should hang off a
-- tv_series_extras table (one row per extra of a series) rather than off the
-- series, as tv_episode_files hang off tv_episodes. That's what identifying
-- extras needs, e.g. to know that two files are copies of the same opening.
-- Until something identifies extras, each file would be its own extra, so
-- files link to the series directly.
--
CREATE TABLE tv_series_extra_files (
    id INTEGER PRIMARY KEY,
    series_id INTEGER NOT NULL,
    file_id INTEGER NOT NULL,
    -- Whether the file is a disc's (see `discs`) kept with the series, like
    -- an extra, but not one of its episodes, e.g. a season's Blu-ray, whose
    -- episodes aren't cataloged separately.
    disc INTEGER NOT NULL DEFAULT 0 CHECK (disc IN (0, 1)),

    FOREIGN KEY (series_id)
        REFERENCES tv_series(id) ON DELETE CASCADE,
    FOREIGN KEY (file_id)
        REFERENCES file_measurements(id) ON DELETE CASCADE,
    UNIQUE (series_id, file_id)
) STRICT;

-- The UNIQUE constraint leads with the series, so it only serves lookups
-- from that side.
CREATE INDEX tv_series_extra_files_by_file
    ON tv_series_extra_files (file_id);

-- The files an interpretation judged to be videos, with moving pictures,
-- identified or not, so that the files page can tell videos from other files
-- without repeating that judgment.
CREATE TABLE video_files (
    interpretation_id INTEGER NOT NULL,
    file_id INTEGER NOT NULL,

    PRIMARY KEY (interpretation_id, file_id),
    FOREIGN KEY (interpretation_id)
        REFERENCES interpretations(id) ON DELETE CASCADE,
    FOREIGN KEY (file_id)
        REFERENCES file_measurements(id) ON DELETE CASCADE
) STRICT;

-- The video files that no identified work in an interpretation's catalog
-- includes: what's left to identify. "Video" is the interpretation's judgment
-- of the file's measurements, e.g. that a scan's one video stream is a still
-- image, so it's recorded with the catalog rather than derived by queries.
CREATE TABLE unidentified_videos (
    interpretation_id INTEGER NOT NULL,
    file_id INTEGER NOT NULL,

    PRIMARY KEY (interpretation_id, file_id),
    FOREIGN KEY (interpretation_id)
        REFERENCES interpretations(id) ON DELETE CASCADE,
    FOREIGN KEY (file_id)
        REFERENCES file_measurements(id) ON DELETE CASCADE
) STRICT;

-- The files of each interpretation's identified works: those with an
-- external ID, which says which work it is.
CREATE VIEW identified_files AS
SELECT mo.interpretation_id, mf.file_id
FROM movie_files mf
JOIN movies mo
    ON mo.id = mf.movie_id
WHERE mo.tmdb_id IS NOT NULL OR mo.imdb_id IS NOT NULL OR mo.tvdb_id IS NOT NULL
UNION
SELECT s.interpretation_id, ef.file_id
FROM tv_episode_files ef
JOIN tv_episodes e
    ON e.id = ef.episode_id
JOIN tv_seasons se
    ON se.id = e.season_id
JOIN tv_series s
    ON s.id = se.series_id
WHERE s.tmdb_id IS NOT NULL OR s.imdb_id IS NOT NULL OR s.tvdb_id IS NOT NULL
UNION
SELECT s.interpretation_id, xf.file_id
FROM tv_series_extra_files xf
JOIN tv_series s
    ON s.id = xf.series_id
WHERE s.tmdb_id IS NOT NULL OR s.imdb_id IS NOT NULL OR s.tvdb_id IS NOT NULL;

-- Each movie's and series' files, by the work's UUID: a movie's files, and a
-- series' episodes' and extras' files.
CREATE VIEW work_files AS
SELECT mo.uuid AS work_uuid, mf.file_id
FROM movie_files mf
JOIN movies mo
    ON mo.id = mf.movie_id
UNION
SELECT s.uuid, ef.file_id
FROM tv_episode_files ef
JOIN tv_episodes e
    ON e.id = ef.episode_id
JOIN tv_seasons se
    ON se.id = e.season_id
JOIN tv_series s
    ON s.id = se.series_id
UNION
SELECT s.uuid, xf.file_id
FROM tv_series_extra_files xf
JOIN tv_series s
    ON s.id = xf.series_id;

-- NOTE: we don't create "God" VIEWs for the catalog because they would be very
-- JOIN heavy, and sqlite does not yet implement JOIN elimination in its query
-- planner.

-- AUDIT LOGS
--
-- Records knowledge base queries and claims for each (interpretation, file) pair.
-- Used for debugging and understanding how claims were derived.
--
CREATE TABLE audit_logs (
    id INTEGER PRIMARY KEY,
    interpretation_id INTEGER NOT NULL,
    file_id INTEGER NOT NULL,
    data TEXT NOT NULL, -- versioned JSON "SubjectAuditLog"
    FOREIGN KEY (interpretation_id) REFERENCES interpretations(id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES file_measurements(id),
    UNIQUE (interpretation_id, file_id)
) STRICT;

-- ASSERTIONS
--
-- An assertion is a user-authored claim about a file. It names the file by
-- (data source, path) rather than by measurement, so it applies to that file
-- in every ingest.
--
-- Assertions are never edited or deleted, so interpretations keep the
-- assertions they were derived from. A newer assertion of the same kind about
-- the same file retracts the older one, and removing an assertion retracts
-- it. Only unretracted assertions go into new interpretations.
--
-- An assertion is one claim about a set of files: correcting a movie's
-- identity asserts it about each of the movie's files. Each row of assertions
-- is that claim about one of the files, and its group is the assertion as a
-- whole, which is shown and removed as one. Removing it from only some of a
-- movie's files would split the movie.
--
-- A group's files are retracted one by one when newer assertions of the same
-- kind replace them, so a group is retracted once all of its files are.
--
CREATE TABLE assertion_groups (
    id INTEGER PRIMARY KEY,
    uuid BLOB NOT NULL UNIQUE,
    created_at INTEGER NOT NULL, -- UNIX timestamp
    -- The changeset it was made in.
    changeset_id INTEGER NOT NULL,
    -- Why it was made.
    rationale TEXT,
    -- How sure its author was, from 0 to 1, for ordering reviews. It never
    -- affects belief.
    confidence REAL,

    FOREIGN KEY (changeset_id)
        REFERENCES changesets(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX idx_assertion_groups_by_changeset
    ON assertion_groups(changeset_id);

CREATE TABLE assertions (
    id INTEGER PRIMARY KEY,
    uuid BLOB NOT NULL UNIQUE,
    group_id INTEGER NOT NULL,
    data_source_id INTEGER NOT NULL,
    rel_path TEXT NOT NULL,
    claim TEXT NOT NULL, -- versioned JSON "ClaimPredicate"
    created_at INTEGER NOT NULL, -- UNIX timestamp
    retracted_at INTEGER, -- UNIX timestamp

    FOREIGN KEY (group_id)
        REFERENCES assertion_groups(id) ON DELETE CASCADE,
    FOREIGN KEY (data_source_id)
        REFERENCES data_sources(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX idx_assertions_by_path
    ON assertions(data_source_id, rel_path);

CREATE INDEX idx_assertions_by_group
    ON assertions(group_id);

-- The assertions an interpretation was derived from.
CREATE TABLE interpretation_assertions (
    interpretation_id INTEGER NOT NULL,
    assertion_id INTEGER NOT NULL,

    PRIMARY KEY (interpretation_id, assertion_id),
    FOREIGN KEY (interpretation_id)
        REFERENCES interpretations(id) ON DELETE CASCADE,
    FOREIGN KEY (assertion_id)
        REFERENCES assertions(id) ON DELETE CASCADE
) STRICT;

-- CHANGESETS
--
-- Every assertion, and every retraction of one, is made in a changeset: the
-- changes made together, e.g. one correction in the web UI or one agent run.
-- While a changeset is open, its changes are pending: new interpretations
-- leave them out, but for the changeset's own. Accepting it makes its
-- assertions active and its retractions take effect, and a newer assertion of
-- the same kind about the same file then retracts the older one. Discarding it
-- drops its pending changes, and keeps the changeset as history.
--
CREATE TABLE changesets (
    id INTEGER PRIMARY KEY,
    uuid BLOB NOT NULL UNIQUE,
    -- Who made it, e.g. "user", or an agent run's name.
    author TEXT NOT NULL,
    -- What it's for, e.g. what an agent run set out to do.
    description TEXT NOT NULL,
    created_at INTEGER NOT NULL, -- UNIX timestamp
    state TEXT NOT NULL CHECK (state IN ('open', 'accepted', 'discarded')),
    -- When it was accepted or discarded. UNIX timestamp.
    closed_at INTEGER,

    CHECK ((state = 'open') = (closed_at IS NULL))
) STRICT;

-- The accepted assertions a changeset retracts once it's accepted. Retracting
-- one of its own pending assertions just withdraws it: its files' rows are
-- marked retracted.
CREATE TABLE changeset_retractions (
    changeset_id INTEGER NOT NULL,
    assertion_group_id INTEGER NOT NULL,

    PRIMARY KEY (changeset_id, assertion_group_id),
    FOREIGN KEY (changeset_id)
        REFERENCES changesets(id) ON DELETE CASCADE,
    FOREIGN KEY (assertion_group_id)
        REFERENCES assertion_groups(id) ON DELETE CASCADE
) STRICT;

-- MARKS
--
-- Notes that files need reviewing, made while browsing, to correct later.
-- They're review state, not claims, and never change the catalog. Like
-- assertions, they name files by data source and path, so they survive
-- ingests and reinterpretation. A mark is done once its files' corrections
-- are accepted: see list_marks.
--
CREATE TABLE marks (
    id INTEGER PRIMARY KEY,
    uuid BLOB NOT NULL UNIQUE,
    -- What was marked, for display, e.g. "movie Submarine D-1".
    label TEXT NOT NULL,
    -- For whoever reviews it, e.g. "this is a TV series, not a movie".
    note TEXT NOT NULL,
    created_at INTEGER NOT NULL -- UNIX timestamp
) STRICT;

CREATE TABLE mark_files (
    mark_id INTEGER NOT NULL,
    data_source_id INTEGER NOT NULL,
    rel_path TEXT NOT NULL,

    PRIMARY KEY (mark_id, data_source_id, rel_path),
    FOREIGN KEY (mark_id)
        REFERENCES marks(id) ON DELETE CASCADE,
    FOREIGN KEY (data_source_id)
        REFERENCES data_sources(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX idx_mark_files_by_path
    ON mark_files(data_source_id, rel_path);

-- VIEWS
--
-- A view materializes the canonical catalog for an external consumer, e.g. a
-- tree of symlinks named for Jellyfin. Views are derived artifacts: they add no
-- knowledge and can be rebuilt at any time.
--
-- A view may not overlap the namespace of any data source. Enforced outside of
-- SQL.
--
CREATE TABLE views (
    id INTEGER PRIMARY KEY,
    uuid BLOB NOT NULL UNIQUE,
    name TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL,
    created_at INTEGER NOT NULL, -- UNIX timestamp
    -- Whether it's refreshed whenever another interpretation becomes
    -- canonical.
    auto_refresh INTEGER NOT NULL DEFAULT 1
        CHECK (auto_refresh IN (0, 1))
) STRICT;
CREATE TABLE jellyfin_views (
    id INTEGER PRIMARY KEY,
    root_path TEXT NOT NULL UNIQUE,
    FOREIGN KEY (id)
        REFERENCES views(id) ON DELETE CASCADE
) STRICT;
-- A tree of symlinks named for Plex, as jellyfin_views is for Jellyfin.
CREATE TABLE plex_views (
    id INTEGER PRIMARY KEY,
    root_path TEXT NOT NULL UNIQUE,
    FOREIGN KEY (id)
        REFERENCES views(id) ON DELETE CASCADE
) STRICT;

-- Every attempt to bring a view up to date with an interpretation. The latest
-- row is the view's current state.
--
CREATE TABLE view_materializations (
    id INTEGER PRIMARY KEY,
    view_id INTEGER NOT NULL,
    interpretation_id INTEGER,
    started_at INTEGER NOT NULL, -- UNIX timestamp
    stopped_at INTEGER NOT NULL, -- UNIX timestamp
    success INTEGER NOT NULL CHECK (success IN (0, 1)),

    -- Set on success.
    links_created INTEGER,
    links_updated INTEGER,
    links_removed INTEGER,

    -- Set on failure.
    error TEXT,

    FOREIGN KEY (view_id)
        REFERENCES views(id) ON DELETE CASCADE,
    FOREIGN KEY (interpretation_id)
        REFERENCES interpretations(id) ON DELETE SET NULL
) STRICT;

CREATE INDEX idx_view_materializations_by_view
    ON view_materializations(view_id);
-- Deleting an interpretation unsets it on its materializations.
CREATE INDEX idx_view_materializations_by_interpretation
    ON view_materializations (interpretation_id);
