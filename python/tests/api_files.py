from fatcat_openapi_client import *
from fixtures import *


def test_file(api):

    eg = quick_eg(api)

    # all the fields!
    f1 = FileEntity(
        size=89238,
        md5="7ce6615b2a5904939576d9567bd5f68e",
        sha1="027e7ed3ea1a40e92dd2657a1e3c992b5dc45dd2",
        sha256="f1f4f18a904e76818863ccbc6141fce92b0dcb47b0d6041aec98bc6806e393c3",
        mimetype="application/pdf",
        urls=[
            FileUrl(
                url="https://web.archive.org/web/12345542/something.com/blah.pdf",
                rel="webarchive",
            ),
        ],
        release_ids=[],
        extra=dict(a=2, b=5),
        edit_extra=dict(test_key="files rule"),
    )

    f1edit = api.create_file(eg.editgroup_id, f1)
    assert f1edit.extra == f1.edit_extra
    api.accept_editgroup(eg.editgroup_id)
    f2 = api.get_file(f1edit.ident)

    # check that fields match
    assert f1.size == f2.size
    assert f1.md5 == f2.md5
    assert f1.sha1 == f2.sha1
    assert f1.sha256 == f2.sha256
    assert f1.mimetype == f2.mimetype
    assert f1.extra == f2.extra
    assert f1.urls == f2.urls
    assert f1.release_ids == f2.release_ids
    assert f1.extra == f2.extra

    # get revision
    f2_rev = api.get_file_revision(f1edit.revision)
    assert f1edit.revision == f2_rev.revision
    assert f2.revision == f2_rev.revision

    # get redirects (none)
    assert api.get_file_redirects(f2.ident) == []

    # delete
    eg = quick_eg(api)
    api.delete_file(eg.editgroup_id, f2.ident)
    api.accept_editgroup(eg.editgroup_id)
    f2 = api.get_file(f2.ident)
    assert f2.state == "deleted"


def test_file_examples(api):

    f1 = api.lookup_file(
        sha256="ffc1005680cb620eec4c913437dfabbf311b535cfe16cbaeb2faec1f92afc362"
    )
    assert f1.releases is None

    f1 = api.get_file("aaaaaaaaaaaaamztaaaaaaaaam", expand="releases")
    assert f1.sha256 == "ffc1005680cb620eec4c913437dfabbf311b535cfe16cbaeb2faec1f92afc362"
    assert f1.releases[0].ident
    assert f1.releases[0].abstracts is None
    assert f1.releases[0].refs is None

    # expansion (back from release)
    r1 = api.get_release(f1.releases[0].ident, expand="files")
    assert r1.files
    assert f1.ident in [f.ident for f in r1.files]
