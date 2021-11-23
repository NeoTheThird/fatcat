/*
 * This file contains API server tests that hit the API by connecting via the Iron client,
 * essentially doing raw HTTP/JSON requests.
 *
 * These tests are relatively simple and don't mutate much database state; they are primarily to
 * test basic serialization/deserialization, and take advantage of hard-coded example entities.
 */

use diesel::prelude::*;
use fatcat::database_schema::*;
use iron::status;
use iron_test::request;

mod helpers;

#[test]
fn test_entity_gets() {
    let (headers, router, _conn) = helpers::setup_http();

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/container/aaaaaaaaaaaaaeiraaaaaaaaai",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("Trivial Results"),
    );

    // Check revision encoding
    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/container/aaaaaaaaaaaaaeiraaaaaaaaai",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("00000000-0000-0000-1111-fff000000002"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/creator/aaaaaaaaaaaaaircaaaaaaaaae",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("Grace Hopper"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/file/aaaaaaaaaaaaamztaaaaaaaaai",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("archive.org"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/fileset/aaaaaaaaaaaaaztgaaaaaaaaam",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some(".tar.gz"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/webcapture/aaaaaaaaaaaaa53xaaaaaaaaam",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("asheesh.org"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/aaaaaaaaaaaaarceaaaaaaaaai",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    // expand keyword
    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/aaaaaaaaaaaaarceaaaaaaaaai?expand=container",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("MySpace Blog"),
    );

    // hide keyword
    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/aaaaaaaaaaaaarceaaaaaaaaai?hide=refs,container",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    helpers::check_http_response(
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
    let (headers, router, _conn) = helpers::setup_http();

    helpers::check_http_response(
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
    let (headers, router, _conn) = helpers::setup_http();

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/container/aaaaaaaaaaaaaeiraaaaaaaaai/history",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("changelog"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/creator/aaaaaaaaaaaaaircaaaaaaaaae/history",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("changelog"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/file/aaaaaaaaaaaaamztaaaaaaaaam/history",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("changelog"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/fileset/aaaaaaaaaaaaaztgaaaaaaaaam/history",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("changelog"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/webcapture/aaaaaaaaaaaaa53xaaaaaaaaam/history",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("changelog"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/aaaaaaaaaaaaarceaaaaaaaaai/history",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("changelog"),
    );

    helpers::check_http_response(
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
    let (headers, router, _conn) = helpers::setup_http();

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/container/lookup?issnl=1234-0000",
            headers.clone(),
            &router,
        ),
        status::NotFound,
        None,
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/container/lookup?issnl=1234",
            headers.clone(),
            &router,
        ),
        status::BadRequest,
        None,
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/container/lookup?wikidata_qid=Q84913959359",
            headers.clone(),
            &router,
        ),
        status::NotFound,
        None,
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/container/lookup?wikidata_qid=84913959359",
            headers.clone(),
            &router,
        ),
        status::BadRequest,
        None,
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/container/lookup?wikidata_qid=Q84913959359&issnl=1234-0000",
            headers.clone(),
            &router,
        ),
        status::BadRequest,
        None,
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/container/lookup?issne=1234-3333",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("Journal of Trivial Results"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/container/lookup?issn=1234-3333",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("Journal of Trivial Results"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/creator/lookup?orcid=0000-0003-2088-7465",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("Christine Moran"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/creator/lookup?wikidata_qid=Q5678",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("John P. A. Ioannidis"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/creator/lookup?orcid=0000-0003-2088-0000",
            headers.clone(),
            &router,
        ),
        status::NotFound,
        None,
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/file/lookup?md5=00000000000ab9fdc2a128f962faebff",
            headers.clone(),
            &router,
        ),
        status::NotFound,
        None,
    );
    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/file/lookup?md5=00000000000ab9fdc2a128f962faebfff",
            headers.clone(),
            &router,
        ),
        status::BadRequest,
        None,
    );
    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/file/lookup?md5=f4de91152c7ab9fdc2a128f962faebff",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("0020124&type=printable"),
    );
    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/file/lookup?sha1=7d97e98f8af710c7e7fe703abc8f639e0ee507c4",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("robots.txt"),
    );
    helpers::check_http_response(
            request::get(
                "http://localhost:9411/v0/file/lookup?sha256=ffc1005680cb620eec4c913437dfabbf311b535cfe16cbaeb2faec1f92afc362",
                headers.clone(),
                &router,
            ),
            status::Ok,
            Some("0020124&type=printable"),
        );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/file/lookup?sha1=00000000000000c7e7fe703abc8f639e0ee507c4",
            headers.clone(),
            &router,
        ),
        status::NotFound,
        None,
    );

    // not URL encoded
    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/lookup?doi=10.123/abc",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    // not lower-case
    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/lookup?doi=10.123/ABC",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    // URL encoded
    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/lookup?doi=10.123%2Fabc",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/lookup?wikidata_qid=Q55555",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/lookup?pmid=54321",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/lookup?pmcid=PMC555",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/lookup?isbn13=978-3-16-148410-0",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/lookup?core=42022773",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/lookup?jstor=1819117828",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/lookup?mag=992489213",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/lookup?ark=ark:/13030/m53r5pzm",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/lookup?hdl=20.500.23456/ABC/DUMMY",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );
}

