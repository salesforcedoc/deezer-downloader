#!/bin/bash

#some default track
track=879754432
url=https://cdnt-proxy-2.dzcdn.net/media/1/eed1f7cedc8b1cb07bbbd1d5e04488db0b9beb58ba6e05785bc93150346cb67bcfb4fc3adec6683da662ecd199c92d46099bce52b95767e6c28ea07c83044de954f9415c02ac428792e048c0f3d170f5?hdnea=exp=1662736578~acl=/media/1/eed1f7cedc8b1cb07bbbd1d5e04488db0b9beb58ba6e05785bc93150346cb67bcfb4fc3adec6683da662ecd199c92d46099bce52b95767e6c28ea07c83044de954f9415c02ac428792e048c0f3d170f5*~data=user_id=1923762066~hmac=d4e8a972411026ceb9504a1ebd778500d4ff1b52c29430df0f13fbdc6628aeaf#127b3056-7b20-4919-b951-fe7902b94d6b

function get_input {
 read -p "Enter track_id and URL: " track url
 echo $url
 echo $track
}

while true; do

    get_input
    #validation check
    python3 app/directdl.py -i $track -u $url

done;


