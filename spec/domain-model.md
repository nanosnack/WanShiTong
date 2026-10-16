# Domain Model

This document defines the **core concepts** used throughout WST.
All code, schemas, and APIs must align with these definitions.

## Overview and Catalog

The ultimate goal of this model is to create a high-level *catalog* of a media collection,
based on *measurements* made from file data and external knowledge databases.
This catalog should present as a collection of *media works*,
e.g. movies, TV shows, books, etc.
The user should be able to browse this catalog and drill down into specific components
of a media work, e.g. seasons, parts, episodes, files, and even components within file formats,
like media containers and streams.
Not only do we provide this hierarchical catalog;
we treat the catalog as a purely functional *interpretation* of measurements.
The interpretation parameters are treated as data:
*heuristics* that derive *claims* from measurements,
*belief policies* about which heuristics to trust, etc.
We allow the user to create multiple interpretations,
browse hypothetical catalogs based on those interpretations,
and choose which one should be *canonical*.

The following glossary of domain concepts starts with familiar concrete ideas
then drills down into the more mechanical, WST-specific concepts.

---

# View

A view is a materialized projection of an interpretation’s resolved knowledge,
intended for consumption by external systems or users.

Views are derived artifacts:
they are computed from an interpretation’s catalog and do not introduce new knowledge.
They may be created, deleted, or rebuilt at any time without loss of information.

Examples:

- A filesystem tree of symlinks organized for a media server
- A flattened directory of movies grouped by genre
- A read-only export for a third-party application

Properties:

- View type (e.g. filesystem, export)
- Associated interpretation
- Configuration specific to the view type
- Materialization state (e.g. up to date, stale)

In the MVP, the primary view type is a filesystem view implemented as a synchronized
tree of symlinks suitable for consumption by media servers such as Jellyfin and Plex,
each view laid out and named by one media server's rules.

Views include every work in the catalog, i.e. every identified work.
A work with only a user ID reaches a view with its asserted title but no knowledge base ID.

A view links every manifestation of a movie (see *Manifestation*), not just one,
when its consumer can show several, e.g. as Jellyfin's or Plex's versions of a movie.
A manifestation's link is named by its expression's edition, e.g. `Brazil (1985) - Director's Cut.mkv`,
whether or not the movie has other manifestations,
so that adding or removing a manifestation doesn't rename the others' links,
which would lose what a media server keeps by path, e.g. watch state.
A manifestation of the unspecified expression is named after the movie alone.
Manifestations of the same expression are told apart by a number after it, in the order of their paths.
A manifestation of several parts (see *Manifestation*) is linked as its consumer's parts of one version,
each named by its part's number, e.g. `Brazil (1985) - part1.mkv` and `- part2.mkv`,
so that the whole movie plays.

Likewise a view links every file of an episode, as its consumer's versions of the episode,
told apart by a number after the episode's code, e.g. `Show S01E03.mkv` and `Show S01E03 - 2.mkv`.
A consumer takes the files that begin on one episode for versions of one entry, whichever episode each ends on,
e.g. Jellyfin, and shows the entry as its first version by name.
So the files beginning on an episode are all named by the longest range among them,
the longest first and then by path, e.g. `E450-E451` beside `E450` are `Show S01E450-E451.mkv` and `Show S01E450-E451 - 2.mkv`:
the whole range plays by default, and the shorter file, which holds nothing the longer doesn't, is still there to choose.
A file within another's range that begins later, e.g. `E450` beside `E447-E450`, is an entry of its own.

---

## Media Work

A *media work* represents a cataloged creative work (e.g. a film, television series, album, or book)
as inferred from one or more measurements.

Below is a disambiguation of terms related to the concept of a media work.
This hierarchy is inspired by FRBR but adapted pragmatically for media files.

### Media Work

A *media work* represents the abstract creative work itself (e.g. *Blade* (1998)),
independent of any particular cut, language, or encoding.

### Expression

An *expression* represents a specific realization of a media work,
such as a director’s cut, extended edition, or language-specific version.

An expression is named by its *edition*, e.g. `Director's Cut`, `Extended` or `Theatrical`.
A work's catalog entry has one expression per edition its manifestations realize,
and an *unspecified* expression for manifestations that nothing names an edition for.
The unspecified expression is not any particular cut:
a manifestation named only `Alien (1979)` may be the theatrical cut or not,
so it isn't assumed to be `Theatrical`.

Interpretation decides a manifestation's expression from the *edition* claims about its main files,
which its release name, a knowledge base or an assertion makes (see *Claim*).
Files are the only subjects claims are made about,
so an edition claim on a file is evidence about the manifestation the file is a main file of,
not a property of the file:
the extras of a director's cut release are not themselves a director's cut.
A manifestation whose main files claim no edition, or claim different ones,
realizes the unspecified expression.