#[test]
fn test_reverse_lookups() {
    let (headers, router, _conn) = helpers::setup_http();

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/creator/aaaaaaaaaaaaaircaaaaaaaaai/releases",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("bigger example"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/aaaaaaaaaaaaarceaaaaaaaaai/files",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("7d97e98f8af710c7e7fe703abc8f639e0ee507c4"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/aaaaaaaaaaaaarceaaaaaaaaai/filesets",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("README.md"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/aaaaaaaaaaaaarceaaaaaaaaai/webcaptures",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("http://example.org"),
    );

    helpers::check_http_response(
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
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/container",
                editgroup_id
            ),
            headers,
            r#"{"name": "test journal", "publication_status": "active"}"#,
            &router,
        ),
        status::Created,
        None,
    ); // TODO: "test journal"
}

#[test]
fn test_post_batch_container() {
    let (headers, router, _conn) = helpers::setup_http();

    helpers::check_http_response(
        request::post(
            "http://localhost:9411/v0/editgroup/auto/container/batch",
            headers,
            r#"{"editgroup": {},
                "entity_list": [{"name": "test journal"}, {"name": "another test journal"}]}"#,
            &router,
        ),
        status::Created,
        None,
    ); // TODO: "test journal"
}

#[test]
fn test_post_creator() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/creator",
                editgroup_id
            ),
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
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    helpers::check_http_response(
        request::post(
            &format!("http://localhost:9411/v0/editgroup/{}/file", editgroup_id),
            headers.clone(),
            r#"{ }"#,
            &router,
        ),
        status::Created,
        None,
    );

    helpers::check_http_response(
        request::post(
            &format!("http://localhost:9411/v0/editgroup/{}/file", editgroup_id),
            headers.clone(),
            r#"{"size": 76543,
                "sha1": "f0000000000000008b7eb2a93e6d0440c1f3e7f8",
                "md5": "0b6d347b01d437a092be84c2edfce72c",
                "sha256": "a77e4c11a57f1d757fca5754a8f83b5d4ece49a2d28596889127c1a2f3f28832",
                "urls": [
                    {"url": "http://archive.org/asdf.txt", "rel": "web" },
                    {"url": "http://web.archive.org/2/http://archive.org/asdf.txt", "rel": "webarchive" }
                ],
                "mimetype": "application/pdf",
                "content_scope": "article",
                "release_ids": [
                    "aaaaaaaaaaaaarceaaaaaaaaae",
                    "aaaaaaaaaaaaarceaaaaaaaaai"
                ],
                "extra": { "source": "speculation" }
                }"#,
            &router,
        ),
        status::Created,
        None,
    );

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/accept",
                &editgroup_id
            ),
            headers.clone(),
            "",
            &router,
        ),
        status::Ok,
        None,
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/file/lookup?sha1=f0000000000000008b7eb2a93e6d0440c1f3e7f8",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("web.archive.org/2/http"),
    );
}

