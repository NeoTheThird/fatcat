# REST API

The Fatcat HTTP API is mostly a classic REST "CRUD" (Create, Read, Update,
Delete) API, with a few twists.

A declarative specification of all API endpoints, JSON data models, and
response types is available in OpenAPI 2.0 format. Code generation tools are
used to generate both server-side type-safe endpoint routes and client-side
libraries. Auto-generated reference documentation is, for now, available at
<https://api.fatcat.wiki>.

All API traffic is over HTTPS; there is no HTTP endpoint, even for read-only
operations. All endpoints accept and return only JSON serialized content.

## Entity Endpoints/Actions

Actions could, in theory, be directed at any of:

    entities (ident)
    revision
    edit

Top-level entity actions (resulting in edits):

    create (new rev)
    update (new rev)
    delete
    redirect
    split (remove redirect)

On existing entity edits (within a group):

    update
    delete

An edit group as a whole can be:

    create
    submit
    accept

Other per-entity endpoints:

    lookup (by external persistent identifier)
    match (by field/context; unimplemented)

## Editgroups

All mutating entity operations (create, update, delete) accept a required
`editgroup_id` query parameter. Editgroups (with contextual metadata) should be
created before starting edits.

Related edits (to multiple entities) should be collected under a single
editgroup, up to a reasonable size. More than 50 edits per entity type, or more
than 100 edits total in an editgroup become unwieldy.

After creating and modifying the editgroup, it may be "submitted", which flags
it for review by bot and human editors. The editgroup may be "accepted"
(merged), or if changes are necessary the edits can be updated and
re-submitted.

## Sub-Entity Expansion

To reduce the need for multiple GET queries when looking for common related
metadata, it is possible to include linked entities in responses using the
`expand` query parameter. For example, by default the `release` model only
includes an optional `container_id` field which points to a container entity.
If the `expand` parameter is set:

    https://api.fatcat.wiki/v0/release/aaaaaaaaaaaaarceaaaaaaaaam?expand=container

Then the full container model will be included under the `container` field.
Multiple expand parameters can be passed, comma-separated.

## Authentication and Authorization

There are two editor types: bots and humans. Additionally, either type of
editor may have additional privileges which allow them to, eg, directly accept
editgroups (as opposed to submitting edits for review).

All mutating API calls (POST, PUT, DELETE HTTP verbs) require token-based
authentication using an HTTP Bearer token. New tokens can be generated in the
web interface.

## Autoaccept Flag

Currently only on batch creation (POST) for entities.

For all bulk operations, optional 'editgroup' query parameter overrides
individual editgroup parameters.

If autoaccept flag is set and editgroup is not, a new editgroup is
automatically created and overrides for all entities inserted. Note
that this is different behavior from the "use current or create new"
default behavior for regular creation.

Unfortunately, "true" and "false" are the only values acceptable for boolean
rust/openapi2 query parameters

## QA Instance

The intent is to run a public "sandbox" QA instance of the catalog, using a
subset of the full catalog, running the most recent development branch of the
API specification. This instance can be used by developers for prototyping and
experimentation, though note that all data is periodically wiped, and this
endpoint is more likely to have bugs or be offline.

