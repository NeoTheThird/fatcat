
import os
import sys
import json
from flask import render_template, make_response, send_from_directory, \
    request, url_for, abort, redirect, jsonify, session, flash, Response
from flask_login import login_required
from flask_wtf.csrf import CSRFError

from fatcat_openapi_client import EditgroupAnnotation
from fatcat_openapi_client.rest import ApiException
from fatcat_tools.transforms import *
from fatcat_tools.normal import *
from fatcat_web import app, api, auth_api, priv_api, mwoauth, Config
from fatcat_web.auth import handle_token_login, handle_logout, load_user, handle_ia_xauth, handle_wmoauth
from fatcat_web.cors import crossdomain
from fatcat_web.search import ReleaseQuery, GenericQuery, do_release_search, do_container_search, get_elastic_entity_stats, get_elastic_container_stats, get_elastic_container_histogram
from fatcat_web.entity_helpers import *
from fatcat_web.graphics import *
from fatcat_web.kafka import *
from fatcat_web.forms import SavePaperNowForm


### Generic Entity Views ####################################################

@app.route('/container/<ident>/history', methods=['GET'])
def container_history(ident):
    try:
        entity = api.get_container(ident)
        history = api.get_container_history(ident)
    except ApiException as ae:
        app.log.info(ae)
        abort(ae.status)
    return render_template('entity_history.html',
        entity_type="container",
        entity=entity,
        history=history)

@app.route('/creator/<ident>/history', methods=['GET'])
def creator_history(ident):
    try:
        entity = api.get_creator(ident)
        history = api.get_creator_history(ident)
    except ApiException as ae:
        abort(ae.status)
    return render_template('entity_history.html',
        entity_type="creator",
        entity=entity,
        history=history)

@app.route('/file/<ident>/history', methods=['GET'])
def file_history(ident):
    try:
        entity = api.get_file(ident)
        history = api.get_file_history(ident)
    except ApiException as ae:
        abort(ae.status)
    return render_template('entity_history.html',
        entity_type="file",
        entity=entity,
        history=history)

@app.route('/fileset/<ident>/history', methods=['GET'])
def fileset_history(ident):
    try:
        entity = api.get_fileset(ident)
        history = api.get_fileset_history(ident)
    except ApiException as ae:
        abort(ae.status)
    return render_template('entity_history.html',
        entity_type="fileset",
        entity=entity,
        history=history)

@app.route('/webcapture/<ident>/history', methods=['GET'])
def webcapture_history(ident):
    try:
        entity = api.get_webcapture(ident)
        history = api.get_webcapture_history(ident)
    except ApiException as ae:
        abort(ae.status)
    return render_template('entity_history.html',
        entity_type="webcapture",
        entity=entity,
        history=history)

@app.route('/release/<ident>/history', methods=['GET'])
def release_history(ident):
    try:
        entity = api.get_release(ident)
        history = api.get_release_history(ident)
    except ApiException as ae:
        abort(ae.status)
    return render_template('entity_history.html',
        entity_type="release",
        entity=entity,
        history=history)

@app.route('/work/<ident>/history', methods=['GET'])
def work_history(ident):
    try:
        entity = api.get_work(ident)
        history = api.get_work_history(ident)
    except ApiException as ae:
        abort(ae.status)
    return render_template('entity_history.html',
        entity_type="work",
        entity=entity,
        history=history)

def generic_lookup_view(entity_type, lookup_template, extid_types, lookup_lambda):
    extid = None
    for key in extid_types:
        if request.args.get(key):
            extid = key
            extid_value = request.args.get(extid)
            if extid_value:
                extid_value = extid_value.strip()
                break
    if extid is None:
        return render_template(lookup_template)
    try:
        resp = lookup_lambda({extid: extid_value})
    except ValueError:
        return make_response(
            render_template(lookup_template,
                lookup_key=extid,
                lookup_value=extid_value,
                lookup_error=400),
            400)
    except ApiException as ae:
        if ae.status == 404 or ae.status == 400:
            return make_response(
                render_template(lookup_template,
                    lookup_key=extid,
                    lookup_value=extid_value,
                    lookup_error=ae.status),
                ae.status)
        else:
            app.log.info(ae)
            abort(ae.status)
    return redirect('/{}/{}'.format(entity_type, resp.ident))

@app.route('/container/lookup', methods=['GET'])
def container_lookup():
    return generic_lookup_view(
        'container',
        'container_lookup.html',
        ('issnl', 'wikidata_qid'),
        lambda p: api.lookup_container(**p))