#[test]
fn test_post_fileset() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/fileset",
                editgroup_id
            ),
            headers.clone(),
            r#"{ }"#,
            &router,
        ),
        status::Created,
        None,
    );

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/fileset",
                editgroup_id
            ),
            headers.clone(),
            r#"{"manifest": [
                    {"path": "new_file.txt", "size": 12345, "sha1": "e9dd75237c94b209dc3ccd52722de6931a310ba3", "mimetype": "text/plain" },
                    {"path": "output/bogus.hdf5", "size": 43210, "sha1": "e9dd75237c94b209dc3ccd52722de6931a310ba3", "extra": {"some": "other value"} }
                ],
                "urls": [
                    {"url": "http://archive.org/download/dataset-0123/", "rel": "archive" },
                    {"url": "http://homepage.name/dataset/", "rel": "web" }
                ],
                "release_ids": [
                    "aaaaaaaaaaaaarceaaaaaaaaae",
                    "aaaaaaaaaaaaarceaaaaaaaaai"
                ],
                "content_scope": "dataset",
                "extra": { "source": "speculation" }
                }"#,
            &router,
        ),
        status::Created,
        None,
    );

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/accept",
                &editgroup_id
            ),
            headers.clone(),
            "",
            &router,
        ),
        status::Ok,
        None,
    );
    // TODO: there is no lookup for filesets
}

#[test]
fn test_post_webcapture() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/webcapture",
                editgroup_id
            ),
            headers.clone(),
            r#"{ "original_url": "https://fatcat.wiki",
                 "timestamp": "2018-12-28T11:11:11Z" }"#,
            &router,
        ),
        status::Created,
        None,
    );

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/webcapture",
                editgroup_id
            ),
            headers.clone(),
            r#"{"original_url": "https://bnewbold.net/",
                "timestamp": "2018-12-28T05:06:07Z",
                "content_scope": "landing-page",
                "cdx": [
                    {"surt": "org,asheesh,)/robots.txt",
                     "timestamp": "2018-12-28T05:06:07Z",
                     "url": "https://asheesh.org/robots.txt",
                     "status_code": 200,
                     "mimetype": "text/html",
                     "sha1": "e9dd75237c94b209dc3ccd52722de6931a310ba3" }
                ],
                "archive_urls": [
                    {"url": "http://archive.org/download/dataset-0123/", "rel": "archive" },
                    {"url": "http://homepage.name/dataset/", "rel": "web" }
                ],
                "release_ids": [
                    "aaaaaaaaaaaaarceaaaaaaaaae",
                    "aaaaaaaaaaaaarceaaaaaaaaai"
                ],
                "extra": { "source": "speculation" }
                }"#,
            &router,
        ),
        status::Created,
        None,
    );

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/accept",
                &editgroup_id
            ),
            headers.clone(),
            "",
            &router,
        ),
        status::Ok,
        None,
    );
    // TODO: there is no lookup for filesets
}

#[test]
fn test_post_release() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            // TODO: target_release_id
            r#"{"title": "secret minimal paper",
                "ext_ids": {},
                "release_type": "article-journal",
                "work_id": "aaaaaaaaaaaaavkvaaaaaaaaae"
                }"#,
            &router,
        ),
        status::Created,
        None,
    ); // TODO: "secret paper"

    // No work_id supplied (auto-created)
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            // TODO: target_release_id
            r#"{"title": "secret minimal paper the second",
                "ext_ids": {},
                "release_type": "article-journal"
                }"#,
            &router,
        ),
        status::Created,
        None,
    );

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            // TODO: target_release_id
            r#"{"title": "secret paper",
                "release_type": "article-journal",
                "release_date": "2000-01-02",
                "release_year": 2000,
                "ext_ids": {
                    "doi": "10.1234/abcde.781231231239",
                    "pmid": "54321",
                    "pmcid": "PMC12345",
                    "wikidata_qid": "Q12345",
                    "core": "7890"
                },
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
                        "raw_name": "textual description of contributor (aka, name)",
                        "given_name": "western first",
                        "surname": "western last",
                        "creator_id": "aaaaaaaaaaaaaircaaaaaaaaae",
                        "role": "author"
                    },{
                        "raw_name": "shorter"
                    }],
                "extra": { "source": "speculation" }
                }"#,
            &router,
        ),
        status::Created,
        None,
    ); // TODO: "secret paper"

    // Bogus non-existant fields
    /* TODO: doesn't fail
    helpers::check_http_response(
        request::post(
            &format!("http://localhost:9411/v0/editgroup/{}/release", editgroup_id),
            headers.clone(),
            r#"{"title": "secret minimal paper the second",
                "asdf123": "lalala"
                }"#,
            &router,
        ),
        status::BadRequest,
        None,
    );
    */
}

