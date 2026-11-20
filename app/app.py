#!/usr/bin/env python3
from subprocess import Popen, PIPE
from functools import wraps
import atexit
from flask import Flask, render_template, request, jsonify, escape

from music_backend import sched
from deezer import deezer_search
from configuration import config

app = Flask(__name__)


# user input validation
def validate_schema(*parameters_to_check):
    def decorator(f):
        @wraps(f)
        def wrapper(*args, **kw):
            j = request.get_json(force=True)
            print("User request: {} with {}".format(request.path, j))
            # check if all parameters are supplied by the user
            if set(j.keys()) != set(parameters_to_check):
                return jsonify({"error": 'parameters not fitting. Required: {}'.format(parameters_to_check)}), 400
            if "type" in j.keys():
                if j['type'] not in ["all", "album", "track", "album_track", "playlist", "artist", "artist_track"]:
                    return jsonify({"error": "type must be album, track, album_track or playlist"}), 400
            if "music_id" in j.keys():
                if type(j['music_id']) != int:
                    return jsonify({"error": "music_id must be a integer"}), 400
            if "add_to_playlist" in j.keys():
                if type(j['add_to_playlist']) != bool:
                    return jsonify({"error": "add_to_playlist must be a boolean"}), 400
            if "create_zip" in j.keys():
                if type(j['create_zip']) != bool:
                    return jsonify({"error": "create_zip must be a boolean"}), 400
            if "query" in j.keys():
                if type(j['query']) != str:
                    return jsonify({"error": "query is not a string"}), 400
                if j['query'] == "":
                    return jsonify({"error": "query is empty"}), 400
            if "playlist_url" in j.keys():
                if type(j['playlist_url']) != str:
                    return jsonify({"error": "playlist_url is not a string"}), 400
                if len(j['playlist_url'].strip()) == 0:
                    return jsonify({"error": "playlist_url is empty"}), 400
            if "user_id" in j.keys():
                if type(j['user_id']) != str or not j['user_id'].isnumeric():
                    return jsonify({"error": "user_id must be a numeric string"}), 400
            return f(*args, **kw)
        return wrapper
    return decorator


@app.route("/", methods=["GET"])
def index():
    query = request.args.get('query')
    print(f'query={query}')
    return render_template("index.html",
                           query=query,
                           api_root=config["http"]["api_root"],
                           static_root=config["http"]["static_root"],
                           use_mpd=str(config['mpd'].getboolean('use_mpd')).lower())

@app.route('/find', methods=['GET'])
def find():
    query = request.args.get('query')
    print(f'query={query}')
    return render_template("index.html",
                           query=query,
                           api_root=config["http"]["api_root"],
                           static_root=config["http"]["static_root"],
                           use_mpd=str(config['mpd'].getboolean('use_mpd')).lower())


@app.route("/debug")
def show_debug():
    p = Popen(config["debug"]["command"], shell=True, stdout=PIPE)
    p.wait()
    stdout, __ = p.communicate()
    return jsonify({'debug_msg': stdout.decode()})


@app.route('/queue', methods=['GET'])
def show_queue():
    """
    shows queued tasks
    return:
        json: [ { tasks } ]
    """
    results = [
        {'id': id(task),
         'description': escape(task.description),
         #'command': task.fn_name,
         'args': escape(task.kwargs),
         'state': escape(task.state),
         'result': escape(task.result),
         'exception': escape(str(task.exception)),
         'progress': [task.progress, task.progress_maximum]
        } for task in sched.all_tasks
    ]
    return jsonify(results)


@app.route('/search', methods=['POST'])
@validate_schema("type", "query")
def search():
    """
    searches for available music in the Deezer library
    para:
        type: track|album|album_track|playlist
        query: search query
    return:
        json: [ { artist, id, (title|album) } ]
    """
    user_input = request.get_json(force=True)
    results = deezer_search(user_input['query'], user_input['type'])
    return jsonify(results)


@app.route('/download', methods=['POST'])
@validate_schema("type", "music_id", "add_to_playlist", "create_zip")
def deezer_download_song_or_album():
    """
    downloads a song or an album from Deezer to the dir specified in settings.py
    para:
        type: album|track
        music_id: id of the album or track (int)
        add_to_playlist: True|False (add to mpd playlist)
        create_zip: True|False (create a zip for the album)
    """
    user_input = request.get_json(force=True)
    desc = "Downloading song {}".format(user_input['type'])
    if user_input['type'] == "track":
        task = sched.enqueue_task(desc, "download_deezer_song_and_queue",
                                  track_id=user_input['music_id'],
                                  add_to_playlist=user_input['add_to_playlist'])
    else:
        task = sched.enqueue_task(desc, "download_deezer_album_and_queue_and_zip",
                                  album_id=user_input['music_id'],
                                  add_to_playlist=user_input['add_to_playlist'],
                                  create_zip=user_input['create_zip'])
    return jsonify({"task_id": id(task), })


@app.route('/playlist/deezer', methods=['POST'])
@validate_schema("playlist_url", "add_to_playlist", "create_zip")
def deezer_playlist_download():
    """
    downloads songs of a public Deezer playlist.
    A directory with the name of the playlist will be created.
    para:
        playlist_url: link to a public Deezer playlist (the id of the playlist works too)
        add_to_playlist: True|False (add to mpd playlist)
        create_zip: True|False (create a zip for the playlist)
    """
    user_input = request.get_json(force=True)
    desc = "Downloading Deezer playlist"
    task = sched.enqueue_task(desc, "download_deezer_playlist_and_queue_and_zip",
                              playlist_id=user_input['playlist_url'],
                              add_to_playlist=user_input['add_to_playlist'],
                              create_zip=user_input['create_zip'])
    return jsonify({"task_id": id(task), })


@app.route('/favorites/deezer', methods=['POST'])
@validate_schema("user_id", "add_to_playlist", "create_zip")
def deezer_favorites_download():
    """
    downloads favorite songs of a Deezer user (looks like this in the brwoser:
       https://www.deezer.com/us/profile/%%user_id%%/loved)
    a subdirecotry with the name of the user_id will be created.
    para:
        user_id: deezer user_id
        add_to_playlist: True|False (add to mpd playlist)
        create_zip: True|False (create a zip for the playlist)
    """
    user_input = request.get_json(force=True)
    desc = "Downloading Deezer favorites"
    task = sched.enqueue_task(desc, "download_deezer_favorites",
                              user_id=user_input['user_id'],
                              add_to_playlist=user_input['add_to_playlist'],
                              create_zip=user_input['create_zip'])
    return jsonify({"task_id": id(task), })


@atexit.register
def stop_workers():
    sched.stop_workers()


# do not put this in 'if __name__ == '__main__':'. Otherwise it won't get executed if run by gunicorn
sched.run_workers(config.getint('threadpool', 'workers'))

if __name__ == '__main__':
    app.run(host=config['http']['host'],
            port=config['http'].getint('port'),
            debug=config['http'].getboolean('debug'),
            use_reloader=False)
