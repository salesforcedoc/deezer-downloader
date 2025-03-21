#!/bin/bash

# assume you have conda installed / python 3.10
source ./conda_env.sh

#some default track
function get_input {
 read -p "Enter track_id: " trackid
 read -p "Enter URL: " url
 echo $trackid
 echo $url
 echo $trackid >> ${0%.*}.log
 echo $url >> ${0%.*}.log
 echo >> ${0%.*}.log
 echo >> ${0%.*}.log
}

while true; do
    get_input
    #validation check
    python3 app/directdl.py -i $trackid -u $url
done;