@app.route('/creator/lookup', methods=['GET'])
def creator_lookup():
    return generic_lookup_view(
        'creator',
        'creator_lookup.html',
        ('orcid', 'wikidata_qid'),
        lambda p: api.lookup_creator(**p))

@app.route('/file/lookup', methods=['GET'])
def file_lookup():
    return generic_lookup_view(
        'file',
        'file_lookup.html',
        ('md5', 'sha1', 'sha256'),
        lambda p: api.lookup_file(**p))

@app.route('/fileset/lookup', methods=['GET'])
def fileset_lookup():
    abort(404)

@app.route('/webcapture/lookup', methods=['GET'])
def webcapture_lookup():
    abort(404)

@app.route('/release/lookup', methods=['GET'])
def release_lookup():
    return generic_lookup_view(
        'release',
        'release_lookup.html',
        ('doi', 'wikidata_qid', 'pmid', 'pmcid', 'isbn13', 'jstor', 'arxiv',
         'core', 'ark', 'mag'),
        lambda p: api.lookup_release(**p))

@app.route('/work/lookup', methods=['GET'])
def work_lookup():
    abort(404)

### More Generic Entity Views ###############################################

def generic_entity_view(entity_type, ident, view_template):
    entity = generic_get_entity(entity_type, ident)

    if entity.state == "redirect":
        return redirect('/{}/{}'.format(entity_type, entity.redirect))
    elif entity.state == "deleted":
        return render_template('deleted_entity.html', entity_type=entity_type, entity=entity)

    metadata = entity.to_dict()
    metadata.pop('extra')
    entity._metadata = metadata

    return render_template(view_template, entity_type=entity_type, entity=entity, editgroup_id=None)

def generic_entity_revision_view(entity_type, revision_id, view_template):
    entity = generic_get_entity_revision(entity_type, revision_id)

    metadata = entity.to_dict()
    metadata.pop('extra')
    entity._metadata = metadata

    return render_template(view_template, entity_type=entity_type, entity=entity, editgroup_id=None)

def generic_editgroup_entity_view(editgroup_id, entity_type, ident, view_template):
    try:
        editgroup = api.get_editgroup(editgroup_id)
    except ApiException as ae:
        abort(ae.status)

    entity, edit = generic_get_editgroup_entity(editgroup, entity_type, ident)

    if entity.state == "deleted":
        return render_template('deleted_entity.html', entity=entity,
            entity_type=entity_type, editgroup=editgroup)

    metadata = entity.to_dict()
    metadata.pop('extra')
    entity._metadata = metadata

    return render_template(view_template, entity_type=entity_type, entity=entity, editgroup=editgroup)


@app.route('/container/<ident>', methods=['GET'])
def container_view(ident):
    return generic_entity_view('container', ident, 'container_view.html')

@app.route('/container_<ident>', methods=['GET'])
def container_underscore_view(ident):
    return redirect('/container/{}'.format(ident))

@app.route('/container/<ident>/coverage', methods=['GET'])
def container_view_coverage(ident):
    return generic_entity_view('container', ident, 'container_view_coverage.html')

@app.route('/container/<ident>/metadata', methods=['GET'])
def container_view_metadata(ident):
    return generic_entity_view('container', ident, 'entity_view_metadata.html')

@app.route('/container/rev/<revision_id>', methods=['GET'])
def container_revision_view(revision_id):
    return generic_entity_revision_view('container', revision_id, 'container_view.html')

@app.route('/container/rev/<revision_id>/metadata', methods=['GET'])
def container_revision_view_metadata(revision_id):
    return generic_entity_revision_view('container', revision_id, 'entity_view_metadata.html')

@app.route('/editgroup/<editgroup_id>/container/<ident>', methods=['GET'])
def container_editgroup_view(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'container', ident, 'container_view.html')

@app.route('/editgroup/<editgroup_id>/container/<ident>/metadata', methods=['GET'])
def container_editgroup_view_metadata(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'container', ident, 'entity_view_metadata.html')


@app.route('/creator/<ident>', methods=['GET'])
def creator_view(ident):
    return generic_entity_view('creator', ident, 'creator_view.html')

@app.route('/creator_<ident>', methods=['GET'])
def creator_underscore_view(ident):
    return redirect('/creator/{}'.format(ident))

@app.route('/creator/<ident>/metadata', methods=['GET'])
def creator_view_metadata(ident):
    return generic_entity_view('creator', ident, 'entity_view_metadata.html')

