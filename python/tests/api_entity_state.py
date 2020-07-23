
import pytest

from fatcat_openapi_client import *
from fatcat_openapi_client.rest import ApiException
from fixtures import *


def test_redirect_entity(api):
    """
    Create two creators; merge
        => get both by ident
        => lookup by orcid; should not get old/merged one
        => update first; check that get on second by ident returns updated record
        => split second back out and re-get by ident/orcid
    """

    offset = 0
    while True:
        offset += 1
        o1 = '0000-0000-1111-%04d' % offset
        o2 = '0000-0000-2222-%04d' % offset
        try:
            api.lookup_creator(orcid=o1)
            continue
        except ApiException:
            pass
        try:
            api.lookup_creator(orcid=o2)
            continue
        except ApiException:
            pass
        break

    c1 = CreatorEntity(display_name="test one", orcid=o1)
    c2 = CreatorEntity(display_name="test two", orcid=o2)

    # create two creators
    eg = quick_eg(api)
    c1_edit = api.create_creator(eg.editgroup_id, c1)
    c2_edit = api.create_creator(eg.editgroup_id, c2)
    c1 = api.get_creator(c1_edit.ident)
    c2 = api.get_creator(c2_edit.ident)
    assert c1_edit.prev_revision is None
    assert c2_edit.prev_revision is None
    api.accept_editgroup(eg.editgroup_id)
    redirs = api.get_creator_redirects(c1.ident)
    assert redirs == []

    # merge second into first
    c2_redirect = CreatorEntity(redirect=c1.ident)
    eg = quick_eg(api)
    merge_edit = api.update_creator(eg.editgroup_id, c2.ident, c2_redirect)
    assert merge_edit.prev_revision == c2.revision
    redirs = api.get_creator_redirects(c1.ident)
    assert redirs == []
    api.accept_editgroup(eg.editgroup_id)
    redirs = api.get_creator_redirects(c1.ident)
    assert redirs == [c2.ident, ]

    # get both by ident
    res = api.get_creator(c1.ident)
    assert res.state == "active"
    res = api.get_creator(c2.ident)
    assert res.state == "redirect"
    assert res.revision == c1.revision
    assert res.redirect == c1.ident
    assert res.display_name == "test one"

    # get by orcid
    res = api.lookup_creator(orcid=o1)
    assert res.ident == c1.ident
    with pytest.raises(fatcat_openapi_client.rest.ApiException):
        res = api.lookup_creator(orcid=o2)

    # update first; check that get on second updates
    c1.display_name = "test one one"
    eg = quick_eg(api)
    update_edit = api.update_creator(eg.editgroup_id, c1.ident, c1)
    assert update_edit.prev_revision == c1.revision
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c2.ident)
    assert res.state == "redirect"
    assert res.display_name == "test one one"

    # delete first; check that second is deleted (but state is redirect)
    eg = quick_eg(api)
    del_edit = api.delete_creator(eg.editgroup_id, c1.ident)
    assert del_edit.prev_revision == update_edit.revision
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c1.ident)
    assert res.state == "deleted"
    assert res.display_name is None
    res = api.get_creator(c2.ident)
    assert res.state == "redirect"
    assert res.display_name is None
    assert res.revision is None
    redirs = api.get_creator_redirects(c1.ident)
    assert redirs == [c2.ident, ]

    # undelete first; check that second is a redirect
    eg = quick_eg(api)
    undelete_edit = api.update_creator(eg.editgroup_id, c1.ident, c1)
    assert undelete_edit.prev_revision is None
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c2.ident)
    assert res.state == "redirect"
    assert res.display_name == "test one one"
    redirs = api.get_creator_redirects(c1.ident)
    assert redirs == [c2.ident, ]

    # split second entity back out
    assert c2.revision
    assert c2.redirect is None
    eg = quick_eg(api)
    update_edit = api.update_creator(eg.editgroup_id, c2.ident, c2)
    # prev_revision should be none after an un-redirect
    assert update_edit.prev_revision is None
    redirs = api.get_creator_redirects(c1.ident)
    assert redirs == [c2.ident, ]
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c2.ident)
    assert res.state == "active"
    assert res.display_name == "test two"
    res = api.lookup_creator(orcid=o2)
    assert res.display_name == "test two"
    redirs = api.get_creator_redirects(c1.ident)
    assert redirs == []

    # cleanup
    eg = quick_eg(api)
    api.delete_creator(eg.editgroup_id, c1.ident)
    api.delete_creator(eg.editgroup_id, c2.ident)
    api.accept_editgroup(eg.editgroup_id)


