
## In Progress


## Next Up


## Bugs


## Next Full Release "Touch"

Will update all release entities (or at least all Crossref-derived entities).
Want to minimize edit counts, so will bundle a bunch of changes

- structured contrib names (given, sur)
- reference linking (release-to-release), via crossref DOI refs
- subtitle as field; remove from extra
- remove crossref alt ids that are just the DOI (?)

## Production Public Launch Blockers

- audit fatcat metadata for CC-0
- guide updates for auth
- privacy policy, and link from: create account, create edit

## Production Tech Sanity

- postgresql replication
- haproxy somewhere/how
- logging iteration: larger journald buffers? point somewhere?

## Unsorted

- broader use of external identifier normalizer functions
- "delete entity" and "merge entity" webface flows
- update editor, editgroup, changelog views?
- review bots:
    - tests
    - not actually processing work entities
    - filter out already reviewed
    - handle deletions, merges
    - examples of warnings, etc
- missing test coverage (python):
    batch create work, fileset, webcapture
    delete entity (for each entity type)
    delete entity edits (for each entity type)
    get entity edit (for each entity type)
    get entity redirects (for each entity type)
    get entity revision (for each entity type)
    get release webcaptures
    update editor (?)
    update fileset, webcapture
    release elastic transform (rich extra)
    successful web entity edits (create fresh entities first)
    editgroup web submit, accept, annotate
- API: ability to expand containers (and files, etc?) in releases-for-work
- API: /releases endpoint (and/or expansion) for releases-for-file (etc)
- cleanup ./notes/ directory
- links say "Download ..." but open in same page, not download
- workers (like entity updater) should use env vars more
- ansible: ISSN-L download/symlink
- page-one.live.cf.public.springer.com seems to serve up bogus one-pagers; should exclude
- QA sentry has very little host info; also not URL of request
- elastic schemas:
    release: creator_id
        should `release_year` be of date type, instead of int?
    files: domain list; mimetype; release count; url count; web/publisher/etc;
        size; has_md5/sha256/sha1; in_ia, in_shadow
- webface: still need to collapse links by domain better, and also vs. www.x/x
- entity edit JSON objects could include `entity_type`
- refactor 'fatcatd' to 'fatcat-api'
- changelog elastic stuff (is there even a fatcat-export for this?)
- 'hide' flag for exporter (eg, to skip abstracts and refs in some release dumps)
- https://tech.labs.oliverwyman.com/blog/2019/01/14/serialising-rust-tests/
- changelog elastic index (for stats)
- API: allow deletion of empty, un-accepted editgroups

## Ideas

- `poster` as a `release_type`
- "revert editgroup" mechanism (creates new editgroup)
- can guess some `release_stage` of files by looking at wayback date vs.
  published date
- ORCID apparently has 37 mil "work activities" (patents, etc), and only 14 mil
  unique DOIs; could import those other "work activities"? do they have
  identifiers?
- use https://github.com/codelucas/newspaper to extract fulltext+metadata from HTML crawls
- `fatcat-auth` tool should support more caveats, both when generating new or mutating existing tokens
- fast path to skip recursive redirect checks for bulk inserts
- when API GET-ing "wip" entities, require a parameter ("allow_wip"), else get a 404
- maybe better 'success' return message? eg, "success: true" flag
- idea: allow users to generate their own editgroup UUIDs, to reduce a round
  trips and "hanging" editgroups (created but never edited)
- refactor API schema for some entity-generic methods (eg, history, edit
  operations) to take entity type as a URL path param. greatly reduce macro
  foolery and method count/complexity, and ease creation of new entities
    => /{entity}/edit/{edit_id}
    => /{entity}/{ident}/redirects
    => /{entity}/{ident}/history
- investigate data quality by looking at, eg, most popular author strings, most
  popular titles, duplicated containers, etc

## Metadata Import

- 158 "NULL" publishers in journal metadata
- crossref: many ISBNs not getting copied; use python library to convert?
- remove 'first' from contrib crossref 'seq' (not helpful?)
- should probably check for 'jats:' in abstract before setting mimetype, even from crossref
- XML etc in metadata
    => (python) tests for these!
    https://qa.fatcat.wiki/release/search?q=xmlns
    https://qa.fatcat.wiki/release/search?q=%24gt
- better/complete reltypes probably good (eg, list of IRs, academic domain)
- include crossref-capitalized DOI in extra
- crossref: relations ("is-preprint-of")
- crossref: two phase: no citations, then matched citations (via DOI table)
- special "alias" DOIs... in crossref metadata?

new importers:
- DOAJ
- CORE (filtered)
- semantic scholar (up to 39 million; includes author de-dupe)

## Guide / Book / Style

- more+better terms+policies: https://tosdr.org/index.html

## Schema / Entity Fields

- file+fileset "first seen" datetime
- file type/scope/coverage: "fulltext", "abstract", etc
- elastic transform should only include authors, not editors (?)
- `translation_of` field on releases (or similar/general). `retraction_of` to a
  specific release? `alias_of`/`duplicate_of`
- 'part-of' relation for releases (release to release, eg for book chapters) and possibly containers

## API Schema / Design

- `release_month` field. for journals, having the year and month but not day is relatively common (citation needed)

## Web Interface

- include that ISO library to do lang/country name decodes
- container-name when no `container_id`. eg: 10.1016/b978-0-08-037302-7.50022-7

## Other / Backburner

- regression test imports for missing orcid display and journal metadata name
- try out beautifulsoup? (https://stackoverflow.com/a/34532382/4682349)
- `doi` field for containers (at least for "journal" type; maybe for "series" as well?)
- refactor webface views to use shared entity_view.html template
- shadow library manifest importer
- book identifiers: OCLC, openlibrary
- ref from guide: https://creativecommons.org/2012/08/14/library-catalog-metadata-open-licensing-or-public-domain/
- test redirect/delete elasticsearch change
- fake DOI (use in examples): 10.5555/12345678
- refactor elasticsearch inserter to be a class (eg, for command line use)
- document: elastic query date syntax is like: date:[2018-10-01 TO 2018-12-31]
- display abstracts better. no hashes or metadata; prefer plain or HTML,
  convert JATS if necessary
- switch from slog to simple pretty_env_log
- format returned datetimes with only second precision, not millisecond (RFC mode)
    => buried in model serialization internals
- refactor openapi schema to use shared response types
- consider using "HTTP 202: Accepted" for entity-mutating calls
- basic python hbase/elastic matcher
  => takes sha1 keys
  => checks fatcat API + hbase
  => if not matched yet, tries elastic search
  => simple ~exact match heuristic
  => proof-of-concept, no tests
- add_header Strict-Transport-Security "max-age=3600";
    => 12 hours? 24?
- haproxy for rate-limiting

better API docs
- readme.io has a free open source plan (or at least used to)
- https://github.com/readmeio/api-explorer
- https://github.com/lord/slate
- https://sourcey.com/spectacle/
- https://github.com/DapperDox/dapperdox

CSL:
- https://citationstyles.org/
- https://github.com/citation-style-language/documentation/blob/master/primer.txt
- https://citeproc-js.readthedocs.io/en/latest/csl-json/markup.html
- https://github.com/citation-style-language/schema/blob/master/csl-types.rnc
- perhaps a "create from CSL" endpoint?