#[test]
fn test_post_work() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    helpers::check_http_response(
        request::post(
            &format!("http://localhost:9411/v0/editgroup/{}/work", editgroup_id),
            headers.clone(),
            // TODO: target_work_id
            r#"{
                "extra": { "source": "speculation" }
            }"#,
            &router,
        ),
        status::Created,
        None,
    );
}

#[test]
fn test_update_work() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    helpers::check_http_response(
        request::post(
            &format!("http://localhost:9411/v0/editgroup/{}/work", editgroup_id),
            headers.clone(),
            r#"{
                "extra": { "source": "other speculation" }
            }"#,
            &router,
        ),
        status::Created,
        None,
    );

    helpers::check_http_response(
        request::post(
            &format!("http://localhost:9411/v0/editgroup/{}/accept", editgroup_id),
            headers.clone(),
            "",
            &router,
        ),
        status::Ok,
        None,
    );
}

#[test]
fn test_delete_work() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    helpers::check_http_response(
        request::delete(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/work/aaaaaaaaaaaaavkvaaaaaaaaai",
                editgroup_id
            ),
            headers.clone(),
            &router,
        ),
        status::Ok,
        None,
    );

    helpers::check_http_response(
        request::post(
            &format!("http://localhost:9411/v0/editgroup/{}/accept", editgroup_id),
            headers.clone(),
            "",
            &router,
        ),
        status::Ok,
        None,
    );
}

#[test]
fn test_accept_editgroup() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    let c: i64 = container_ident::table
        .filter(container_ident::is_live.eq(false))
        .count()
        .get_result(&conn)
        .unwrap();
    assert_eq!(c, 0);
    let c: i64 = changelog::table
        .filter(changelog::editgroup_id.eq(editgroup_id.to_uuid()))
        .count()
        .get_result(&conn)
        .unwrap();
    assert_eq!(c, 0);

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/container",
                editgroup_id
            ),
            headers.clone(),
            &format!(
                "{{\"name\": \"test journal 1\", \"editgroup_id\": \"{}\"}}",
                editgroup_id
            ),
            &router,
        ),
        status::Created,
        None,
    );
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/container",
                editgroup_id
            ),
            headers.clone(),
            &format!(
                "{{\"name\": \"test journal 2\", \"editgroup_id\": \"{}\"}}",
                editgroup_id
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

    helpers::check_http_response(
        request::get(
            &format!("http://localhost:9411/v0/editgroup/{}", editgroup_id),
            headers.clone(),
            &router,
        ),
        status::Ok,
        None,
    );

    helpers::check_http_response(
        request::post(
            &format!("http://localhost:9411/v0/editgroup/{}/accept", editgroup_id),
            headers.clone(),
            "",
            &router,
        ),
        status::Ok,
        None,
    );

    helpers::check_http_response(
        request::post(
            &format!("http://localhost:9411/v0/editgroup/{}/accept", editgroup_id),
            headers.clone(),
            "",
            &router,
        ),
        status::BadRequest,
        None,
    );

    let c: i64 = container_ident::table
        .filter(container_ident::is_live.eq(false))
        .count()
        .get_result(&conn)
        .unwrap();
    assert_eq!(c, 0);
    let c: i64 = changelog::table
        .filter(changelog::editgroup_id.eq(editgroup_id.to_uuid()))
        .count()
        .get_result(&conn)
        .unwrap();
    assert_eq!(c, 1);
}