@app.route('/creator/rev/<revision_id>', methods=['GET'])
def creator_revision_view(revision_id):
    return generic_entity_revision_view('creator', revision_id, 'creator_view.html')

@app.route('/creator/rev/<revision_id>/metadata', methods=['GET'])
def creator_revision_view_metadata(revision_id):
    return generic_entity_revision_view('creator', revision_id, 'entity_view_metadata.html')

@app.route('/editgroup/<editgroup_id>/creator/<ident>', methods=['GET'])
def creator_editgroup_view(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'creator', ident, 'creator_view.html')

@app.route('/editgroup/<editgroup_id>/creator/<ident>/metadata', methods=['GET'])
def creator_editgroup_view_metadata(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'creator', ident, 'entity_view_metadata.html')


@app.route('/file/<ident>', methods=['GET'])
def file_view(ident):
    return generic_entity_view('file', ident, 'file_view.html')

@app.route('/file_<ident>', methods=['GET'])
def file_underscore_view(ident):
    return redirect('/file/{}'.format(ident))

@app.route('/file/<ident>/metadata', methods=['GET'])
def file_view_metadata(ident):
    return generic_entity_view('file', ident, 'entity_view_metadata.html')

@app.route('/file/rev/<revision_id>', methods=['GET'])
def file_revision_view(revision_id):
    return generic_entity_revision_view('file', revision_id, 'file_view.html')

@app.route('/file/rev/<revision_id>/metadata', methods=['GET'])
def file_revision_view_metadata(revision_id):
    return generic_entity_revision_view('file', revision_id, 'entity_view_metadata.html')

@app.route('/editgroup/<editgroup_id>/file/<ident>', methods=['GET'])
def file_editgroup_view(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'file', ident, 'file_view.html')

@app.route('/editgroup/<editgroup_id>/file/<ident>/metadata', methods=['GET'])
def file_editgroup_view_metadata(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'file', ident, 'entity_view_metadata.html')


@app.route('/fileset/<ident>', methods=['GET'])
def fileset_view(ident):
    return generic_entity_view('fileset', ident, 'fileset_view.html')

@app.route('/fileset_<ident>', methods=['GET'])
def fileset_underscore_view(ident):
    return redirect('/fileset/{}'.format(ident))

@app.route('/fileset/<ident>/metadata', methods=['GET'])
def fileset_view_metadata(ident):
    return generic_entity_view('fileset', ident, 'entity_view_metadata.html')

@app.route('/fileset/rev/<revision_id>', methods=['GET'])
def fileset_revision_view(revision_id):
    return generic_entity_revision_view('fileset', revision_id, 'fileset_view.html')

@app.route('/fileset/rev/<revision_id>/metadata', methods=['GET'])
def fileset_revision_view_metadata(revision_id):
    return generic_entity_revision_view('fileset', revision_id, 'entity_view_metadata.html')

@app.route('/editgroup/<editgroup_id>/fileset/<ident>', methods=['GET'])
def fileset_editgroup_view(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'fileset', ident, 'fileset_view.html')

@app.route('/editgroup/<editgroup_id>/fileset/<ident>/metadata', methods=['GET'])
def fileset_editgroup_view_metadata(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'fileset', ident, 'entity_view_metadata.html')


@app.route('/webcapture/<ident>', methods=['GET'])
def webcapture_view(ident):
    return generic_entity_view('webcapture', ident, 'webcapture_view.html')

@app.route('/webcapture_<ident>', methods=['GET'])
def webcapture_underscore_view(ident):
    return redirect('/webcapture/{}'.format(ident))

@app.route('/webcapture/<ident>/metadata', methods=['GET'])
def webcapture_view_metadata(ident):
    return generic_entity_view('webcapture', ident, 'entity_view_metadata.html')

@app.route('/webcapture/rev/<revision_id>', methods=['GET'])
def webcapture_revision_view(revision_id):
    return generic_entity_revision_view('webcapture', revision_id, 'webcapture_view.html')

@app.route('/webcapture/rev/<revision_id>/metadata', methods=['GET'])
def webcapture_revision_view_metadata(revision_id):
    return generic_entity_revision_view('webcapture', revision_id, 'entity_view_metadata.html')

@app.route('/editgroup/<editgroup_id>/webcapture/<ident>', methods=['GET'])
def webcapture_editgroup_view(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'webcapture', ident, 'webcapture_view.html')

@app.route('/editgroup/<editgroup_id>/webcapture/<ident>/metadata', methods=['GET'])
def webcapture_editgroup_view_metadata(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'webcapture', ident, 'entity_view_metadata.html')