def test_delete_entity(api):

    offset = 0
    while True:
        offset += 1
        o1 = '0000-0000-1111-%04d' % offset
        try:
            api.lookup_creator(orcid=o1)
            continue
        except ApiException:
            pass
        break

    c1 = CreatorEntity(display_name="test deletable", orcid=o1)

    # create
    eg = quick_eg(api)
    c1 = api.get_creator(api.create_creator(eg.editgroup_id, c1).ident)
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c1.ident)
    assert res.state == "active"
    assert res.display_name == "test deletable"
    res = api.lookup_creator(orcid=c1.orcid)
    assert res.state == "active"
    assert res.display_name == "test deletable"

    # delete
    eg = quick_eg(api)
    api.delete_creator(eg.editgroup_id, c1.ident)
    with pytest.raises(fatcat_openapi_client.rest.ApiException):
        # can't re-delete in same editgroup
        api.delete_creator(eg.editgroup_id, c1.ident)
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c1.ident)
    assert res.state == "deleted"
    assert res.display_name is None
    with pytest.raises(fatcat_openapi_client.rest.ApiException):
        res = api.lookup_creator(orcid=c1.orcid)

    # re-delete
    eg = quick_eg(api)
    try:
        # can't re-delete an entity
        api.delete_creator(eg.editgroup_id, c1.ident)
        #api.accept_editgroup(eg.editgroup_id)
        assert False
    except fatcat_openapi_client.rest.ApiException as e:
        assert 400 <= e.status < 500 # error is 4xx

    # undelete
    eg = quick_eg(api)
    api.update_creator(eg.editgroup_id, c1.ident, c1)
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c1.ident)
    assert res.state == "active"
    assert res.display_name == "test deletable"
    res = api.lookup_creator(orcid=c1.orcid)
    assert res.state == "active"
    assert res.display_name == "test deletable"

    # cleanup
    eg = quick_eg(api)
    api.delete_creator(eg.editgroup_id, c1.ident)
    api.accept_editgroup(eg.editgroup_id)