An edition says which cut a manifestation is, not which work:
a director's cut is a manifestation of the same work as the theatrical release,
and its files are still that manifestation's main files.

Only movies' expressions are cataloged for now.
A TV series' episodes all realize its unspecified expression.

### Manifestation

A *manifestation* represents a concrete encoding or release of an expression,
such as a particular video encode, container format, or bitrate.

A manifestation can span many files, e.g. a movie in two parts, `CD1` and `CD2`.
A *disc* (see *Measurement*) is one manifestation, and an indivisible one:
its index file stands for the whole disc.

A movie's manifestations are its main files:
each main file is a manifestation of its own, unless something says two are one.
The same file reached by two paths (the same device and inode) is one manifestation.
So are the *parts* of one manifestation: main files in one directory
whose names number them as different parts of their work, e.g. `CD1` and `CD2` or `Part 1` and `Part 2`
(a part is a claim, which an assertion can make or correct; see *Claim*).
Of several parts numbered alike beside each other, e.g. two `CD1`s,
each goes with the first manifestation, by its files' paths, that has no part of that number yet.
Nothing else joins main files into one manifestation: not sharing a directory, nor a release,
as two encodes of a movie are often kept side by side,
and a manifestation that is really two would hide one from a consumer that shows versions.
WST doesn't assume what a directory is, e.g. a movie's release, a collection, or a series' release.
Manifestations are numbered in the order of their first files' paths, so the numbering is the same
whatever order the claims about the files are found in.

An episode's manifestations are its files: each file of an episode is a manifestation of it,
and a file of several episodes is one manifestation of each.
Two files numbered as the same episode are two manifestations of it, wherever they are,
as two encodes of a movie are: nothing in the catalog says one of them is wrong.

A manifestation belongs to exactly one of its work's expressions.
Which manifestation a consumer should prefer, e.g. the highest resolution, is a policy of its own,
not part of the catalog.

### File

A *file* represents a physical or logical file discovered by *ingest*,
which may be evidence for one or more manifestations.

### Identification

The catalog contains only *identified* works.
A work is identified when it has an *external ID*, which says which work it is:

- an ID from a knowledge base, e.g. TMDB, IMDb or TVDB
- a *user ID*, minted by a user's assertion for a work that no knowledge base has,
  e.g. a home video or an obscure release (see *Assertion*)

Files that no identified work includes are *unplaced*.
They are not in the catalog, but their claims are kept in audit logs,
so nothing learned about them is lost,
and an assertion that identifies one file can bring its extras or episodes along with it.

Heuristics can suggest what an unplaced file is, e.g. a movie title parsed from its name,
and those suggestions drive knowledge base searches.
But a suggestion never places a file in the catalog by itself.
Deciding *what kind* of work a file is (its *classification*)
without deciding *which* work it is (its identification) buys little,
and there is no clear line for how much evidence a classification would need.
So identification, by a knowledge base or a user, is what makes a work.

---

## Data Source

A Data Source represents an origin of raw file data.
All *measurements* taken during *ingest* are linked to a specific data source.

Examples:
- A local directory tree
- A directory tree accessed over HTTP

Properties:
- Stable ID
- Human-readable name
- Configuration (root path, follow symlinks)

A data source is passive; it does not imply ingestion has occurred.

WST assumes nothing about how a data source is organized:
its releases may sit directly under its root, be grouped by genre or by kind, or be nested at any depth,
and a release may be a movie's, a collection's, or a series'.
Heuristics go by the structure *within* a release, which its publisher chose and its names describe:
the file names, a season's or an extras directory, a disc's folder, numbered parts or episodes beside each other.
They never go by where a release sits in the data source, e.g. its depth or the directory that groups it.
A file's top directory, the directory directly below the data source's root that holds it,
is shown as context when auditing, not relied on.

Deleting a data source only tombstones it. It takes no part in new ingests,
assertions or interpretations, and new interpretations leave out its
assertions. Past ingests and interpretations still refer to it, so the
canonical catalog includes it until a newer interpretation replaces it. A
deleted data source can be restored, with its ingests and assertions, unless
another data source has taken its name or a view has taken its place.

---

## Ingest

Ingest is a process that runs on a set of data sources
to extract *measurements* about the contained directories, files, and media.
Every measurement is linked to a single ingest.
The measurements are stored in SQL so that users can
tweak their *heuristics* and run *interpretation* many times
without needing to do more expensive ingests.

---

## Measurement

A *measurement* is an append-only event about a *subject* produced by ingestion.
Measurements are not in dispute; we assume they are true (at that point in time),
because we extract them directly from either file system metadata
or metadata found in file formats via tools like `ffprobe`.