@app.route('/release/<ident>', methods=['GET'])
def release_view(ident):
    return generic_entity_view('release', ident, 'release_view.html')

@app.route('/release_<ident>', methods=['GET'])
def release_underscore_view(ident):
    return redirect('/release/{}'.format(ident))

@app.route('/release/<ident>/contribs', methods=['GET'])
def release_view_contribs(ident):
    return generic_entity_view('release', ident, 'release_view_contribs.html')

@app.route('/release/<ident>/references', methods=['GET'])
def release_view_references(ident):
    return generic_entity_view('release', ident, 'release_view_references.html')

@app.route('/release/<ident>/metadata', methods=['GET'])
def release_view_metadata(ident):
    return generic_entity_view('release', ident, 'entity_view_metadata.html')

@app.route('/release/rev/<revision_id>', methods=['GET'])
def release_revision_view(revision_id):
    return generic_entity_revision_view('release', revision_id, 'release_view.html')

@app.route('/release/rev/<revision_id>/contribs', methods=['GET'])
def release_revision_view_contribs(revision_id):
    return generic_entity_revision_view('release', revision_id, 'release_view_contribs.html')

@app.route('/release/rev/<revision_id>/references', methods=['GET'])
def release_revision_view_references(revision_id):
    return generic_entity_revision_view('release', revision_id, 'release_view_references.html')

@app.route('/release/rev/<revision_id>/metadata', methods=['GET'])
def release_revision_view_metadata(revision_id):
    return generic_entity_revision_view('release', revision_id, 'entity_view_metadata.html')

@app.route('/editgroup/<editgroup_id>/release/<ident>', methods=['GET'])
def release_editgroup_view(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'release', ident, 'release_view.html')

@app.route('/editgroup/<editgroup_id>/release/<ident>/contribs', methods=['GET'])
def release_editgroup_view_contribs(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'release', ident, 'release_view_contribs.html')

@app.route('/editgroup/<editgroup_id>/release/<ident>/references', methods=['GET'])
def release_editgroup_view_references(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'release', ident, 'release_view_references.html')

@app.route('/editgroup/<editgroup_id>/release/<ident>/metadata', methods=['GET'])
def release_editgroup_view_metadata(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'release', ident, 'entity_view_metadata.html')


@app.route('/work/<ident>', methods=['GET'])
def work_view(ident):
    return generic_entity_view('work', ident, 'work_view.html')

@app.route('/work_<ident>', methods=['GET'])
def work_underscore_view(ident):
    return redirect('/work/{}'.format(ident))

@app.route('/work/<ident>/metadata', methods=['GET'])
def work_view_metadata(ident):
    return generic_entity_view('work', ident, 'entity_view_metadata.html')

@app.route('/work/rev/<revision_id>', methods=['GET'])
def work_revision_view(revision_id):
    return generic_entity_revision_view('work', revision_id, 'work_view.html')

@app.route('/work/rev/<revision_id>/metadata', methods=['GET'])
def work_revision_view_metadata(revision_id):
    return generic_entity_revision_view('work', revision_id, 'entity_view_metadata.html')

@app.route('/editgroup/<editgroup_id>/work/<ident>', methods=['GET'])
def work_editgroup_view(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'work', ident, 'work_view.html')

@app.route('/editgroup/<editgroup_id>/work/<ident>/metadata', methods=['GET'])
def work_editgroup_view_metadata(editgroup_id, ident):
    return generic_editgroup_entity_view(editgroup_id, 'work', ident, 'entity_view_metadata.html')


### Views ###################################################################

@app.route('/editgroup/<ident>', methods=['GET'])
def editgroup_view(ident):
    try:
        eg = api.get_editgroup(str(ident))
        eg.editor = api.get_editor(eg.editor_id)
        eg.annotations = api.get_editgroup_annotations(eg.editgroup_id, expand="editors")
    except ApiException as ae:
        abort(ae.status)
    # TODO: idomatic check for login?
    auth_to = dict(
        submit=False,
        accept=False,
        edit=False,
        annotate=False,
    )
    if session.get('editor'):
        user = load_user(session['editor']['editor_id'])
        auth_to['annotate'] = True
        if user.is_admin or user.editor_id == eg.editor_id:
            auth_to['submit'] = True
            auth_to['edit'] = True
        if user.is_admin:
            auth_to['accept'] = True
    return render_template('editgroup_view.html', editgroup=eg,
        auth_to=auth_to)