def test_recursive_redirects_entity(api):

    offset = 0
    while True:
        offset += 1
        o1 = '0000-0000-1111-%04d' % offset
        o2 = '0000-0000-2222-%04d' % offset
        o3 = '0000-0000-3333-%04d' % offset
        try:
            api.lookup_creator(orcid=o1)
            continue
        except ApiException:
            pass
        try:
            api.lookup_creator(orcid=o2)
            continue
        except ApiException:
            pass
        try:
            api.lookup_creator(orcid=o3)
            continue
        except ApiException:
            pass
        break

    c1 = CreatorEntity(display_name="test one", orcid=o1)
    c2 = CreatorEntity(display_name="test two", orcid=o2)
    c3 = CreatorEntity(display_name="test three", orcid=o3)

    # create three creators
    eg = quick_eg(api)
    c1 = api.get_creator(api.create_creator(eg.editgroup_id, c1).ident)
    c2 = api.get_creator(api.create_creator(eg.editgroup_id, c2).ident)
    c3 = api.get_creator(api.create_creator(eg.editgroup_id, c3).ident)
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c3.ident)
    assert res.display_name == "test three"

    # redirect third to second
    c3_redirect = CreatorEntity(redirect=c2.ident)
    eg = quick_eg(api)
    api.update_creator(eg.editgroup_id, c3.ident, c3_redirect)
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c3.ident)
    assert res.display_name == "test two"

    # redirect second to first: should be an error at merge time
    c2_redirect = CreatorEntity(redirect=c1.ident)
    eg = quick_eg(api)
    api.update_creator(eg.editgroup_id, c2.ident, c2_redirect)
    with pytest.raises(fatcat_openapi_client.rest.ApiException):
        api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c2.ident)
    assert res.display_name == "test two"
    with pytest.raises(fatcat_openapi_client.rest.ApiException):
        res = api.lookup_creator(orcid=o3)
    res = api.lookup_creator(orcid=o2)
    assert res.ident == c2.ident

    # redirect first to third: should be an error at merge time
    c1_redirect = CreatorEntity(redirect=c3.ident)
    eg = quick_eg(api)
    api.update_creator(eg.editgroup_id, c1.ident, c1_redirect)
    with pytest.raises(fatcat_openapi_client.rest.ApiException):
        api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c1.ident)
    assert res.display_name == "test one"

    # update second; check that third updated
    c2.display_name = "test two updated"
    eg = quick_eg(api)
    api.update_creator(eg.editgroup_id, c2.ident, c2)
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c2.ident)
    c2 = res
    assert res.display_name == "test two updated"
    res = api.get_creator(c3.ident)
    assert res.display_name == "test two updated"
    assert res.state == "redirect"
    res = api.lookup_creator(orcid=o2)
    assert res.ident == c2.ident

    # delete second; check that third updated
    eg = quick_eg(api)
    api.delete_creator(eg.editgroup_id, c2.ident)
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c2.ident)
    assert res.state == "deleted"
    res = api.get_creator(c3.ident)
    assert res.state == "redirect"
    assert res.display_name is None
    with pytest.raises(fatcat_openapi_client.rest.ApiException):
        res = api.lookup_creator(orcid=o2)

    # undelete second; check that third updated
    eg = quick_eg(api)
    c2_undelete = CreatorEntity(revision=c2.revision)
    api.update_creator(eg.editgroup_id, c2.ident, c2_undelete)
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c2.ident)
    assert res.state == "active"
    assert res.display_name == "test two updated"
    res = api.get_creator(c3.ident)
    assert res.state == "redirect"
    assert res.display_name == "test two updated"

    # delete third (a redirect)
    eg = quick_eg(api)
    api.delete_creator(eg.editgroup_id, c3.ident)
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c3.ident)
    assert res.state == "deleted"
    assert res.display_name is None

    # re-redirect third
    eg = quick_eg(api)
    api.update_creator(eg.editgroup_id, c3.ident, c3_redirect)
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c3.ident)
    assert res.state == "redirect"
    assert res.display_name == "test two updated"

    # delete second, then delete third
    eg = quick_eg(api)
    api.delete_creator(eg.editgroup_id, c2.ident)
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c3.ident)
    assert res.state == "redirect"
    assert res.display_name is None
    with pytest.raises(fatcat_openapi_client.rest.ApiException):
        res = api.lookup_creator(orcid=o2)
    eg = quick_eg(api)
    api.delete_creator(eg.editgroup_id, c3.ident)
    api.accept_editgroup(eg.editgroup_id)
    res = api.get_creator(c3.ident)
    assert res.state == "deleted"
    assert res.display_name is None

    # cleanup
    eg = quick_eg(api)
    api.delete_creator(eg.editgroup_id, c1.ident)
    # c2 already deleted
    # c3 already deleted
    api.accept_editgroup(eg.editgroup_id)


def test_self_redirect(api):

    c1 = CreatorEntity(display_name="test self-redirect")

    # create creator
    eg = quick_eg(api)
    c1 = api.get_creator(api.create_creator(eg.editgroup_id, c1).ident)
    api.accept_editgroup(eg.editgroup_id)

    # redirect first to itself; should error on PUT
    c1_redirect = CreatorEntity(redirect=c1.ident)
    eg = quick_eg(api)
    with pytest.raises(fatcat_openapi_client.rest.ApiException):
        api.update_creator(eg.editgroup_id, c1.ident, c1_redirect)


