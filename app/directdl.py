#!/usr/bin/env python3
import sys
import getopt

from deezer import TYPE_TRACK, get_song_infos_from_deezer_website
from music_backend import download_song_and_get_absolute_filename


def main(argv):
    track_id = ''
    track_url = ''
    quality = '9'
    try:
        opts, args = getopt.getopt(argv, "hi:q:u:", ["trackid=", "quality=", "track_url="])
    except getopt.GetoptError:
        print('directdl.py -i <trackid> -u <trackurl> -q <quality>')
        sys.exit(2)
    for opt, arg in opts:
        if opt == '-h':
            print('directdl.py -i <trackid> -u <trackurl> -q <quality>')
            sys.exit()
        elif opt in ("-i", "--trackid"):
            track_id = arg
        elif opt in ("-u", "--trackurl"):
            track_url = arg
        elif opt in ("-q", "--quality"):
            quality = arg
    # check
    assert track_id != '', "you must provide a trackid"
    assert track_url != '', "you must provide a track url"
    print('TrackId: ', track_id)
    print('Quality: ', quality)
    song = get_song_infos_from_deezer_website(TYPE_TRACK, track_id)
    print('Song: ', song)
    absolute_filename = download_song_and_get_absolute_filename(TYPE_TRACK, song, '', track_url)
    print('Saved to: ', absolute_filename)


if __name__ == "__main__":
    main(sys.argv[1:])