@app.route('/editgroup/<ident>/annotation', methods=['POST'])
@login_required
def editgroup_create_annotation(ident):
    if not app.testing:
        app.csrf.protect()
    comment_markdown = request.form.get('comment_markdown')
    if not comment_markdown:
        app.log.info("empty comment field")
        abort(400)
    # on behalf of user...
    user_api = auth_api(session['api_token'])
    try:
        eg = user_api.get_editgroup(str(ident))
        if eg.changelog_index:
            flash("Editgroup already accepted")
            abort(400)
        ega = EditgroupAnnotation(
            comment_markdown=comment_markdown,
            extra=None,
        )
        user_api.create_editgroup_annotation(eg.editgroup_id, ega)
    except ApiException as ae:
        app.log.info(ae)
        abort(ae.status)
    return redirect('/editgroup/{}'.format(ident))

@app.route('/editgroup/<ident>/accept', methods=['POST'])
@login_required
def editgroup_accept(ident):
    if not app.testing:
        app.csrf.protect()
    # on behalf of user...
    user_api = auth_api(session['api_token'])
    try:
        eg = user_api.get_editgroup(str(ident))
        if eg.changelog_index:
            flash("Editgroup already accepted")
            abort(400)
        user_api.accept_editgroup(str(ident))
    except ApiException as ae:
        app.log.info(ae)
        abort(ae.status)
    return redirect('/editgroup/{}'.format(ident))

@app.route('/editgroup/<ident>/unsubmit', methods=['POST'])
@login_required
def editgroup_unsubmit(ident):
    if not app.testing:
        app.csrf.protect()
    # on behalf of user...
    user_api = auth_api(session['api_token'])
    try:
        eg = user_api.get_editgroup(str(ident))
        if eg.changelog_index:
            flash("Editgroup already accepted")
            abort(400)
        user_api.update_editgroup(eg.editgroup_id, eg, submit=False)
    except ApiException as ae:
        app.log.info(ae)
        abort(ae.status)
    return redirect('/editgroup/{}'.format(ident))

@app.route('/editgroup/<ident>/submit', methods=['POST'])
@login_required
def editgroup_submit(ident):
    if not app.testing:
        app.csrf.protect()
    # on behalf of user...
    print("submitting...")
    user_api = auth_api(session['api_token'])
    try:
        eg = user_api.get_editgroup(str(ident))
        if eg.changelog_index:
            flash("Editgroup already accepted")
            abort(400)
        user_api.update_editgroup(eg.editgroup_id, eg, submit=True)
    except ApiException as ae:
        print(ae)
        app.log.info(ae)
        abort(ae.status)
    return redirect('/editgroup/{}'.format(ident))

@app.route('/editor/<ident>', methods=['GET'])
def editor_view(ident):
    try:
        entity = api.get_editor(ident)
    except ApiException as ae:
        abort(ae.status)
    return render_template('editor_view.html', editor=entity)

@app.route('/editor/<ident>/editgroups', methods=['GET'])
def editor_editgroups(ident):
    try:
        editor = api.get_editor(ident)
        editgroups = api.get_editor_editgroups(ident, limit=50)
        # cheaper than API-side expand?
        for eg in editgroups:
            eg.editor = editor
    except ApiException as ae:
        abort(ae.status)
    return render_template('editor_editgroups.html', editor=editor,
        editgroups=editgroups)

@app.route('/editor/<ident>/annotations', methods=['GET'])
def editor_annotations(ident):
    try:
        editor = api.get_editor(ident)
        annotations = api.get_editor_annotations(ident, limit=50)
    except ApiException as ae:
        abort(ae.status)
    return render_template('editor_annotations.html', editor=editor,
        annotations=annotations)

@app.route('/changelog', methods=['GET'])
def changelog_view():
    try:
        #limit = int(request.args.get('limit', 10))
        entries = api.get_changelog() # TODO: expand="editors"
    except ApiException as ae:
        abort(ae.status)
    return render_template('changelog.html', entries=entries)

@app.route('/changelog/<int:index>', methods=['GET'])
def changelog_entry_view(index):
    try:
        entry = api.get_changelog_entry(int(index))
        entry.editgroup.editor = api.get_editor(entry.editgroup.editor_id)
        entry.editgroup.annotations = \
            api.get_editgroup_annotations(entry.editgroup_id, expand="editors")
    except ApiException as ae:
        abort(ae.status)
    return render_template('changelog_view.html', entry=entry, editgroup=entry.editgroup)