#[test]
fn test_changelog() {
    let (headers, router, _conn) = helpers::setup_http();

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/changelog",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("editgroup_id"),
    );

    helpers::check_http_response(
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
fn test_400() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers,
            r#"{"title": "secret paper",
                "release_type": "article-journal",
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
                        "raw_name": "textual description of contributor (aka, name)",
                        "creator_id": "aaaaaaaaaaaaaircaaaaaaaaae",
                        "contrib_type": "author"
                    },{
                        "raw_name": "shorter"
                    }],
                "extra": { "source": "speculation" }
                }"#,
            &router,
        ),
        status::BadRequest,
        None,
    );
}

#[test]
fn test_edit_gets() {
    let (headers, router, _conn) = helpers::setup_http();

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/editor/aaaaaaaaaaaabkvkaaaaaaaaae",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("admin"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/editor/aaaaaaaaaaaabkvkaaaaaaaaae/editgroups",
            headers.clone(),
            &router,
        ),
        status::Ok,
        None,
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/editgroup/aaaaaaaaaaaabo53aaaaaaaaae",
            headers.clone(),
            &router,
        ),
        status::Ok,
        None,
    );
}

#[test]
fn test_bad_external_idents() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    // Bad wikidata QID
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret paper",
                "ext_ids": {
                  "wikidata_qid": "P12345"
                }
                }"#,
            &router,
        ),
        status::BadRequest,
        Some("Wikidata QID"),
    );
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"name": "my journal",
                "ext_ids": {
                  "wikidata_qid": "P12345"
                }
                }"#,
            &router,
        ),
        status::BadRequest,
        Some("Wikidata QID"),
    );
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"display_name": "some body",
                "ext_ids": {
                  "wikidata_qid": "P12345"
                }
                }"#,
            &router,
        ),
        status::BadRequest,
        Some("Wikidata QID"),
    );

    // Bad PMCID
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret paper",
                "ext_ids": {
                  "pmcid": "12345"
                }
                }"#,
            &router,
        ),
        status::BadRequest,
        Some("PMCID"),
    );

    // Bad PMID
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret paper",
                "ext_ids": {
                  "pmid": "not-a-number"
                }
                }"#,
            &router,
        ),
        status::BadRequest,
        Some("PMID"),
    );

    // Bad DOI
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret paper",
                "ext_ids": {
                  "doi": "asdf"
                }
                }"#,
            &router,
        ),
        status::BadRequest,
        Some("DOI"),
    );

    // Good identifiers
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret paper",
                "ext_ids": {
                  "doi": "10.1234/abcde.781231231239",
                  "pmid": "54321",
                  "pmcid": "PMC12345",
                  "wikidata_qid": "Q12345"
                }
                }"#,
            &router,
        ),
        status::Created,
        None,
    );
}

#[test]
fn test_abstracts() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "some paper",
                "ext_ids": {
                  "doi": "10.1234/iiiiiii"
                },
                "abstracts": [
                  {"lang": "zh",
                   "mimetype": "text/plain",
                   "content": "some rando abstract 24iu3i25u2" },
                  {"lang": "en",
                   "mimetype": "application/xml+jats",
                   "content": "some other abstract 99139405" }
                ]
                }"#,
            &router,
        ),
        status::Created,
        None,
    );

    // Same abstracts; checking that re-inserting works
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "some paper again",
                "ext_ids": {},
                "abstracts": [
                  {"lang": "zh",
                   "mimetype": "text/plain",
                   "content": "some rando abstract 24iu3i25u2" },
                  {"lang": "en",
                   "mimetype": "application/xml+jats",
                   "content": "some other abstract 99139405" }
                ]
                }"#,
            &router,
        ),
        status::Created,
        None,
    );

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/accept",
                &editgroup_id
            ),
            headers.clone(),
            "",
            &router,
        ),
        status::Ok,
        None,
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/lookup?doi=10.1234/iiiiiii",
            headers.clone(),
            &router,
        ),
        status::Ok,
        // SHA-1 of first abstract string (with no trailing newline)
        Some("65c171bd8c968e12ede25ad95f02cd4b2ce9db52"),
    );
    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/lookup?doi=10.1234/iiiiiii",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("99139405"),
    );
    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/release/lookup?doi=10.1234/iiiiiii",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("24iu3i25u2"),
    );
}

