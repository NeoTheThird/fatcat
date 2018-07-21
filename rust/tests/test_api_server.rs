extern crate diesel;
extern crate fatcat;
extern crate fatcat_api;
extern crate iron;
extern crate iron_test;
extern crate uuid;

use diesel::prelude::*;
use fatcat::api_helpers::*;
use fatcat::database_schema::*;
use iron::headers::ContentType;
use iron::mime::Mime;
use iron::{status, Headers};
use iron_test::{request, response};
use uuid::Uuid;

fn setup() -> (
    Headers,
    fatcat_api::router::Router,
    diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>>,
) {
    let server = fatcat::test_server().unwrap();
    let conn = server.db_pool.get().expect("db_pool error");
    let router = fatcat_api::router(server);
    let mut headers = Headers::new();
    let mime: Mime = "application/json".parse().unwrap();
    headers.set(ContentType(mime));
    (headers, router, conn)
}

fn check_response(
    resp: iron::IronResult<iron::response::Response>,
    want_status: status::Status,
    in_body: Option<&str>,
) {
    let resp = resp.unwrap();
    let status = resp.status;
    let body = response::extract_body_to_string(resp);
    println!("{}", body);
    assert_eq!(status, Some(want_status));
    if let Some(thing) = in_body {
        assert!(body.contains(thing));
    }
}

