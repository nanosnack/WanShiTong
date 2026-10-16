-- The text of an NFO file, read at ingest: a release's, e.g. with its IMDb
-- link, or a media server's or its manager's, e.g. Kodi's XML, beside a video
-- or at its series' root. NULL for every other file. Heuristics read it as a
-- fact, as they read ffprobe's measurements; nothing interprets it at ingest.
-- Read at every ingest, as an NFO is small, rather than reused with the media
-- measurements.
ALTER TABLE file_measurements ADD COLUMN nfo_text TEXT;