@app.route('/reviewable', methods=['GET'])
def reviewable_view():
    try:
        #limit = int(request.args.get('limit', 10))
        entries = api.get_editgroups_reviewable(expand="editors")
    except ApiException as ae:
        abort(ae.status)
    return render_template('editgroup_reviewable.html', entries=entries)

@app.route('/release/<ident>/save', methods=['GET', 'POST'])
def release_save(ident):

    form = SavePaperNowForm()

    # lookup release ident, ensure it exists
    try:
        release = api.get_release(ident)
    except ApiException as ae:
        abort(ae.status)

    if not Config.KAFKA_PIXY_ENDPOINT:
        return render_template('release_save.html', entity=release, form=form, spn_status='not-configured'), 501

    if form.is_submitted():
        if form.validate_on_submit():
            # got a valid spn request! try to send to kafka-pixy
            msg = form.to_ingest_request(release, ingest_request_source="savepapernow-web")
            try:
                kafka_pixy_produce(
                    Config.KAFKA_SAVEPAPERNOW_TOPIC,
                    json.dumps(msg, sort_keys=True),
                )
            except Exception as e:
                print(e, file=sys.stderr)
                return render_template('release_save.html', entity=release, form=form, spn_status='kafka-error'), 500
            return render_template('release_save.html', entity=release, form=form, spn_status='success'), 200
        elif form.errors:
            return render_template('release_save.html', entity=release, form=form), 400

    # form was not submitted; populate defaults
    if release.release_stage:
        form.release_stage.data = release.release_stage
    if release.ext_ids.doi:
        form.base_url.data = "https://doi.org/{}".format(release.ext_ids.doi)
    elif release.ext_ids.arxiv:
        form.base_url.data = "https://arxiv.org/pdf/{}.pdf".format(release.ext_ids.arxiv)
    elif release.ext_ids.pmcid:
        form.base_url.data = "http://europepmc.org/backend/ptpmcrender.fcgi?accid={}&blobtype=pdf".format(release.ext_ids.pmcid)
    return render_template('release_save.html', entity=release, form=form), 200

### Search ##################################################################

@app.route('/search', methods=['GET', 'POST'])
def generic_search():
    if not 'q' in request.args.keys():
        return redirect('/release/search')
    query = request.args.get('q').strip()

    if len(query.split()) != 1:
        # multi-term? must be a real search
        return redirect(url_for('release_search', q=query))

    if clean_doi(query):
        return redirect(url_for('release_lookup', doi=clean_doi(query)))
    if clean_pmcid(query):
        return redirect(url_for('release_lookup', pmcid=clean_pmcid(query)))
    if clean_sha1(query):
        return redirect(url_for('file_lookup', sha1=clean_sha1(query)))
    if clean_sha256(query):
        return redirect(url_for('file_lookup', sha256=clean_sha256(query)))
    if clean_issn(query):
        return redirect(url_for('container_lookup', issnl=clean_issn(query)))
    if clean_isbn13(query):
        return redirect(url_for('release_lookup', isbn13=clean_isbn13(query)))
    if clean_arxiv_id(query):
        return redirect(url_for('release_lookup', arxiv=clean_arxiv_id(query)))
    if clean_orcid(query):
        return redirect(url_for('creator_lookup', orcid=clean_orcid(query)))

    return redirect(url_for('release_search', q=query))

@app.route('/release/search', methods=['GET', 'POST'])
def release_search():

    if 'q' not in request.args.keys():
        return render_template('release_search.html', query=ReleaseQuery(), found=None)

    query = ReleaseQuery.from_args(request.args)
    found = do_release_search(query)
    return render_template('release_search.html', query=query, found=found)

@app.route('/container/search', methods=['GET', 'POST'])
def container_search():

    if 'q' not in request.args.keys():
        return render_template('container_search.html', query=GenericQuery(), found=None)

    query = GenericQuery.from_args(request.args)
    found = do_container_search(query)
    return render_template('container_search.html', query=query, found=found)

def get_changelog_stats():
    stats = {}
    latest_changelog = api.get_changelog(limit=1)[0]
    stats['changelog'] = {"latest": {
        "index": latest_changelog.index,
        "timestamp": latest_changelog.timestamp.isoformat(),
    }}
    return stats

@app.route('/stats', methods=['GET'])
def stats_page():
    try:
        stats = get_elastic_entity_stats()
        stats.update(get_changelog_stats())
    except Exception as ae:
        app.log.error(ae)
        abort(503)
    return render_template('stats.html', stats=stats)