Examples:
- "File exists at path X"
- "This MP4 container has one video stream and two audio streams"
- "This video stream uses the H.264 codec"
- "This NFO file's text is `<movie><title>Heat</title>...`"

An NFO's text is a measurement, not what it says: that a release's link or a media server's
`<uniqueid>` names the work is a *claim*, made by a *heuristic* that reads the text,
so that what an NFO is trusted with can change without another ingest.

Properties:
- Data Source ID
- Ingest ID
- Path
- Many more fields collected via file content probing

Measurements are never mutated and only rarely deleted (when orphaned).

### Disc

A *disc* is a disc structure among an ingest's files, e.g. a ripped Blu-ray or DVD:

- a *Blu-ray*: a folder holding `BDMV/index.bdmv`, with at least one stream in `BDMV/STREAM`
- a *DVD*: a folder holding `VIDEO_TS.IFO`, with at least one title set (`VTS_*.VOB`) beside it,
  usually in a folder named `VIDEO_TS`, though it is sometimes renamed

Ingest detects discs from the file listing and records each as a measurement:
its kind, its folder, its *index file* (`index.bdmv` or `VIDEO_TS.IFO`), and its other files.
Detection is strict, as nothing can override it:
a partial structure, e.g. a `BDMV` folder without an index or without streams, is not a disc,
and its files are ordinary files.

A *disc image*, e.g. an `.iso`, is a disc of its own: the image is its index file and its folder,
and it has no other files.
Ingest knows one by its content, not its name: an ISO 9660 or UDF filesystem
that ffprobe reads as a Blu-ray through libbluray, whose measurements are then its longest title's.
DVD images aren't read yet, as they need an ffprobe with libdvdnav,
so they stay ordinary files.

A disc is a fact, not a claim, and it is indivisible everywhere after ingest:

- Interpretation treats it as one subject, its index file, with the disc's measurements,
  e.g. its duration, which is its longest title's, e.g. a Blu-ray's longest stream.
  Its other files are not subjects of their own: no heuristic, knowledge base or assertion
  makes claims about them.
- The catalog places it as a whole, as its index file:
  a movie's main file or extra, a series' disc, or excluded.
  Its other files go wherever it goes.
- Assertions name it by its index file (see *Assertion*).
- Views link it as a disc, e.g. its `BDMV` folder, rather than its files,
  or leave it out for a media server that can't play discs, e.g. Plex.
- An unidentified disc is one unidentified video, not one per stream.

So a disc's titles are never cataloged separately,
e.g. the episodes on a TV series' disc are not episodes of their own:
the disc as a whole is placed, as a media player plays it.
A series' disc is kept with the series, like an extra, but is not one of its episodes,
so views that list episodes leave it out.

---

## Interpretation

An *interpretation* is everything required to create a catalog, except the measurements.
Mathematically, `catalog = f(measurements, interpretation)` and `f` is a pure function.

Concretely, an interpretation includes:
- heuristics
- belief policy (a lattice or metric over the heuristics)
- claim sources, e.g. external knowledge databases like TVDB
- ingest set: the set of ingests (IDs) under consideration
  - Exactly one ingest per data source is selected.
  - More than one ingest per data source would create potential for inconsistency.

Knowledge bases are the exception to purity:
an interpretation names a knowledge base, not a snapshot of its contents.
Responses are cached and reused, so interpretations sharing a cache see the same answers,
but a new query or a lost cache can see different data.
Audit logs record the knowledge base queries and results that each interpretation used,
so a catalog stays explainable even where it is not exactly reproducible.

Interpretations are immutable, versioned configurations identified by content, not name.

### Canonical Interpretation

When we say that a catalog is *canonical*,
we mean that it was produced by the *canonical interpretation*.
Users may create multiple interpretations and compare them to decide which one should be canonical.
The canonical catalog is the one which users see by default
if they are not testing out new interpretations.

### Interpretations of the accepted assertions, and of a changeset

An interpretation's assertions are a snapshot, taken when it is created.
There are two kinds of interpretation, by which assertions they include:

- An interpretation of the *accepted assertions* includes every assertion in effect,
  i.e. of an accepted changeset and not since retracted or replaced.
  Ingests, new interpretations in the web UI, and accepted changesets create these.
- A *changeset's interpretation* also includes what accepting one open changeset would change:
  its pending assertions, but not the accepted ones it retracts or replaces.
  It is created on demand, to preview the changeset (see *Changesets*).

What a changeset's interpretation amounts to follows its changeset:
while the changeset is open, or once it is discarded, it previews changes that haven't happened;
once it is accepted, it is what accepting made, if it was still up to date then.

Only an interpretation of changes that happened can be canonical:
one of the accepted assertions, or one of an accepted changeset.
An open or discarded changeset's interpretation can't be,
as making it canonical would apply the changeset without accepting it.
Accepting a changeset makes its interpretation canonical, if it is up to date.

