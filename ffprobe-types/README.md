`ffprobe-types` is the shape of ffprobe's JSON output, as `ffprobe
-print_format json -show_format -show_streams -show_chapters -show_programs`
writes it, for serde to read. Last tested against ffprobe 8.0.1.

`FFProbeOutput`, `Format`, `Stream`, `Disposition`, `Chapter` and `Program`
mirror ffprobe's own keys, including fields WST doesn't read yet.
`wanshitong-app` reads the output from ffprobe, and `wanshitong-core` keeps what
it measures from it.

`SideData` is tagged by ffprobe's `side_data_type` names. The payload types it
carries, from `DisplayMatrix` to `MatrixEncoding`, are WST's own shapes rather
than ffprobe's keys, as are `VideoSideData` and `AudioSideData`, which sort side
data by the kind of stream it belongs to.