### Pseudo-APIs #############################################################

@app.route('/stats.json', methods=['GET', 'OPTIONS'])
@crossdomain(origin='*',headers=['access-control-allow-origin','Content-Type'])
def stats_json():
    try:
        stats = get_elastic_entity_stats()
        stats.update(get_changelog_stats())
    except Exception as ae:
        app.log.error(ae)
        abort(503)
    return jsonify(stats)

@app.route('/container/issnl/<issnl>/stats.json', methods=['GET', 'OPTIONS'])
@crossdomain(origin='*',headers=['access-control-allow-origin','Content-Type'])
def container_issnl_stats(issnl):
    if not (len(issnl) == 9 and issnl[4] == '-'):
        flash("Not a valid ISSN-L: {}".format(issnl))
        abort(400)
    try:
        container = api.lookup_container(issnl=issnl)
    except ApiException as ae:
        abort(ae.status)
    try:
        stats = get_elastic_container_stats(container.ident, issnl=container.issnl)
    except Exception as ae:
        app.log.error(ae)
        abort(503)
    return jsonify(stats)

@app.route('/container/<ident>/stats.json', methods=['GET', 'OPTIONS'])
@crossdomain(origin='*',headers=['access-control-allow-origin','Content-Type'])
def container_ident_stats(ident):
    try:
        container = api.get_container(ident)
    except ApiException as ae:
        abort(ae.status)
    try:
        stats = get_elastic_container_stats(container.ident, issnl=container.issnl)
    except Exception as ae:
        app.log.error(ae)
        abort(503)
    return jsonify(stats)

@app.route('/container/<ident>/ia_coverage_years.json', methods=['GET', 'OPTIONS'])
@crossdomain(origin='*',headers=['access-control-allow-origin','Content-Type'])
def container_ident_ia_coverage_years_json(ident):
    try:
        container = api.get_container(ident)
    except ApiException as ae:
        abort(ae.status)
    try:
        histogram = get_elastic_container_histogram(container.ident)
    except Exception as ae:
        app.log.error(ae)
        abort(503)
    histogram = [dict(year=h[0], in_ia=h[1], count=h[2]) for h in histogram]
    return jsonify({'container_id': ident, "histogram": histogram})

@app.route('/container/<ident>/ia_coverage_years.svg', methods=['GET', 'OPTIONS'])
@crossdomain(origin='*',headers=['access-control-allow-origin','Content-Type'])
def container_ident_ia_coverage_years_svg(ident):
    try:
        container = api.get_container(ident)
    except ApiException as ae:
        abort(ae.status)
    try:
        histogram = get_elastic_container_histogram(container.ident)
    except Exception as ae:
        app.log.error(ae)
        abort(503)
    return ia_coverage_histogram(histogram).render_response()

@app.route('/release/<ident>.bib', methods=['GET'])
def release_bibtex(ident):
    try:
        entity = api.get_release(ident)
    except ApiException as ae:
        abort(ae.status)
    csl = release_to_csl(entity)
    bibtex = citeproc_csl(csl, 'bibtex')
    return Response(bibtex, mimetype="text/plain")

@app.route('/release/<ident>/citeproc', methods=['GET'])
def release_citeproc(ident):
    style = request.args.get('style', 'harvard1')
    is_html = request.args.get('html', False)
    if is_html and is_html.lower() in ('yes', '1', 'true', 'y', 't'):
        is_html = True
    else:
        is_html = False

    try:
        entity = api.get_release(ident)
    except ApiException as ae:
        abort(ae.status)
    csl = release_to_csl(entity)
    cite = citeproc_csl(csl, style, is_html)
    if is_html:
        return Response(cite)
    elif style == "csl-json":
        return jsonify(json.loads(cite))
    else:
        return Response(cite, mimetype="text/plain")

@app.route('/health.json', methods=['GET', 'OPTIONS'])
@crossdomain(origin='*',headers=['access-control-allow-origin','Content-Type'])
def health_json():
    return jsonify({'ok': True})


### Auth ####################################################################

@app.route('/auth/login')
def login():
    # show the user a list of login options
    if not priv_api:
        flash("This web interface not configured with credentials to actually allow login (other than via token)")
    return render_template('auth_login.html')

@app.route('/auth/ia/login', methods=['GET', 'POST'])
def ia_xauth_login():
    if 'email' in request.form:
        # if a login attempt...
        return handle_ia_xauth(request.form.get('email'), request.form.get('password'))
    # else show form
    return render_template('auth_ia_login.html')