---

## Fact

A *fact* is just a measurement within the context of some interpretation.
Facts are a projection of measurements, not stored separately.
Facts are chosen by selecting only measurements that come from
the *ingest set* of an interpretation.

---

## Claim

A *claim* is any piece of knowledge used by an interpretation that is not a *fact*.
This includes statements like:
- "This file is a manifestation of episode 3 of season 1 of Friends"
- "The show Friends began airing in 1994"

Notably, claims are distinct from measurements and facts.
Even knowledge that comes from an external database like TVDB are not considered facts,
as these databases are always changing and cannot be considered authoritative.

---

## Subject

A subject is probably the most abstract concept in our domain.
We produce measurements *about* subjects.
We also produce claims *about* subjects.

Subjects of measurements can be things like:
- a directory
- a file
- a disc (see *Disc*)
- a media container
- a video stream
- an audio stream
- a text stream

Subjects of claims can be things like:
- a movie
- the director's cut of a movie
- a tv show season
- a set of file measurements

Under the hood, we make some distinctions between subjects of measurements and subjects of claims,
but the idea is mostly the same.
Measurement subjects are fully enumerated and strictly typed in our schema.
Claim subjects, however, need to be more flexible.
For example, a claim subject may need to refer to an arbitrary set of other subjects.

---

## Heuristic

A *heuristic* is a process by which a claim is derived from facts.
Next to external knowledge bases, heuristics are the main producer of claims.
The canonical example of a heuristic is a regular expression
that attempts to parse "S##E##" from a video file name.

---

## Assertion

An *assertion* is a user-authored claim.

Examples:
- “The title of entity X is Y”
- “File A belongs to entity B”

Assertions:
- Always have higher priority than heuristics
- Are append-only
- May supersede earlier assertions
- Are made in a changeset

Assertions are the primary correction mechanism.
Assertions must survive reinterpretation,
so once accepted (see *Changesets*) they are included in every new interpretation.

An assertion names its files by data source and path.
It names a disc by its index file, and never names a disc's other files:
an assertion that does is refused, saying which disc the file is part of.
Should a later ingest find that a file an accepted assertion names has become part of a disc,
the assertion stops applying to that file, as interpretation sees only the disc.

### Changesets

Every assertion, and every retraction of one, is made in a *changeset*:
a set of changes made together, e.g. one correction in the web UI, or one agent run,
with its provenance (who made it, and why).

A changeset is *open* while its changes are being made, and then *accepted* or *discarded*:

- While open, its changes are pending.
  New interpretations leave them out, except the changeset's own (see below).
  Changes can be added and taken back freely; nothing is interpreted until asked for.
- Accepting it makes its assertions active and its retractions take effect,
  so every new interpretation includes them, and a new canonical interpretation applies them.
  An assertion it makes replaces any older one of the same kind about the same file.
- Discarding it drops its pending changes. The changeset stays, as history.

An open changeset's *interpretation* is one that includes the accepted assertions
and the changeset's pending changes.
It is created on demand, when something asks what the changeset would do,
and reused until the changeset or the ingests change.
Comparing it with the canonical catalog shows what accepting the changeset would change.

A correction in the web UI is a changeset of one change that is accepted at once.
An agent's run is a changeset that is reviewed, by comparing catalogs, before it is accepted.

### User IDs

A user identifies a work that no knowledge base has by asserting a *user ID* for its files:
an external ID that WST mints, in a namespace of its own, instead of one from a knowledge base.
It works like any other external ID:

- Files with the same user ID are one work, e.g. two files of one home video.
- The predicate says the work's kind, e.g. a movie's or a TV series' external ID.
- There is no knowledge base to look it up in,
  so its title, year and the like are asserted alongside it.

A user ID is identification by a user.
Heuristics never mint one, so a heuristic's guess never becomes a work.

---

## Belief Policy

*Belief policy* is a metric or lattice over the set of claims in an interpretation.
As such, the policy is a projection of claims which resolves conflicts between claims.
The output of this policy is an internally consistent set of claims
which contains all information required to create a catalog.

Conflicts are between claims about one subject, e.g. two titles for one file:
from the highest-ranked claim down, a claim stays active unless it conflicts with one that did.

Files aren't settled against each other.
Two files whose claims number them as the same episode don't conflict:
they are two manifestations of it (see *Manifestation*), as two encodes of a movie are,
and a view links them as its consumer's versions of the episode (see *View*).
Files whose episodes overlap don't conflict either, e.g. One Piece's `E447-E450` and `E450-E451`,
which both hold episode 450: each keeps its whole range,
as holding only some of a file's episodes would say less than the file is.
Episodes numbered through the whole series, without a season, e.g. One Piece's `E92`,
are numbered differently from a season's.