def test_wip_redirect(api):

    # create first
    c1 = CreatorEntity(display_name="test one")
    eg = quick_eg(api)
    c1 = api.get_creator(api.create_creator(eg.editgroup_id, c1).ident)
    api.accept_editgroup(eg.editgroup_id)

    # start creating second entity
    c2 = CreatorEntity(display_name="test two")
    eg = quick_eg(api)
    c2 = api.get_creator(api.create_creator(eg.editgroup_id, c2).ident)
    assert c2.state == "wip"

    # redirect first to second (should fail)
    eg = quick_eg(api)
    c1_redirect = CreatorEntity(redirect=c2.ident)
    try:
        api.update_creator(eg.editgroup_id, c1.ident, c1_redirect)
        assert False
    except fatcat_openapi_client.rest.ApiException as e:
        assert 400 <= e.status < 500
        assert "WIP" in e.body


def test_create_redirect(api):

    # create first
    c1 = CreatorEntity(display_name="test one")
    eg = quick_eg(api)
    c1 = api.get_creator(api.create_creator(eg.editgroup_id, c1).ident)
    api.accept_editgroup(eg.editgroup_id)

    # create second
    c2 = CreatorEntity(display_name="blah", redirect=c1.ident)
    eg = quick_eg(api)
    try:
        api.create_creator(eg.editgroup_id, c2)
        assert False
    except fatcat_openapi_client.rest.ApiException as e:
        assert 400 <= e.status < 500
        assert "redirect" in e.body

    # again with releases
    r1 = ReleaseEntity(title="test one", ext_ids=ReleaseExtIds())
    eg = quick_eg(api)
    r1 = api.get_release(api.create_release(eg.editgroup_id, r1).ident)
    api.accept_editgroup(eg.editgroup_id)
    r2 = ReleaseEntity(title="blah", redirect=c1.ident, ext_ids=ReleaseExtIds())
    eg = quick_eg(api)
    try:
        api.create_release(eg.editgroup_id, r2)
        assert False
    except fatcat_openapi_client.rest.ApiException as e:
        assert 400 <= e.status < 500
        assert "redirect" in e.body


def test_required_entity_fields(api):
    eg = quick_eg(api)

    # Creator
    try:
        c1 = CreatorEntity()
        api.create_creator(eg.editgroup_id, c1)
        assert False
    except fatcat_openapi_client.rest.ApiException as e:
        assert 400 <= e.status < 500
        assert "display_name" in e.body

    # Container
    try:
        c1 = ContainerEntity()
        api.create_container(eg.editgroup_id, c1)
        assert False
    except fatcat_openapi_client.rest.ApiException as e:
        assert 400 <= e.status < 500
        assert "name" in e.body

    # Release
    try:
        c1 = ReleaseEntity(ext_ids=ReleaseExtIds())
        api.create_release(eg.editgroup_id, c1)
        assert False
    except fatcat_openapi_client.rest.ApiException as e:
        assert 400 <= e.status < 500
        assert "title" in e.body

def test_revert_current_status(api):

    c1 = CreatorEntity(display_name="test updates")

    # create
    eg = quick_eg(api)
    c1 = api.get_creator(api.create_creator(eg.editgroup_id, c1).ident)
    api.accept_editgroup(eg.editgroup_id)

    # try to "revert" to current revision
    eg = quick_eg(api)
    c1_revert = CreatorEntity(revision=c1.revision)
    try:
        api.update_creator(eg.editgroup_id, c1.ident, c1_revert)
        assert False
    except fatcat_openapi_client.rest.ApiException as e:
        assert 400 <= e.status < 500
        assert "current" in e.body