#[test]
fn test_contribs() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "some paper",
                "ext_ids": {
                  "doi": "10.1234/iiiiiii"
                },
                "contribs": [{
                        "index": 1,
                        "raw_name": "textual description of contributor (aka, name)",
                        "creator_id": "aaaaaaaaaaaaaircaaaaaaaaae",
                        "contrib_type": "author",
                        "extra": {"key": "value 28328424942"}
                    },{
                        "raw_name": "shorter"
                    }]
                }"#,
            &router,
        ),
        status::Created,
        None,
    );

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/accept",
                &editgroup_id
            ),
            headers.clone(),
            "",
            &router,
        ),
        status::Ok,
        None,
    );
}

#[test]
fn test_release_dates() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    // Ok
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret minimal paper",
                "ext_ids": {},
                "release_type": "article-journal",
                "release_date": "2000-01-02"
                }"#,
            &router,
        ),
        status::Created,
        None,
    );

    // Ok
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret minimal paper",
                "ext_ids": {},
                "release_type": "article-journal",
                "release_year": 2000
                }"#,
            &router,
        ),
        status::Created,
        None,
    );

    // Ok; ISO 8601
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret minimal paper",
                "ext_ids": {},
                "release_type": "article-journal",
                "release_year": -100
                }"#,
            &router,
        ),
        status::Created,
        None,
    );
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret minimal paper",
                "ext_ids": {},
                "release_type": "article-journal",
                "release_year": 0
                }"#,
            &router,
        ),
        status::Created,
        None,
    );

    // Ok
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret minimal paper",
                "ext_ids": {},
                "release_type": "article-journal",
                "release_date": "2000-01-02",
                "release_year": 2000
                }"#,
            &router,
        ),
        status::Created,
        None,
    );

    // Ok for now, but may be excluded later
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret minimal paper",
                "ext_ids": {},
                "release_type": "article-journal",
                "release_date": "2000-01-02",
                "release_year": 1999
                }"#,
            &router,
        ),
        status::Created,
        None,
    );

    // Bad: year/month only
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret minimal paper",
                "ext_ids": {},
                "release_type": "article-journal",
                "release_date": "2000-01"
                }"#,
            &router,
        ),
        status::BadRequest,
        None,
    );

    // Bad: full timestamp
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret minimal paper",
                "ext_ids": {},
                "release_type": "article-journal",
                "release_date": "2005-08-30T00:00:00Z"
                }"#,
            &router,
        ),
        status::BadRequest,
        None,
    );

    // Bad: bogus month/day
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret minimal paper",
                "ext_ids": {},
                "release_type": "article-journal",
                "release_date": "2000-88-99"
                }"#,
            &router,
        ),
        status::BadRequest,
        None,
    );
}

#[test]
fn test_release_types() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    // Ok
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret minimal paper",
                "ext_ids": {},
                "release_type": "article-journal"
                }"#,
            &router,
        ),
        status::Created,
        None,
    );

    // Bad
    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/release",
                editgroup_id
            ),
            headers.clone(),
            r#"{"title": "secret minimal paper",
                "ext_ids": {},
                "release_type": "journal-article"
                }"#,
            &router,
        ),
        status::BadRequest,
        Some("release_type"),
    );
}

#[test]
fn test_create_editgroup() {
    let (headers, router, _conn) = helpers::setup_http();

    // We're authenticated, so don't need to supply editor_id
    helpers::check_http_response(
        request::post(
            "http://localhost:9411/v0/editgroup",
            headers.clone(),
            "{}",
            &router,
        ),
        status::Created,
        None,
    );

    // But can if we want to
    helpers::check_http_response(
        request::post(
            "http://localhost:9411/v0/editgroup",
            headers.clone(),
            r#"{"editor_id": "aaaaaaaaaaaabkvkaaaaaaaaae"}"#,
            &router,
        ),
        status::Created,
        None,
    );
}