@app.route('/auth/token_login', methods=['GET', 'POST'])
def token_login():
    # show the user a list of login options
    if 'token' in request.args:
        return handle_token_login(request.args.get('token'))
    if 'token' in request.form:
        return handle_token_login(request.form.get('token'))
    return render_template('auth_token_login.html')

@app.route('/auth/change_username', methods=['POST'])
@login_required
def change_username():
    if not app.testing:
        app.csrf.protect()
    # show the user a list of login options
    if not 'username' in request.form:
        abort(400)
    # on behalf of user...
    user_api = auth_api(session['api_token'])
    try:
        editor = user_api.get_editor(session['editor']['editor_id'])
        editor.username = request.form['username']
        editor = user_api.update_editor(editor.editor_id, editor)
    except ApiException as ae:
        app.log.info(ae)
        abort(ae.status)
    # update our session
    session['editor'] = editor.to_dict()
    load_user(editor.editor_id)
    flash("Username updated successfully")
    return redirect('/auth/account')

@app.route('/auth/create_token', methods=['POST'])
@login_required
def create_auth_token():
    if not app.testing:
        app.csrf.protect()

    duration_seconds = request.form.get('duration_seconds', None)
    if duration_seconds:
        try:
            duration_seconds = int(duration_seconds)
            assert duration_seconds >= 1
        except (ValueError, AssertionError):
            flash("duration_seconds must be a positive non-zero integer")
            abort(400)

    # check user's auth. api_token and editor_id are signed together in session
    # cookie, so if api_token is valid editor_id is assumed to match. If that
    # wasn't true, users could manipulate session cookies and create tokens for
    # any user
    user_api = auth_api(session['api_token'])
    resp = user_api.auth_check()
    assert(resp.success)

    # generate token using *superuser* privs
    editor_id = session['editor']['editor_id']
    try:
        resp = priv_api.create_auth_token(editor_id,
            duration_seconds=duration_seconds)
    except ApiException as ae:
        app.log.info(ae)
        abort(ae.status)
    return render_template('auth_token.html', auth_token=resp.token)

@app.route('/auth/logout')
def logout():
    handle_logout()
    return render_template('auth_logout.html')

@app.route('/auth/account')
@login_required
def auth_account():
    # auth check on account page
    user_api = auth_api(session['api_token'])
    resp = user_api.auth_check()
    assert(resp.success)
    editor = user_api.get_editor(session['editor']['editor_id'])
    session['editor'] = editor.to_dict()
    load_user(editor.editor_id)
    return render_template('auth_account.html')

@app.route('/auth/wikipedia/auth')
def wp_oauth_rewrite():
    """
    This is a dirty hack to rewrite '/auth/wikipedia/auth' to '/auth/wikipedia/oauth-callback'
    """
    return redirect(b"/auth/wikipedia/oauth-callback?" + request.query_string, 307)

@app.route('/auth/wikipedia/finish-login')
def wp_oauth_finish_login():
    wp_username = mwoauth.get_current_user(cached=True)
    assert(wp_username)
    return handle_wmoauth(wp_username)


### Static Routes ###########################################################

@app.errorhandler(404)
def page_not_found(e):
    return render_template('404.html'), 404

@app.errorhandler(401)
@app.errorhandler(403)
def page_not_authorized(e):
    return render_template('403.html'), 403

@app.errorhandler(405)
def page_method_not_allowed(e):
    return render_template('405.html'), 405

@app.errorhandler(400)
def page_bad_request(e):
    return render_template('400.html'), 400

@app.errorhandler(409)
def page_edit_conflict(e):
    return render_template('409.html'), 409

@app.errorhandler(500)
def page_server_error(e):
    return render_template('500.html'), 500

@app.errorhandler(502)
@app.errorhandler(503)
@app.errorhandler(504)
def page_server_down(e):
    return render_template('503.html'), 503

@app.errorhandler(CSRFError)
def page_csrf_error(e):
    return render_template('csrf_error.html', reason=e.description), 400

@app.route('/', methods=['GET'])
def page_home():
    return render_template('home.html')

@app.route('/about', methods=['GET'])
def page_about():
    return render_template('about.html')

@app.route('/rfc', methods=['GET'])
def page_rfc():
    return render_template('rfc.html')

@app.route('/robots.txt', methods=['GET'])
def robots():
    return send_from_directory(os.path.join(app.root_path, 'static'),
                               'robots.txt',
                               mimetype='text/plain')