#[test]
fn test_entity_gets() {
    let (headers, router, _conn) = setup();

    check_response(
        request::get(
            "http://localhost:9411/v0/container/aaaaaaaaaaaaaeiraaaaaaaaai",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("Trivial Results"),
    );

    check_response(
        request::get(
            "http://localhost:9411/v0/creator/aaaaaaaaaaaaaircaaaaaaaaae",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("Grace Hopper"),
    );

    check_response(
        request::get(
            "http://localhost:9411/v0/file/aaaaaaaaaaaaamztaaaaaaaaai",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("archive.org"),
    );

    check_response(
        request::get(
            "http://localhost:9411/v0/release/aaaaaaaaaaaaarceaaaaaaaaai",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    check_response(
        request::get(
            "http://localhost:9411/v0/work/aaaaaaaaaaaaavkvaaaaaaaaai",
            headers.clone(),
            &router,
        ),
        status::Ok,
        None,
    );
}

#[test]
fn test_entity_404() {
    let (headers, router, _conn) = setup();

    check_response(
        request::get(
            "http://localhost:9411/v0/creator/aaaaaaaaaaaaairceeeeeeeeee",
            headers.clone(),
            &router,
        ),
        status::NotFound,
        None,
    );
}

#[test]
fn test_entity_history() {
    let (headers, router, _conn) = setup();

    check_response(
        request::get(
            "http://localhost:9411/v0/container/aaaaaaaaaaaaaeiraaaaaaaaai/history",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("changelog"),
    );

    check_response(
        request::get(
            "http://localhost:9411/v0/creator/aaaaaaaaaaaaaircaaaaaaaaae/history",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("changelog"),
    );

    check_response(
        request::get(
            "http://localhost:9411/v0/file/aaaaaaaaaaaaamztaaaaaaaaai/history",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("changelog"),
    );

    check_response(
        request::get(
            "http://localhost:9411/v0/release/aaaaaaaaaaaaarceaaaaaaaaai/history",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("changelog"),
    );

    check_response(
        request::get(
            "http://localhost:9411/v0/work/aaaaaaaaaaaaavkvaaaaaaaaai/history",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("changelog"),
    );
}

#[test]
fn test_lookups() {
    let (headers, router, _conn) = setup();

    check_response(
        request::get(
            "http://localhost:9411/v0/container/lookup?issnl=1234-5678",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("Journal of Trivial Results"),
    );

    check_response(
        request::get(
            "http://localhost:9411/v0/creator/lookup?orcid=0000-0003-2088-7465",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("Christine Moran"),
    );

    check_response(
        request::get(
            "http://localhost:9411/v0/file/lookup?sha1=7d97e98f8af710c7e7fe703abc8f639e0ee507c4",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("robots.txt"),
    );
}

#[test]
fn test_reverse_lookups() {
    let (headers, router, _conn) = setup();

    check_response(
        request::get(
            "http://localhost:9411/v0/creator/aaaaaaaaaaaaaircaaaaaaaaai/releases",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    check_response(
        request::get(
            "http://localhost:9411/v0/release/aaaaaaaaaaaaarceaaaaaaaaai/files",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("7d97e98f8af710c7e7fe703abc8f639e0ee507c4"),
    );

    check_response(
        request::get(
            "http://localhost:9411/v0/work/aaaaaaaaaaaaavkvaaaaaaaaai/releases",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );
}

#[test]
fn test_post_container() {
    let (headers, router, _conn) = setup();

    check_response(
        request::post(
            "http://localhost:9411/v0/container",
            headers,
            r#"{"name": "test journal"}"#,
            &router,
        ),
        status::Created,
        None,
    ); // TODO: "test journal"
}

#[test]
fn test_post_batch_container() {
    let (headers, router, _conn) = setup();

    check_response(
        request::post(
            "http://localhost:9411/v0/container/batch",
            headers,
            r#"[{"name": "test journal"}, {"name": "another test journal"}]"#,
            &router,
        ),
        status::Created,
        None,
    ); // TODO: "test journal"
}

#[test]
fn test_post_creator() {
    let (headers, router, _conn) = setup();

    check_response(
        request::post(
            "http://localhost:9411/v0/creator",
            headers,
            r#"{"display_name": "some person"}"#,
            &router,
        ),
        status::Created,
        None,
    );
}

#[test]
fn test_post_file() {
    let (headers, router, _conn) = setup();

    check_response(
        request::post(
            "http://localhost:9411/v0/file",
            headers.clone(),
            r#"{ }"#,
            &router,
        ),
        status::Created,
        None,
    );

    check_response(
        request::post(
            "http://localhost:9411/v0/file",
            headers,
            r#"{"size": 76543,
                "sha1": "f013d66c7f6817d08b7eb2a93e6d0440c1f3e7f8",
                "md5": "f013d66c7f6817d08b7eb2a93e6d0440c1f3e7f8",
                "sha256": "f013d66c7f6817d08b7eb2a93e6d0440c1f3e7f8",
                "url": "http://archive.org/asdf.txt",
                "mimetype": "application/pdf",
                "releases": [
                    "aaaaaaaaaaaaarceaaaaaaaaae",
                    "aaaaaaaaaaaaarceaaaaaaaaai"
                ],
                "extra": { "source": "speculation" }
                }"#,
            &router,
        ),
        status::Created,
        None,
    ); // TODO: "secret paper"
}

#[test]
fn test_post_release() {
    let (headers, router, _conn) = setup();

    check_response(
        request::post(
            "http://localhost:9411/v0/release",
            headers.clone(),
            // TODO: target_release_id
            r#"{"title": "secret minimal paper",
                "release_type": "journal-article",
                "work_id": "aaaaaaaaaaaaavkvaaaaaaaaae"
                }"#,
            &router,
        ),
        status::Created,
        None,
    ); // TODO: "secret paper"

    // No work_id supplied (auto-created)
    check_response(
        request::post(
            "http://localhost:9411/v0/release",
            headers.clone(),
            // TODO: target_release_id
            r#"{"title": "secret minimal paper the second",
                "release_type": "journal-article"
                }"#,
            &router,
        ),
        status::Created,
        None,
    );

    check_response(
        request::post(
            "http://localhost:9411/v0/release",
            headers,
            // TODO: target_release_id
            r#"{"title": "secret paper",
                "release_type": "journal-article",
                "doi": "10.1234/abcde.781231231239",
                "volume": "439",
                "issue": "IV",
                "pages": "1-399",
                "work_id": "aaaaaaaaaaaaavkvaaaaaaaaai",
                "container_id": "aaaaaaaaaaaaaeiraaaaaaaaae",
                "refs": [{
                        "index": 3,
                        "raw": "just a string"
                    },{
                        "raw": "just a string"
                    }],
                "contribs": [{
                        "index": 1,
                        "raw": "textual description of contributor (aka, name)",
                        "creator_id": "aaaaaaaaaaaaaircaaaaaaaaae",
                        "contrib_type": "author"
                    },{
                        "raw": "shorter"
                    }],
                "extra": { "source": "speculation" }
                }"#,
            &router,
        ),
        status::Created,
        None,
    ); // TODO: "secret paper"
}

#[test]
fn test_post_work() {
    let (headers, router, _conn) = setup();

    check_response(
        request::post(
            "http://localhost:9411/v0/work",
            headers.clone(),
            // TODO: target_work_id
            r#"{
                "work_type": "journal-article",
                "extra": { "source": "speculation" }
            }"#,
            &router,
        ),
        status::Created,
        None,
    );
}

#[test]
fn test_accept_editgroup() {
    let (headers, router, conn) = setup();

    let editor_id = Uuid::parse_str("00000000-0000-0000-AAAA-000000000001").unwrap();
    let editgroup_id = get_or_create_editgroup(editor_id, &conn).unwrap();

    let c: i64 = container_ident::table
        .filter(container_ident::is_live.eq(false))
        .count()
        .get_result(&conn)
        .unwrap();
    assert_eq!(c, 0);
    let c: i64 = changelog::table
        .filter(changelog::editgroup_id.eq(editgroup_id))
        .count()
        .get_result(&conn)
        .unwrap();
    assert_eq!(c, 0);

    check_response(
        request::post(
            "http://localhost:9411/v0/container",
            headers.clone(),
            &format!(
                "{{\"name\": \"test journal 1\", \"editgroup_id\": \"{}\"}}",
                uuid2fcid(&editgroup_id)
            ),
            &router,
        ),
        status::Created,
        None,
    );
    check_response(
        request::post(
            "http://localhost:9411/v0/container",
            headers.clone(),
            &format!(
                "{{\"name\": \"test journal 2\", \"editgroup_id\": \"{}\"}}",
                uuid2fcid(&editgroup_id)
            ),
            &router,
        ),
        status::Created,
        None,
    );

    let c: i64 = container_ident::table
        .filter(container_ident::is_live.eq(false))
        .count()
        .get_result(&conn)
        .unwrap();
    assert_eq!(c, 2);

    check_response(
        request::get(
            &format!(
                "http://localhost:9411/v0/editgroup/{}",
                uuid2fcid(&editgroup_id)
            ),
            headers.clone(),
            &router,
        ),
        status::Ok,
        None,
    );

    check_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/accept",
                uuid2fcid(&editgroup_id)
            ),
            headers.clone(),
            "",
            &router,
        ),
        status::Ok,
        None,
    );

    let c: i64 = container_ident::table
        .filter(container_ident::is_live.eq(false))
        .count()
        .get_result(&conn)
        .unwrap();
    assert_eq!(c, 0);
    let c: i64 = changelog::table
        .filter(changelog::editgroup_id.eq(editgroup_id))
        .count()
        .get_result(&conn)
        .unwrap();
    assert_eq!(c, 1);
}

#[test]
fn test_changelog() {
    let (headers, router, _conn) = setup();

    check_response(
        request::get(
            "http://localhost:9411/v0/changelog",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("editgroup_id"),
    );

    check_response(
        request::get(
            "http://localhost:9411/v0/changelog/1",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("files"),
    );
}

#[test]
fn test_stats() {
    let (headers, router, _conn) = setup();

    check_response(
        request::get("http://localhost:9411/v0/stats", headers.clone(), &router),
        status::Ok,
        Some("merged_editgroups"),
    );
    check_response(
        request::get(
            "http://localhost:9411/v0/stats?more=yes",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("merged_editgroups"),
    );
}

#[test]
fn test_400() {
    let (headers, router, _conn) = setup();

    check_response(
        request::post(
            "http://localhost:9411/v0/release",
            headers,
            r#"{"title": "secret paper",
                "release_type": "journal-article",
                "doi": "10.1234/abcde.781231231239",
                "volume": "439",
                "issue": "IV",
                "pages": "1-399",
                "work_id": "aaaaaaaaaaaaavkvaaaaaaaaae",
                "container_id": "aaaaaaaaaaaaaeiraaaaaae",
                "refs": [{
                        "index": 3,
                        "raw": "just a string"
                    },{
                        "raw": "just a string"
                    }],
                "contribs": [{
                        "index": 1,
                        "raw": "textual description of contributor (aka, name)",
                        "creator_id": "aaaaaaaaaaaaaircaaaaaaaaae",
                        "contrib_type": "author"
                    },{
                        "raw": "shorter"
                    }],
                "extra": { "source": "speculation" }
                }"#,
            &router,
        ),
        status::BadRequest,
        None,
    );
}
