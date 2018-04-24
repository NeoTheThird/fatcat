
import os
import json
from flask import Flask, render_template, send_from_directory, request, \
    url_for, abort, g, redirect, jsonify, session
from fatcat import app, db, api


### Views ###################################################################

@app.route('/work/create', methods=['GET'])
def work_create():
    return render_template('work_add.html')

@app.route('/work/random', methods=['GET'])
def work_random():
    rv = api.api_work_random()
    ident = rv.location.split('/')[-1]
    return redirect("/work/{}".format(ident))

@app.route('/work/<int:ident>', methods=['GET'])
def work_view(ident):
    rv = api.api_work_get(ident)
    entity = json.loads(rv.data.decode('utf-8'))
    return render_template('work_view.html', work=entity)

@app.route('/release/<int:ident>', methods=['GET'])
def release_view(ident):
    rv = api.api_release_get(ident)
    entity = json.loads(rv.data.decode('utf-8'))
    return render_template('release_view.html', release=entity)

@app.route('/release/random', methods=['GET'])
def release_random():
    rv = api.api_release_random()
    ident = rv.location.split('/')[-1]
    return redirect("/release/{}".format(ident))

@app.route('/creator/<int:ident>', methods=['GET'])
def creator_view(ident):
    rv = api.api_creator_get(ident)
    entity = json.loads(rv.data.decode('utf-8'))
    return render_template('creator_view.html', creator=entity)

@app.route('/container/<int:ident>', methods=['GET'])
def container_view(ident):
    rv = api.api_container_get(ident)
    entity = json.loads(rv.data.decode('utf-8'))
    return render_template('container_view.html', container=entity)

@app.route('/file/<int:ident>', methods=['GET'])
def file_view(ident):
    rv = api.api_file_get(ident)
    entity = json.loads(rv.data.decode('utf-8'))
    return render_template('file_view.html', file=entity)

@app.route('/editgroup/<int:ident>', methods=['GET'])
def editgroup_view(ident):
    rv = api.api_editgroup_get(ident)
    entity = json.loads(rv.data.decode('utf-8'))
    return render_template('editgroup_view.html', editgroup=entity)

@app.route('/editgroup/current', methods=['GET'])
def editgroup_current():
    eg = api.get_or_create_editgroup()
    return redirect('/editgroup/{}'.format(eg.id))

@app.route('/editor/<username>', methods=['GET'])
def editor_view(username):
    rv = api.api_editor_get(username)
    entity = json.loads(rv.data.decode('utf-8'))
    return render_template('editor_view.html', editor=entity)

@app.route('/editor/<username>/changelog', methods=['GET'])
def editor_changelog(username):
    rv = api.api_editor_get(username)
    editor = json.loads(rv.data.decode('utf-8'))
    rv = api.api_editor_changelog(username)
    changelog_entries = json.loads(rv.data.decode('utf-8'))
    return render_template('editor_changelog.html', editor=editor,
        changelog_entries=changelog_entries)


### Static Routes ###########################################################

@app.route('/', methods=['GET'])
def homepage():
    return render_template('home.html')

@app.route('/about', methods=['GET'])
def aboutpage():
    return render_template('about.html')

@app.route('/robots.txt', methods=['GET'])
def robots():
    return send_from_directory(os.path.join(app.root_path, 'static'),
                               'robots.txt',
                               mimetype='text/plain')

@app.route('/health', methods=['GET'])
def health():
    return jsonify({'ok': True})