#[test]
fn test_get_editgroup() {
    let (headers, router, _conn) = helpers::setup_http();

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/editgroup/aaaaaaaaaaaabo53aaaaaaaaae",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("changelog_index"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/editor/aaaaaaaaaaaabkvkaaaaaaaaam/editgroups",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("user edit"),
    );

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/editgroup/reviewable",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("edit for submission"),
    );
}

#[test]
fn test_put_editgroup() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    helpers::check_http_response(
        request::put(
            &format!(
                "http://localhost:9411/v0/editgroup/{}?submit=false",
                editgroup_id
            ),
            headers.clone(),
            &format!(r#"{{"editgroup_id": "{}"}}"#, editgroup_id),
            &router,
        ),
        status::Ok,
        None,
    );

    helpers::check_http_response(
        request::put(
            &format!(
                "http://localhost:9411/v0/editgroup/{}?submit=true",
                editgroup_id
            ),
            headers.clone(),
            &format!(r#"{{"editgroup_id": "{}"}}"#, editgroup_id),
            &router,
        ),
        status::Ok,
        None,
    );

    helpers::check_http_response(
        request::get(
            &format!("http://localhost:9411/v0/editgroup/reviewable"),
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("quick test editgroup"),
    );
}

#[test]
fn test_editgroup_annotations() {
    let (headers, router, conn) = helpers::setup_http();
    let editgroup_id = helpers::quick_editgroup(&conn);

    // pre-existing
    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/editgroup/aaaaaaaaaaaabo53aaaaaaaaa4/annotations",
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("love this edit"),
    );

    helpers::check_http_response(
        request::post(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/annotation",
                editgroup_id
            ),
            headers.clone(),
            r#"{"comment_markdown": "new special test annotation",
                "extra": {
                    "bogus-key": "bogus-value"}
               }"#,
            &router,
        ),
        status::Created,
        None,
    );

    helpers::check_http_response(
        request::get(
            &format!(
                "http://localhost:9411/v0/editgroup/{}/annotations",
                editgroup_id
            ),
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("special test annotation"),
    );

    helpers::check_http_response(
        request::get(
            &format!(
                "http://localhost:9411/v0/editor/{}/annotations",
                helpers::TEST_ADMIN_EDITOR_ID
            ),
            headers.clone(),
            &router,
        ),
        status::Ok,
        Some("special test annotation"),
    );
}

#[test]
fn test_query_params() {
    let (headers, router, _conn) = helpers::setup_http();

    helpers::check_http_response(
        request::get(
            "http://localhost:9411/v0/changelog?limit=true",
            headers.clone(),
            &router,
        ),
        status::BadRequest,
        Some("integer"),
    );

    helpers::check_http_response(
        request::get(
            &format!("http://localhost:9411/v0/editgroup/reviewable?since=asdf"),
            headers.clone(),
            &router,
        ),
        status::BadRequest,
        Some("datetime"),
    );

    helpers::check_http_response(
        request::get(
            &format!("http://localhost:9411/v0/editgroup/reviewable?since=1999-06-05T12:34:00Z"),
            headers.clone(),
            &router,
        ),
        status::Ok,
        None,
    );

    // Python3: datetime.datetime.utcnow().isoformat() + "Z"
    helpers::check_http_response(
        request::get(
            &format!(
                "http://localhost:9411/v0/editgroup/reviewable?since=2019-01-17T23:32:03.269010Z"
            ),
            headers.clone(),
            &router,
        ),
        status::Ok,
        None,
    );

    // Python3: datetime.datetime.now(datetime.timezone.utc).isoformat()
    /* TODO: this doesn't work currently :(
    helpers::check_http_response(
        request::get(
            &format!("http://localhost:9411/v0/editgroup/reviewable?since=2019-01-17T23:30:45.799289+00:00"),
            headers.clone(),
            &router,
        ),
        status::Ok,
        None,
    );
    */

    helpers::check_http_response(
        request::post(
            "http://localhost:9411/v0/editgroup/auto/container/batch",
            headers.clone(),
            r#"{"editgroup": {},
                "entity_list": [{"name": "test journal"}, {"name": "another test journal"}]
               }"#,
            &router,
        ),
        status::Created,
        None,
    );
}
